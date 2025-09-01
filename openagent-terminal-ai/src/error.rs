//! Normalized error types for AI providers

use std::fmt;

/// AI provider error types with rich context
#[derive(Debug, Clone)]
pub enum AiError {
    /// Network-related errors
    Network {
        message: String,
        retryable: bool,
        retry_after: Option<std::time::Duration>,
    },
    /// Authentication/authorization errors
    Auth {
        message: String,
        provider: String,
    },
    /// Rate limiting errors
    RateLimit {
        message: String,
        retry_after: Option<std::time::Duration>,
        limit: Option<usize>,
        remaining: Option<usize>,
        reset: Option<std::time::SystemTime>,
    },
    /// Invalid request errors
    InvalidRequest {
        message: String,
        field: Option<String>,
        suggestion: Option<String>,
    },
    /// Provider-specific errors
    Provider {
        provider: String,
        code: Option<String>,
        message: String,
        context: Vec<(String, String)>,
    },
    /// Resource constraints (e.g., local Ollama)
    ResourceConstraint {
        resource: String,
        message: String,
        suggestion: Option<String>,
    },
    /// Configuration errors
    Configuration {
        setting: String,
        message: String,
        suggestion: Option<String>,
    },
    /// Timeout errors
    Timeout {
        operation: String,
        duration: std::time::Duration,
    },
    /// Cancellation by user
    Cancelled,
    /// Generic/unknown errors
    Unknown(String),
}

impl AiError {
    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Network { retryable, .. } => *retryable,
            Self::RateLimit { .. } => true,
            Self::ResourceConstraint { .. } => true,
            Self::Timeout { .. } => true,
            Self::Auth { .. } => false,
            Self::InvalidRequest { .. } => false,
            Self::Configuration { .. } => false,
            Self::Cancelled => false,
            Self::Provider { .. } => false, // Provider should set Network/RateLimit for retryable
            Self::Unknown(_) => false,
        }
    }
    
    /// Get retry delay if applicable
    pub fn retry_delay(&self) -> Option<std::time::Duration> {
        match self {
            Self::Network { retry_after, .. } => *retry_after,
            Self::RateLimit { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
    
    /// Get user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            Self::Network { message, .. } => format!("Network error: {}", message),
            Self::Auth { message, provider } => format!("{} authentication failed: {}", provider, message),
            Self::RateLimit { message, retry_after, .. } => {
                if let Some(duration) = retry_after {
                    format!("Rate limited: {}. Retry in {} seconds", message, duration.as_secs())
                } else {
                    format!("Rate limited: {}", message)
                }
            },
            Self::InvalidRequest { message, suggestion, .. } => {
                if let Some(sugg) = suggestion {
                    format!("Invalid request: {}. {}", message, sugg)
                } else {
                    format!("Invalid request: {}", message)
                }
            },
            Self::Provider { provider, message, .. } => format!("{} error: {}", provider, message),
            Self::ResourceConstraint { resource, message, suggestion } => {
                if let Some(sugg) = suggestion {
                    format!("{} constraint: {}. {}", resource, message, sugg)
                } else {
                    format!("{} constraint: {}", resource, message)
                }
            },
            Self::Configuration { setting, message, suggestion } => {
                if let Some(sugg) = suggestion {
                    format!("Configuration error ({}): {}. {}", setting, message, sugg)
                } else {
                    format!("Configuration error ({}): {}", setting, message)
                }
            },
            Self::Timeout { operation, duration } => {
                format!("{} timed out after {} seconds", operation, duration.as_secs())
            },
            Self::Cancelled => "Operation cancelled by user".to_string(),
            Self::Unknown(msg) => msg.clone(),
        }
    }
}

impl fmt::Display for AiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.user_message())
    }
}

impl std::error::Error for AiError {}

/// Convert common errors into AiError
impl From<String> for AiError {
    fn from(msg: String) -> Self {
        // Try to detect error type from message
        let lower = msg.to_lowercase();
        
        if lower.contains("timeout") {
            Self::Timeout {
                operation: "Request".to_string(),
                duration: std::time::Duration::from_secs(30),
            }
        } else if lower.contains("rate limit") || lower.contains("429") {
            Self::RateLimit {
                message: msg,
                retry_after: None,
                limit: None,
                remaining: None,
                reset: None,
            }
        } else if lower.contains("unauthorized") || lower.contains("401") || lower.contains("403") {
            Self::Auth {
                message: msg.clone(),
                provider: "Unknown".to_string(),
            }
        } else if lower.contains("invalid") || lower.contains("400") {
            Self::InvalidRequest {
                message: msg,
                field: None,
                suggestion: None,
            }
        } else if lower.contains("connection") || lower.contains("network") {
            Self::Network {
                message: msg,
                retryable: true,
                retry_after: None,
            }
        } else {
            Self::Unknown(msg)
        }
    }
}

/// Result type for AI operations
pub type AiResult<T> = Result<T, AiError>;

/// Extension trait for converting provider-specific errors
pub trait IntoAiError {
    fn into_ai_error(self, provider: &str) -> AiError;
}

impl IntoAiError for String {
    fn into_ai_error(self, provider: &str) -> AiError {
        let mut err: AiError = self.into();
        // Update provider name if it's an auth error
        if let AiError::Auth { message, .. } = err {
            err = AiError::Auth {
                message,
                provider: provider.to_string(),
            };
        }
        err
    }
}

#[cfg(feature = "ollama")]
impl IntoAiError for reqwest::Error {
    fn into_ai_error(self, provider: &str) -> AiError {
        if self.is_timeout() {
            AiError::Timeout {
                operation: "HTTP request".to_string(),
                duration: std::time::Duration::from_secs(30),
            }
        } else if self.is_connect() {
            AiError::Network {
                message: format!("Failed to connect to {}: {}", provider, self),
                retryable: true,
                retry_after: Some(std::time::Duration::from_secs(5)),
            }
        } else if let Some(status) = self.status() {
            match status.as_u16() {
                401 | 403 => AiError::Auth {
                    message: self.to_string(),
                    provider: provider.to_string(),
                },
                429 => AiError::RateLimit {
                    message: self.to_string(),
                    retry_after: None,
                    limit: None,
                    remaining: None,
                    reset: None,
                },
                400 => AiError::InvalidRequest {
                    message: self.to_string(),
                    field: None,
                    suggestion: None,
                },
                500..=599 => AiError::Network {
                    message: format!("Server error from {}: {}", provider, self),
                    retryable: true,
                    retry_after: Some(std::time::Duration::from_secs(10)),
                },
                _ => AiError::Provider {
                    provider: provider.to_string(),
                    code: Some(status.to_string()),
                    message: self.to_string(),
                    context: vec![],
                },
            }
        } else {
            AiError::Network {
                message: self.to_string(),
                retryable: !self.is_builder() && !self.is_decode(),
                retry_after: None,
            }
        }
    }
}
