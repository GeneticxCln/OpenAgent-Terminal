// Session Management - Client-side session state tracking and operations
//
// This module provides session management functionality on the Rust frontend,
// coordinating with the Python backend's SessionManager via IPC messages.

use crate::ipc::{IpcClient, IpcError, Request};
use anyhow::Result;
use chrono::{DateTime, Utc};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Message role in a conversation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// A single message in a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<usize>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Session metadata summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub session_id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
    pub total_tokens: usize,
}

/// Full session with messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub metadata: SessionMetadata,
    pub messages: Vec<Message>,
}

/// Session manager client - handles session operations via IPC
pub struct SessionManager {
    ipc_client: Option<*mut IpcClient>,
    current_session_id: Option<String>,
    sessions_cache: HashMap<String, SessionMetadata>,
    request_counter: u64,
}

// Safety: We ensure single-threaded access to the IpcClient pointer
unsafe impl Send for SessionManager {}
unsafe impl Sync for SessionManager {}

impl SessionManager {
    /// Create a new session manager (not yet connected to IPC)
    pub fn new() -> Self {
        Self {
            ipc_client: None,
            current_session_id: None,
            sessions_cache: HashMap::new(),
            request_counter: 10000, // Start high to avoid collision with other request IDs
        }
    }

    /// Set the IPC client reference (must be called before any operations)
    pub fn set_ipc_client(&mut self, client: &mut IpcClient) {
        self.ipc_client = Some(client as *mut IpcClient);
        info!("📝 Session manager connected to IPC client");
    }

    /// Get the current IPC client or return an error
    fn get_ipc_client(&mut self) -> Result<&mut IpcClient, IpcError> {
        match self.ipc_client {
            Some(ptr) => unsafe { Ok(&mut *ptr) },
            None => Err(IpcError::NotConnected),
        }
    }

    /// Get next request ID for IPC calls
    fn next_request_id(&mut self) -> u64 {
        self.request_counter += 1;
        self.request_counter
    }

    /// List all sessions from the backend
    pub async fn list_sessions(&mut self, limit: Option<usize>) -> Result<Vec<SessionMetadata>, IpcError> {
        debug!("📋 Listing sessions (limit: {:?})", limit);

        let request_id = self.next_request_id();
        let client = self.get_ipc_client()?;

        let params = if let Some(limit) = limit {
            serde_json::json!({ "limit": limit })
        } else {
            serde_json::json!({})
        };

        let request = Request::new(request_id, "session.list", Some(params));
        let response = client.send_request(request).await?;

        if let Some(error) = response.error {
            return Err(IpcError::RpcError { code: error.code, message: error.message });
        }

        let result = response.result
            .ok_or_else(|| IpcError::ParseError("No result in response".to_string()))?;

        let sessions_data = result.get("sessions")
            .ok_or_else(|| IpcError::ParseError("No 'sessions' field".to_string()))?;

        let sessions: Vec<SessionMetadata> = serde_json::from_value(sessions_data.clone())
            .map_err(|e| IpcError::ParseError(format!("Failed to parse sessions: {}", e)))?;

        // Update cache
        for session in &sessions {
            self.sessions_cache.insert(session.session_id.clone(), session.clone());
        }

        info!("📋 Retrieved {} sessions", sessions.len());
        Ok(sessions)
    }

    /// Load a specific session from the backend
    pub async fn load_session(&mut self, session_id: &str) -> Result<Session, IpcError> {
        info!("📂 Loading session: {}", session_id);

        let request_id = self.next_request_id();
        let client = self.get_ipc_client()?;

        let params = serde_json::json!({ "session_id": session_id });
        let request = Request::new(request_id, "session.load", Some(params));
        let response = client.send_request(request).await?;

        if let Some(error) = response.error {
            return Err(IpcError::RpcError { code: error.code, message: error.message });
        }

        let result = response.result
            .ok_or_else(|| IpcError::ParseError("No result in response".to_string()))?;

        // Parse the session data
        let session_id_str = result.get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| IpcError::ParseError("Missing session_id".to_string()))?
            .to_string();

        let messages_data = result.get("messages")
            .ok_or_else(|| IpcError::ParseError("Missing messages field".to_string()))?;

        let messages: Vec<Message> = serde_json::from_value(messages_data.clone())
            .map_err(|e| IpcError::ParseError(format!("Failed to parse messages: {}", e)))?;

        // Get or create metadata
        let metadata = if let Some(cached) = self.sessions_cache.get(&session_id_str) {
            cached.clone()
        } else {
            // Create basic metadata from the loaded data
            SessionMetadata {
                session_id: session_id_str.clone(),
                title: format!("Session {}", &session_id_str[..8]),
                created_at: messages.first().map(|m| m.timestamp).unwrap_or_else(Utc::now),
                updated_at: messages.last().map(|m| m.timestamp).unwrap_or_else(Utc::now),
                message_count: messages.len(),
                total_tokens: messages.iter().filter_map(|m| m.token_count).sum(),
            }
        };

        self.current_session_id = Some(session_id_str.clone());

        let session = Session { metadata, messages };
        info!("📂 Loaded session with {} messages", session.messages.len());
        Ok(session)
    }

    /// Export a session to markdown format
    pub async fn export_session(&mut self, session_id: Option<&str>, format: &str) -> Result<String, IpcError> {
        debug!("📤 Exporting session: {:?} as {}", session_id, format);

        let request_id = self.next_request_id();
        let client = self.get_ipc_client()?;

        let params = if let Some(id) = session_id {
            serde_json::json!({
                "session_id": id,
                "format": format
            })
        } else {
            serde_json::json!({ "format": format })
        };

        let request = Request::new(request_id, "session.export", Some(params));
        let response = client.send_request(request).await?;

        if let Some(error) = response.error {
            return Err(IpcError::RpcError { code: error.code, message: error.message });
        }

        let result = response.result
            .ok_or_else(|| IpcError::ParseError("No result in response".to_string()))?;

        let content = result.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| IpcError::ParseError("Missing content field".to_string()))?;

        info!("📤 Exported session ({} bytes)", content.len());
        Ok(content.to_string())
    }

    /// Delete a session
    pub async fn delete_session(&mut self, session_id: &str) -> Result<(), IpcError> {
        info!("🗑️  Deleting session: {}", session_id);

        let request_id = self.next_request_id();
        let client = self.get_ipc_client()?;

        let params = serde_json::json!({ "session_id": session_id });
        let request = Request::new(request_id, "session.delete", Some(params));
        let response = client.send_request(request).await?;

        if let Some(error) = response.error {
            return Err(IpcError::RpcError { code: error.code, message: error.message });
        }

        // Remove from cache
        self.sessions_cache.remove(session_id);

        // Clear current session if it was deleted
        if self.current_session_id.as_deref() == Some(session_id) {
            self.current_session_id = None;
        }

        info!("🗑️  Session deleted: {}", session_id);
        Ok(())
    }

    /// Get the current session ID
    pub fn current_session_id(&self) -> Option<&str> {
        self.current_session_id.as_deref()
    }

    /// Get cached session metadata
    pub fn get_cached_metadata(&self, session_id: &str) -> Option<&SessionMetadata> {
        self.sessions_cache.get(session_id)
    }

    /// Clear the sessions cache
    pub fn clear_cache(&mut self) {
        self.sessions_cache.clear();
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_manager_creation() {
        let manager = SessionManager::new();
        assert!(manager.current_session_id.is_none());
        assert!(manager.sessions_cache.is_empty());
    }

    #[test]
    fn test_message_role_serialization() {
        let role = MessageRole::User;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"user\"");

        let role: MessageRole = serde_json::from_str("\"assistant\"").unwrap();
        assert_eq!(role, MessageRole::Assistant);
    }

    #[test]
    fn test_message_creation() {
        let msg = Message {
            role: MessageRole::User,
            content: "Hello!".to_string(),
            timestamp: Utc::now(),
            token_count: Some(2),
            metadata: HashMap::new(),
        };

        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "Hello!");
        assert_eq!(msg.token_count, Some(2));
    }

    #[test]
    fn test_session_metadata_creation() {
        let metadata = SessionMetadata {
            session_id: "test-123".to_string(),
            title: "Test Session".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            message_count: 5,
            total_tokens: 100,
        };

        assert_eq!(metadata.session_id, "test-123");
        assert_eq!(metadata.message_count, 5);
        assert_eq!(metadata.total_tokens, 100);
    }

    #[test]
    fn test_clear_cache() {
        let mut manager = SessionManager::new();
        let metadata = SessionMetadata {
            session_id: "test-123".to_string(),
            title: "Test".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            message_count: 0,
            total_tokens: 0,
        };

        manager.sessions_cache.insert("test-123".to_string(), metadata);
        assert_eq!(manager.sessions_cache.len(), 1);

        manager.clear_cache();
        assert_eq!(manager.sessions_cache.len(), 0);
    }

    #[test]
    fn test_get_cached_metadata() {
        let mut manager = SessionManager::new();
        let metadata = SessionMetadata {
            session_id: "test-123".to_string(),
            title: "Test".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            message_count: 3,
            total_tokens: 50,
        };

        manager.sessions_cache.insert("test-123".to_string(), metadata.clone());

        let cached = manager.get_cached_metadata("test-123");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().message_count, 3);

        let not_found = manager.get_cached_metadata("nonexistent");
        assert!(not_found.is_none());
    }
}
