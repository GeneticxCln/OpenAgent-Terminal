// IPC Message Types - JSON-RPC 2.0 Messages

use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 Request
/// Uses deny_unknown_fields for strict validation to catch protocol drift
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub jsonrpc: String, // Always "2.0"
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 Response
/// Uses deny_unknown_fields for strict validation to catch protocol drift
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// JSON-RPC 2.0 Notification (no response expected)
/// Uses deny_unknown_fields for strict validation to catch protocol drift
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// Request ID (can be number or string)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RequestId {
    Number(u64),
    String(String),
}

/// JSON-RPC Error
/// Uses deny_unknown_fields for strict validation to catch protocol drift
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Tolerant wrapper for parsing messages with unknown fields
/// Used for logging unknown fields without failing the parse
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TolerantMessage {
    #[allow(dead_code)] // Used internally for protocol version validation
    pub jsonrpc: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    #[allow(dead_code)] // Used internally for response correlation
    pub id: Option<Value>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl TolerantMessage {
    /// Check for and log unknown fields
    pub fn log_unknown_fields(&self) {
        let expected_fields = ["jsonrpc", "method", "id", "params", "result", "error"];
        let unknown: Vec<&String> = self.extra.keys()
            .filter(|k| !expected_fields.contains(&k.as_str()))
            .collect();
        
        if !unknown.is_empty() {
            let method_info = self.method.as_deref().unwrap_or("response");
            warn!("⚠️  Protocol drift detected in '{}': unknown fields {:?}", method_info, unknown);
        }
    }
}

impl Request {
    /// Create a new request
    pub fn new(id: u64, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: RequestId::Number(id),
            method: method.into(),
            params,
        }
    }

    /// Create initialize request (see IPC_PROTOCOL.md)
    /// Uses actual terminal size if available, falls back to defaults
    pub fn initialize(id: u64) -> Self {
        // Try to get actual terminal size
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
        
        let params = serde_json::json!({
            "protocol_version": "1.0.0",
            "client_info": {
                "name": "openagent-terminal",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "terminal_size": {
                "cols": cols,
                "rows": rows,
            },
            "capabilities": [
                "streaming",
                "blocks",
                "syntax_highlighting",
            ],
        });

        Self::new(id, "initialize", Some(params))
    }

    /// Create agent.query request
    pub fn agent_query(id: u64, message: impl Into<String>) -> Self {
        let params = serde_json::json!({
            "message": message.into(),
            "options": {
                "stream": true,
            },
        });

        Self::new(id, "agent.query", Some(params))
    }
}

impl Notification {
    /// Create a new notification
    #[allow(dead_code)] // Used in tests and future features
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
        }
    }

    /// Create context.update notification with working directory
    #[allow(dead_code)] // For backward compatibility
    pub fn context_update(cwd: impl Into<String>) -> Self {
        let params = serde_json::json!({
            "cwd": cwd.into(),
        });

        Self::new("context.update", Some(params))
    }
    
    /// Create context.update notification with terminal size
    pub fn context_update_terminal_size(cols: u16, rows: u16) -> Self {
        let params = serde_json::json!({
            "terminal_size": {
                "cols": cols,
                "rows": rows,
            },
        });

        Self::new("context.update", Some(params))
    }
    
    /// Create context.update notification with multiple context fields
    #[allow(dead_code)] // Public API for future multi-field context updates
    pub fn context_update_full(cwd: Option<String>, terminal_size: Option<(u16, u16)>) -> Self {
        let mut context = serde_json::Map::new();
        
        if let Some(cwd) = cwd {
            context.insert("cwd".to_string(), serde_json::json!(cwd));
        }
        
        if let Some((cols, rows)) = terminal_size {
            context.insert("terminal_size".to_string(), serde_json::json!({
                "cols": cols,
                "rows": rows,
            }));
        }
        
        Self::new("context.update", Some(serde_json::Value::Object(context)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_request() {
        let req = Request::new(1, "test_method", None);
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"test_method\""));
    }

    #[test]
    fn test_initialize_request() {
        let req = Request::initialize(1);
        assert_eq!(req.method, "initialize");
        assert!(req.params.is_some());
        // Verify it contains terminal_size
        let params = req.params.unwrap();
        assert!(params.get("terminal_size").is_some());
    }
}
