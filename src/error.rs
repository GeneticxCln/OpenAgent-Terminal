// Error types for OpenAgent-Terminal
//
// Provides structured, user-friendly error messages with context.

use std::fmt;
use thiserror::Error;

/// Main error type for OpenAgent-Terminal
#[derive(Error, Debug)]
pub enum TerminalError {
    /// Failed to connect to the Python backend
    #[error("Failed to connect to backend at {path}\n\n\
             Possible solutions:\n\
             1. Make sure the Python backend is running:\n   \
                cd backend && python -m openagent_terminal.bridge\n\
             2. Check if the socket path is correct:\n   \
                ls -la {path}\n\
             3. Try setting a custom socket path:\n   \
                export OPENAGENT_SOCKET=/path/to/socket.sock\n\n\
             Error details: {source}")]
    BackendConnectionError {
        path: String,
        source: std::io::Error,
    },

    /// Backend disconnected unexpectedly
    #[error("Backend disconnected unexpectedly\n\n\
             This usually means the Python backend crashed or was terminated.\n\
             Check the backend logs for error messages.\n\n\
             Error details: {0}")]
    BackendDisconnected(String),

    /// Failed to initialize connection
    #[error("Failed to initialize connection with backend\n\n\
             The backend responded but initialization failed.\n\
             This might indicate a version mismatch or protocol error.\n\n\
             Error details: {0}")]
    InitializationError(String),

    /// Agent query failed
    #[error("Agent query failed: {0}\n\n\
             The AI agent encountered an error processing your query.\n\
             This could be due to:\n\
             - Invalid query format\n\
             - Backend processing error\n\
             - Model unavailable\n\n\
             Try rephrasing your query or check backend logs.")]
    AgentQueryError(String),

    /// Tool execution failed
    #[error("Tool execution failed: {tool}\n\n\
             The tool '{tool}' failed to execute.\n\
             Reason: {reason}\n\n\
             This could be due to:\n\
             - Invalid parameters\n\
             - Insufficient permissions\n\
             - File not found\n\
             - Path safety restrictions\n\n\
             Check the tool parameters and try again.")]
    ToolExecutionError {
        tool: String,
        reason: String,
    },

    /// Configuration error
    #[error("Configuration error: {0}\n\n\
             There was a problem with your configuration file.\n\
             Location: ~/.config/openagent-terminal/config.toml\n\n\
             Possible solutions:\n\
             1. Check the config file syntax\n\
             2. Remove the config file to use defaults\n\
             3. Copy config.example.toml as a template")]
    ConfigError(String),

    /// IPC protocol error
    #[error("IPC protocol error: {0}\n\n\
             There was a problem with the communication protocol.\n\
             This might indicate:\n\
             - Version mismatch between frontend and backend\n\
             - Corrupted message\n\
             - Protocol violation\n\n\
             Try restarting both frontend and backend.")]
    ProtocolError(String),

    /// Request timeout
    #[error("Request timed out after {seconds} seconds\n\n\
             The backend took too long to respond.\n\
             This could mean:\n\
             - The backend is busy processing\n\
             - The query is too complex\n\
             - The backend is unresponsive\n\n\
             Try a simpler query or restart the backend.")]
    Timeout {
        seconds: u64,
    },

    /// IO error
    #[error("IO error: {0}\n\n\
             A file system operation failed.\n\
             Check permissions and disk space.")]
    IoError(#[from] std::io::Error),

    /// Other errors
    #[error("{0}")]
    Other(String),
}

impl TerminalError {
    /// Create a backend connection error
    pub fn backend_connection(path: impl Into<String>, source: std::io::Error) -> Self {
        Self::BackendConnectionError {
            path: path.into(),
            source,
        }
    }

    /// Create a tool execution error
    pub fn tool_execution(tool: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ToolExecutionError {
            tool: tool.into(),
            reason: reason.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout(seconds: u64) -> Self {
        Self::Timeout { seconds }
    }

    /// Check if this is a connection error
    pub fn is_connection_error(&self) -> bool {
        matches!(self, Self::BackendConnectionError { .. } | Self::BackendDisconnected(_))
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::BackendConnectionError { .. }
                | Self::Timeout { .. }
                | Self::AgentQueryError(_)
        )
    }

    /// Get a short error message suitable for display
    pub fn short_message(&self) -> String {
        match self {
            Self::BackendConnectionError { .. } => "Backend connection failed".to_string(),
            Self::BackendDisconnected(_) => "Backend disconnected".to_string(),
            Self::InitializationError(_) => "Initialization failed".to_string(),
            Self::AgentQueryError(_) => "Agent query failed".to_string(),
            Self::ToolExecutionError { tool, .. } => format!("Tool '{}' failed", tool),
            Self::ConfigError(_) => "Configuration error".to_string(),
            Self::ProtocolError(_) => "Protocol error".to_string(),
            Self::Timeout { seconds } => format!("Timeout after {}s", seconds),
            Self::IoError(_) => "IO error".to_string(),
            Self::Other(msg) => msg.clone(),
        }
    }
}

/// Result type alias using TerminalError
pub type Result<T> = std::result::Result<T, TerminalError>;

/// Retry configuration for operations
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    
    /// Initial delay between retries in milliseconds
    pub initial_delay_ms: u64,
    
    /// Maximum delay between retries in milliseconds
    pub max_delay_ms: u64,
    
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Create a retry config for connection attempts
    pub fn for_connection() -> Self {
        Self {
            max_attempts: 5,
            initial_delay_ms: 200,
            max_delay_ms: 3000,
            backoff_multiplier: 1.5,
        }
    }

    /// Create a retry config for agent queries
    pub fn for_query() -> Self {
        Self {
            max_attempts: 2,
            initial_delay_ms: 500,
            max_delay_ms: 2000,
            backoff_multiplier: 2.0,
        }
    }

    /// Calculate delay for a given attempt number
    pub fn delay_for_attempt(&self, attempt: u32) -> std::time::Duration {
        let delay_ms = (self.initial_delay_ms as f64
            * self.backoff_multiplier.powi(attempt as i32))
            .min(self.max_delay_ms as f64) as u64;
        std::time::Duration::from_millis(delay_ms)
    }
}

/// Helper macro for retrying operations
#[macro_export]
macro_rules! retry {
    ($config:expr, $operation:expr) => {{
        let mut last_error = None;
        
        for attempt in 0..$config.max_attempts {
            if attempt > 0 {
                let delay = $config.delay_for_attempt(attempt - 1);
                log::debug!("Retry attempt {} after {:?}", attempt + 1, delay);
                tokio::time::sleep(delay).await;
            }
            
            match $operation.await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    log::warn!("Attempt {} failed: {}", attempt + 1, e);
                    last_error = Some(e);
                }
            }
        }
        
        Err(last_error.unwrap())
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_messages() {
        let err = TerminalError::backend_connection(
            "/tmp/test.sock",
            std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        );
        assert!(err.to_string().contains("Failed to connect"));
        assert!(err.to_string().contains("/tmp/test.sock"));
    }

    #[test]
    fn test_is_recoverable() {
        let conn_err = TerminalError::backend_connection(
            "/tmp/test.sock",
            std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        );
        assert!(conn_err.is_recoverable());

        let config_err = TerminalError::ConfigError("test".to_string());
        assert!(!config_err.is_recoverable());
    }

    #[test]
    fn test_short_message() {
        let err = TerminalError::timeout(30);
        assert_eq!(err.short_message(), "Timeout after 30s");
    }

    #[test]
    fn test_retry_config() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);

        let delay = config.delay_for_attempt(0);
        assert_eq!(delay.as_millis(), 100);

        let delay = config.delay_for_attempt(2);
        assert!(delay.as_millis() > 100);
    }

    #[test]
    fn test_connection_retry_config() {
        let config = RetryConfig::for_connection();
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.initial_delay_ms, 200);
    }
}
