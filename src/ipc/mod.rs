// IPC Module - JSON-RPC over Unix Domain Socket
// Communication between Rust frontend and Python backend

pub mod client;
pub mod message;
pub mod error;

// Re-exports for convenience (used in main.rs)
#[allow(unused_imports)] // These ARE used in main.rs, false positive warning
pub use client::IpcClient;
#[allow(unused_imports)]
pub use message::{Request, Response, Notification};
#[allow(unused_imports)]
pub use error::IpcError;

// TODO: Implement in Phase 1
// - Unix socket connection
// - JSON-RPC message handling
// - Async request/response
// - Notification handling
