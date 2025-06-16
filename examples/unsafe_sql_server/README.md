Unsafe SQL Server Example
This document describes the "Unsafe SQL Server," an example implementation of an MCP server that allows a client, through an LLM, to execute raw SQL queries against a local database.

⚠️ Security Warning

This example is for educational purposes only and demonstrates a highly insecure pattern. It is designed to execute any raw SQL query it receives from an LLM. Allowing an external entity to execute arbitrary SQL presents a significant security risk, including but not limited to:

Data Destruction: DROP TABLE, DELETE, and TRUNCATE commands could be executed.
Data Theft: Unrestricted SELECT queries could expose sensitive information.
SQL Injection: While the immediate client might be trusted, the pattern itself is vulnerable.
Do not use this code in a production environment without implementing significant security controls, such as query sanitization, read-only modes, or a more granular, safer set of tools instead of raw SQL execution.

Project Goal
The purpose of this server is to act as a bridge between an MCP-compliant client (like Warp Terminal) and a local database. It empowers a Large Language Model (LLM) to inspect the database schema and execute commands to create tables, insert data, and run queries based on natural language prompts.

How It Works
A user issues a natural language command in their MCP client (e.g., "Show me all users from the customers table").
The client forwards this to an LLM, which translates the command into a structured JSON-RPC request to call a tool provided by this server.
This server receives the request, and its corresponding handler executes the command against the local database.
The result from the database (e.g., a success message or a set of rows) is packaged into a JSON-RPC response and sent back to the client.
Implemented Tools
This server will expose the following tools to the LLM:

get_schema()

Description: Retrieves the schema of the connected database, including table and column names. This is a crucial read-only tool that provides the LLM with the necessary context to construct accurate queries.
Arguments: None.
execute_sql(query: String)

Description: Executes a raw SQL query string against the database. This is the primary tool for all data manipulation and querying.
Arguments:
query: A string containing the SQL statement to execute.
Setup & Usage
Database Configuration: This server requires a local database to connect to. The implementation will need a database-specific Rust crate (e.g., rusqlite for SQLite, sqlx for PostgreSQL, etc.). You will need to configure the connection details in main.rs.

Dependencies: Add the chosen database crate to the Cargo.toml of this example. For instance, for SQLite:

Ini, TOML

[dependencies]
rusqlite = "0.30"
Running the Server: Launch the server from the root of the repository workspace:

Bash

cargo run -p unsafe_sql_server -- --port 8080
Client Configuration: Configure your MCP client (e.g., Warp Terminal) to connect to the server at the address it's listening on (e.g., 127.0.0.1:8080).
