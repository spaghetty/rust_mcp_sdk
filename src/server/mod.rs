//! Defines the public API for the MCP server.
//!
//! This module declares the sub-modules for the server implementation and re-exports
//! the primary, public-facing types like `Server` and `ConnectionHandle`.

// 1. Declare the child modules. The code for these lives in
//    `server/server.rs` and `server/session.rs`.
mod server;
mod session;

// 2. Publicly re-export the types that consumers of our library will use.
pub use server::Server;
pub use session::ConnectionHandle;
