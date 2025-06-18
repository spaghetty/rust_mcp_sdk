// examples/unsafe_sql_server/src/main.rs

use clap::Parser;
use mcp_sdk::{
    error::{Error, Result},
    CallToolResult, ConnectionHandle, Content, Server, StdioAdapter, Tool, ToolArguments,
};
use rusqlite::Connection;
// use serde_json::json; // No longer needed
use serde::Deserialize; // Added for deriving Deserialize
                        // use serde_json::Value; // No longer needed for args
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, default_value = "local_database.db")]
    db_file: String,
}

#[derive(ToolArguments, Deserialize)] // Added Deserialize
struct GetSchemaArgs {}

#[derive(ToolArguments, Deserialize)] // Added Deserialize
struct ExecuteSqlArgs {
    #[tool_arg(desc = "The SQL query to execute.")]
    query: String,
}

struct ServerState {
    db_path: String,
}

// Helper to convert any error into our SDK's Error::Other variant.
fn to_sdk_error<E: std::fmt::Display>(err: E) -> Error {
    Error::Other(err.to_string())
}

async fn get_schema_handler(
    state: Arc<ServerState>,
    _handle: ConnectionHandle,
    _args: GetSchemaArgs, // New: typed struct
) -> Result<CallToolResult> {
    info!("Handling 'get_schema' request");
    let db_path = state.db_path.clone();

    // The closure now explicitly returns our SDK's Result type.
    let result_text = tokio::task::spawn_blocking(move || -> Result<String> {
        let conn = Connection::open(db_path).map_err(to_sdk_error)?;
        let mut stmt = conn.prepare("SELECT name, sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%';").map_err(to_sdk_error)?;
        let mut rows = stmt.query([]).map_err(to_sdk_error)?;
        let mut schema_text = String::new();

        // Every `?` inside this loop now operates on a `rusqlite::Result`,
        // so we must map the error type for each one.
        while let Some(row) = rows.next().map_err(to_sdk_error)? {
            let table_name: String = row.get(0).map_err(to_sdk_error)?;
            let sql: String = row.get(1).map_err(to_sdk_error)?;
            schema_text.push_str(&format!("-- Table: {}\n{}\n\n", table_name, sql));
        }
        Ok(schema_text)
    })
    .await.map_err(to_sdk_error)??; // The outer `??` handles JoinError and the inner Result

    Ok(CallToolResult {
        content: vec![Content::Text { text: result_text }],
        is_error: false,
    })
}

async fn execute_sql_handler(
    state: Arc<ServerState>,
    _handle: ConnectionHandle,
    args: ExecuteSqlArgs, // New: typed struct
) -> Result<CallToolResult> {
    // args.query is String.
    // Both query_for_prepare and query_clone_for_spawn need to be owned Strings
    // to satisfy the 'static lifetime for spawn_blocking.
    let query_owned_for_spawn = args.query.clone();

    let db_path = state.db_path.clone();

    let result_text = tokio::task::spawn_blocking(move || -> Result<String> {
        // Use the owned string inside spawn_blocking
        debug!(query = %query_owned_for_spawn, "Executing SQL");
        let conn = Connection::open(db_path).map_err(to_sdk_error)?;

        if query_owned_for_spawn
            .trim()
            .to_lowercase()
            .starts_with("select")
        {
            let mut stmt = conn.prepare(&query_owned_for_spawn).map_err(to_sdk_error)?;
            let column_names: Vec<String> = stmt
                .column_names()
                .into_iter()
                .map(|s| s.to_string())
                .collect();
            let column_count = stmt.column_count();
            let mut rows = stmt.query([]).map_err(to_sdk_error)?;
            let mut result_text = String::new();
            result_text.push_str(&column_names.join(" | "));
            result_text.push('\n');

            while let Some(row) = rows.next().map_err(to_sdk_error)? {
                for i in 0..column_count {
                    let value: rusqlite::types::Value = row.get(i).map_err(to_sdk_error)?;
                    let value_str = match value {
                        rusqlite::types::Value::Null => "NULL".to_string(),
                        rusqlite::types::Value::Integer(i) => i.to_string(),
                        rusqlite::types::Value::Real(f) => f.to_string(),
                        rusqlite::types::Value::Text(t) => t,
                        rusqlite::types::Value::Blob(_) => "[BLOB]".to_string(),
                    };
                    result_text.push_str(&value_str);
                    if i < column_count - 1 {
                        result_text.push_str(" | ");
                    }
                }
                result_text.push('\n');
            }
            Ok(result_text)
        } else {
            let rows_affected = conn
                .execute(&query_owned_for_spawn, [])
                .map_err(to_sdk_error)?;
            Ok(format!(
                "Query executed successfully. Rows affected: {}",
                rows_affected
            ))
        }
    })
    .await
    .map_err(to_sdk_error)??;

    Ok(CallToolResult {
        content: vec![Content::Text { text: result_text }],
        is_error: false,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // 1. Create an appender that writes to a daily rotating log file.
    //    Log files will be created in a `logs` directory in your project root.
    let file_appender = tracing_appender::rolling::daily("logs", "server.log");

    // 2. Create a non-blocking writer. This is a performance optimization.
    //    The `_guard` must be kept in scope for the logs to be flushed.
    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(file_appender);

    // 3. Build the subscriber.
    let subscriber = tracing_subscriber::fmt()
        .with_writer(
            // To see logs in both the console and the file, we can combine writers.
            non_blocking_writer,
        )
        // Use the RUST_LOG environment variable for filtering, same as before.
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    // 4. Set the subscriber as the global default.
    tracing::subscriber::set_global_default(subscriber)
        .expect("Unable to set global tracing subscriber");

    // Create channels to listen for termination signals.
    let pid = std::process::id();
    info!(pid, "Server process starting.");
    // SIGTERM is the standard "graceful shutdown" signal.
    let mut sigterm = signal(SignalKind::terminate())?;
    // SIGINT is for Ctrl+C.
    let mut sigint = signal(SignalKind::interrupt())?;

    info!("[Server] Initializing database at '{}'...", args.db_file);
    let conn = Connection::open(&args.db_file).map_err(to_sdk_error)?;
    conn.execute("CREATE TABLE IF NOT EXISTS tasks (id INTEGER PRIMARY KEY, title TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending')", []).map_err(to_sdk_error)?;
    info!("[Server] Database initialized successfully.");
    let shared_state = Arc::new(ServerState {
        db_path: args.db_file,
    });
    let server = Server::new("unsafe-sql-server")
        .register_tool_typed(
            // Use register_tool_typed
            Tool::from_args::<GetSchemaArgs>(
                // Use Tool::from_args
                "get_schema",
                Some("Retrieves the SQL schema for all tables."),
            ),
            {
                let state = Arc::clone(&shared_state);
                move |conn_handle, tool_args: GetSchemaArgs| {
                    get_schema_handler(state.clone(), conn_handle, tool_args)
                }
            },
        )
        .register_tool_typed(
            Tool::from_args::<ExecuteSqlArgs>(
                "execute_sql",
                Some("Executes a raw SQL query against the database."),
            ),
            {
                let state = Arc::clone(&shared_state);
                move |conn_handle, tool_args: ExecuteSqlArgs| {
                    execute_sql_handler(state.clone(), conn_handle, tool_args)
                }
            },
        );

    // Use the simple `serve()` method for stdio communication with Warp.
    // The main function explicitly creates the adapter...
    let adapter = StdioAdapter::new();

    // ...and tells the server to handle the single stdio connection.
    info!("[Server] Starting session on stdio.");

    tokio::select! {
        // Branch 1: The main server logic.
        res = server.handle_connection(adapter) => {
            if let Err(e) = res {
                error!(error = %e, "Server handle_connection returned an error.");
            }
        },

        // Branch 2: Listen for the SIGTERM signal.
        _ = sigterm.recv() => {
            warn!(pid, "Received SIGTERM signal. Process is being terminated externally.");
        },

        // Branch 3: Listen for the SIGINT signal (Ctrl+C).
        _ = sigint.recv() => {
            warn!(pid, "Received SIGINT (Ctrl+C) signal. Shutting down.");
        }
    }

    info!(pid, "[Server] process exiting.");

    Ok(())
}
