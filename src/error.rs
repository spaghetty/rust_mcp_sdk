//! Defines the custom `Error` and `Result` types for the MCP SDK.

use crate::types::ErrorData;
use std::fmt;

/// The primary error type for the MCP SDK.
///
/// This enum consolidates all possible failures that can occur within the SDK,
/// allowing users to programmatically handle different error conditions.
#[derive(Debug)]
pub enum Error {
    /// An error that occurred during network I/O operations (e.g., connection refused,
    /// connection reset). This typically wraps a `std::io::Error`.
    Io(std::io::Error),

    /// An error that occurred during JSON serialization or deserialization. This indicates
    /// a problem with message formatting or a mismatch between the expected and received
    /// data structures.
    Serialization(serde_json::Error),

    /// A JSON-RPC error response was received from the peer. This means the request was
    /// well-formed, but the server encountered an error processing it (e.g., method not found).
    JsonRpc(ErrorData),

    /// An internal channel for asynchronous operations was closed unexpectedly,
    /// often indicating that a background task has panicked or been terminated.
    ChannelClosed,

    /// The future waiting for a response timed out.
    Timeout,

    /// A general-purpose error for miscellaneous issues that don't fit into other categories.
    Other(String),
}

/// A specialized `Result` type for the MCP SDK.
///
/// This type alias is used throughout the SDK for functions that can return
/// one of the variants of the `Error` enum.
pub type Result<T> = std::result::Result<T, Error>;

// --- Error Trait Implementation ---

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {}", e),
            Error::Serialization(e) => write!(f, "Serialization error: {}", e),
            Error::JsonRpc(e) => write!(f, "JSON-RPC error (code {}): {}", e.code, e.message),
            Error::ChannelClosed => write!(f, "Internal communication channel closed"),
            Error::Timeout => write!(f, "Operation timed out"),
            Error::Other(msg) => write!(f, "An internal error occurred: {}", msg),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Serialization(e) => Some(e),
            _ => None,
        }
    }
}

// --- From Implementations for Error Conversion ---

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err)
    }
}

impl From<ErrorData> for Error {
    fn from(err: ErrorData) -> Self {
        Error::JsonRpc(err)
    }
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for Error {
    fn from(_: tokio::sync::mpsc::error::SendError<T>) -> Self {
        Error::ChannelClosed
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for Error {
    fn from(_: tokio::sync::oneshot::error::RecvError) -> Self {
        Error::ChannelClosed
    }
}

impl From<tokio::time::error::Elapsed> for Error {
    fn from(_: tokio::time::error::Elapsed) -> Self {
        Error::Timeout
    }
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Error::Other(msg)
    }
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self {
        Error::Other(msg.to_string())
    }
}
