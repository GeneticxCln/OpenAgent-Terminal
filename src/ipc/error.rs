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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_error() {
        let err = IpcError::ConnectionError("test error".to_string());
        assert!(err.to_string().contains("Connection error"));
        assert!(err.to_string().contains("test error"));
    }

    #[test]
    fn test_socket_not_found() {
        let path = "/tmp/test.sock";
        let err = IpcError::SocketNotFound(path.to_string());
        assert!(err.to_string().contains("Socket not found"));
        assert!(err.to_string().contains(path));
    }

    #[test]
    fn test_send_failed() {
        let err = IpcError::SendFailed("network error".to_string());
        assert!(err.to_string().contains("Failed to send"));
    }

    #[test]
    fn test_receive_failed() {
        let err = IpcError::ReceiveFailed("timeout".to_string());
        assert!(err.to_string().contains("Failed to receive"));
    }

    #[test]
    fn test_serialization_error() {
        let err = IpcError::SerializationError("invalid json".to_string());
        assert!(err.to_string().contains("serialization error"));
    }

    #[test]
    fn test_parse_error() {
        let err = IpcError::ParseError("unexpected token".to_string());
        assert!(err.to_string().contains("parse error"));
    }

    #[test]
    fn test_protocol_error() {
        let err = IpcError::ProtocolError("version mismatch".to_string());
        assert!(err.to_string().contains("Protocol error"));
    }

    #[test]
    fn test_timeout() {
        let err = IpcError::Timeout;
        assert!(err.to_string().contains("Timeout"));
    }

    #[test]
    fn test_rpc_error() {
        let err = IpcError::rpc(-32600, "Invalid Request");
        let error_string = err.to_string();
        assert!(error_string.contains("RPC error"));
        assert!(error_string.contains("-32600"));
        assert!(error_string.contains("Invalid Request"));
    }

    #[test]
    fn test_rpc_error_with_string() {
        let err = IpcError::rpc(-32601, "Method not found".to_string());
        assert!(err.to_string().contains("-32601"));
        assert!(err.to_string().contains("Method not found"));
    }

    #[test]
    fn test_not_connected() {
        let err = IpcError::NotConnected;
        assert_eq!(err.to_string(), "Not connected");
    }

    #[test]
    fn test_internal_error() {
        let err = IpcError::InternalError("unexpected state".to_string());
        assert!(err.to_string().contains("Internal error"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let ipc_err: IpcError = io_err.into();
        assert!(ipc_err.to_string().contains("IO error"));
    }

    #[test]
    fn test_error_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<IpcError>();
    }

    #[test]
    fn test_error_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<IpcError>();
    }
}
