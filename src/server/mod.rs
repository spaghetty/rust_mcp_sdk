//! Defines the public API for the MCP server.
//!
//! This module declares the sub-modules for the server implementation and re-exports
//! the primary, public-facing types like `Server` and `ConnectionHandle`.

// 1. Declare the child modules. The code for these lives in
//    `server/server.rs` and `server/session.rs`.
mod server;
pub mod session; // Made public for integration tests

// 2. Publicly re-export the types that consumers of our library will use.
pub use server::Server;
pub use session::{ConnectionHandle, ServerSession}; // Also re-export ServerSession
