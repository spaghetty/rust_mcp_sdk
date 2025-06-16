// src/network_adapter/mod.rs
pub mod lsp;
pub mod ndjson;
pub mod stdio;
pub mod r#trait; // Using r# to use the keyword `trait` as a module name

pub use lsp::LspAdapter;
pub use ndjson::NdjsonAdapter;
pub use r#trait::NetworkAdapter;
pub use stdio::StdioAdapter;
