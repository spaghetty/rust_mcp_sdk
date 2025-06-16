// examples/unsafe_sql_server/src/main.rs

use clap::Parser;
use mcp_sdk::{
    error::{Error, Result},
    CallToolResult, ConnectionHandle, Content, Server, StdioAdapter,
};
use rusqlite::{Connection, ToSql};
use serde_json::Value;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, default_value = "local_database.db")]
    db_file: String,
}

struct ServerState {
    db_path: String,
}

// Helper to convert any error into our SDK's Error::Other variant.
fn to_sdk_error<E: std::fmt::Display>(err: E) -> Error {
    Error::Other(err.to_string())
}

async fn get_schema_handler(state: Arc<ServerState>) -> Result<String> {
    let db_path = state.db_path.clone();
    tokio::task::spawn_blocking(move || {
        let conn = Connection::open(db_path).map_err(to_sdk_error)?;
        let mut stmt = conn.prepare("SELECT name, sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%';").map_err(to_sdk_error)?;
        let mut rows = stmt.query([]).map_err(to_sdk_error)?;
        let mut schema_text = String::new();
        while let Some(row) = rows.next().map_err(to_sdk_error)? {
            let table_name: String = row.get(0).map_err(to_sdk_error)?;
            let sql: String = row.get(1).map_err(to_sdk_error)?;
            schema_text.push_str(&format!("-- Table: {}\n{}\n\n", table_name, sql));
        }
        Ok(schema_text)
    })
    .await
    .map_err(to_sdk_error)? // Convert JoinError
}

async fn execute_sql_handler(state: Arc<ServerState>, query: String) -> Result<String> {
    let db_path = state.db_path.clone();
    tokio::task::spawn_blocking(move || {
        println!("[DB] Executing query: {}", query);
        let conn = Connection::open(db_path).map_err(to_sdk_error)?;

        if query.trim().to_lowercase().starts_with("select") {
            let mut stmt = conn.prepare(&query).map_err(to_sdk_error)?;

            // --- CORRECTED LOGIC ---
            // 1. Get info from the statement BEFORE creating the row iterator.
            let column_names: Vec<String> = stmt
                .column_names()
                .into_iter()
                .map(|s| s.to_string())
                .collect();
            let column_count = stmt.column_count();

            // 2. NOW, create the row iterator. `stmt` is now borrowed by `rows`.
            let mut rows = stmt.query([]).map_err(to_sdk_error)?;
            let mut result_text = String::new();
            result_text.push_str(&column_names.join(" | "));
            result_text.push('\n');

            // 3. Loop through the rows.
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
                .execute(&query, &[] as &[&dyn ToSql])
                .map_err(to_sdk_error)?;
            Ok(format!(
                "Query executed successfully. Rows affected: {}",
                rows_affected
            ))
        }
    })
    .await
    .map_err(to_sdk_error)?
}

async fn call_tool_dispatcher(
    state: Arc<ServerState>,
    _handle: ConnectionHandle,
    name: String,
    args: Value,
) -> Result<CallToolResult> {
    let result_text = match name.as_str() {
        "get_schema" => get_schema_handler(state).await?,
        "execute_sql" => {
            let query = args
                .get("query")
                .and_then(Value::as_str)
                .ok_or_else(|| Error::Other("Missing 'query' argument for execute_sql".into()))?;
            execute_sql_handler(state, query.to_string()).await?
        }
        _ => return Err(Error::Other(format!("Unknown tool called: {}", name))),
    };

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

    info!("[Server] Initializing database at '{}'...", args.db_file);
    let conn = Connection::open(&args.db_file).map_err(to_sdk_error)?;
    conn.execute("CREATE TABLE IF NOT EXISTS tasks (id INTEGER PRIMARY KEY, title TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending')", []).map_err(to_sdk_error)?;
    info!("[Server] Database initialized successfully.");
    let shared_state = Arc::new(ServerState {
        db_path: args.db_file,
    });
    let server = Server::new("unsafe-sql-server").on_call_tool({
        let state = Arc::clone(&shared_state);
        move |handle, name, value| call_tool_dispatcher(state.clone(), handle, name, value)
    });

    // --- Final Execution Logic for Stdio ---
    // The main function explicitly creates the adapter...
    let adapter = StdioAdapter::new();

    // ...and tells the server to handle the single stdio connection.
    info!("[Server] Starting session on stdio.");
    server.handle_connection(adapter).await?;
    info!("[Server] Stdio session ended.");

    Ok(())
}
