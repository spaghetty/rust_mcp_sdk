//! Defines the public API for the MCP client.

mod client;
mod session;
mod session_group;

pub use client::Client;
pub use session_group::ClientSessionGroup;
