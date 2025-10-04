// IPC Message Types - JSON-RPC 2.0 Messages

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String, // Always "2.0"
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// JSON-RPC 2.0 Notification (no response expected)
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
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
    pub fn initialize(id: u64, cols: u16, rows: u16) -> Self {
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

    /// Create context.update notification
    #[allow(dead_code)] // For future context management features
    pub fn context_update(cwd: impl Into<String>) -> Self {
        let params = serde_json::json!({
            "cwd": cwd.into(),
        });

        Self::new("context.update", Some(params))
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
        let req = Request::initialize(1, 80, 24);
        assert_eq!(req.method, "initialize");
        assert!(req.params.is_some());
    }
}
