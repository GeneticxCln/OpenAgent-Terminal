// IPC Error Types

use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)] // Not all error variants are currently used
pub enum IpcError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Socket not found at path: {0}")]
    SocketNotFound(String),

    #[error("Failed to send message: {0}")]
    SendFailed(String),

    #[error("Failed to receive message: {0}")]
    ReceiveFailed(String),

    #[error("JSON serialization error: {0}")]
    SerializationError(String),

    #[error("JSON parse error: {0}")]
    ParseError(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Timeout waiting for response")]
    Timeout,

    #[error("RPC error (code {code}): {message}")]
    RpcError { code: i32, message: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Not connected")]
    NotConnected,

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl IpcError {
    /// Create an RPC error
    #[allow(dead_code)] // For future error handling
    pub fn rpc(code: i32, message: impl Into<String>) -> Self {
        Self::RpcError {
            code,
            message: message.into(),
        }
    }
}
