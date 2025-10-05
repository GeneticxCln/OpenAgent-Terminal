// Session Management - Client-side session state tracking and operations
//
// This module provides session management functionality on the Rust frontend,
// coordinating with the Python backend's SessionManager via IPC messages.

use crate::ipc::{IpcClient, IpcError, Request};
use anyhow::Result;
use chrono::{DateTime, Utc};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Request ID space for SessionManager - starts at 10000 to avoid collision with interactive IDs (0-9999)
const SESSION_MANAGER_ID_MIN: u64 = 10000;
const SESSION_MANAGER_ID_MAX: u64 = u64::MAX;

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
    ipc_client: Arc<Mutex<IpcClient>>,
    current_session_id: Option<String>,
    sessions_cache: HashMap<String, SessionMetadata>,
    request_counter: u64,
}

impl SessionManager {
    /// Create a new session manager with IPC client
    pub fn new(ipc_client: Arc<Mutex<IpcClient>>) -> Self {
        info!("📝 Session manager created with IPC client");
        Self {
            ipc_client,
            current_session_id: None,
            sessions_cache: HashMap::new(),
            request_counter: SESSION_MANAGER_ID_MIN - 1, // Start at 9999 so first ID is 10000
        }
    }

    /// Get next request ID for IPC calls (SessionManager uses IDs >= 10000)
    fn next_request_id(&mut self) -> u64 {
        self.request_counter += 1;
        // Validate we're in the correct ID space
        if self.request_counter < SESSION_MANAGER_ID_MIN {
            warn!("⚠️  SessionManager ID counter corrupted, resetting to {}", SESSION_MANAGER_ID_MIN);
            self.request_counter = SESSION_MANAGER_ID_MIN;
        }
        if self.request_counter == SESSION_MANAGER_ID_MAX {
            warn!("⚠️  SessionManager ID counter at maximum, wrapping to {}", SESSION_MANAGER_ID_MIN);
            self.request_counter = SESSION_MANAGER_ID_MIN;
        }
        self.request_counter
    }

    /// List all sessions from the backend
    pub async fn list_sessions(&mut self, limit: Option<usize>) -> Result<Vec<SessionMetadata>, IpcError> {
        debug!("📋 Listing sessions (limit: {:?})", limit);

        let request_id = self.next_request_id();

        let params = if let Some(limit) = limit {
            serde_json::json!({ "limit": limit })
        } else {
            serde_json::json!({})
        };

        let request = Request::new(request_id, "session.list", Some(params));
        let response = {
            let mut client = self.ipc_client.lock().await;
            client.send_request(request).await?
        };

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

        let params = serde_json::json!({ "session_id": session_id });
        let request = Request::new(request_id, "session.load", Some(params));
        let response = {
            let mut client = self.ipc_client.lock().await;
            client.send_request(request).await?
        };

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

        let params = if let Some(id) = session_id {
            serde_json::json!({
                "session_id": id,
                "format": format
            })
        } else {
            serde_json::json!({ "format": format })
        };

        let request = Request::new(request_id, "session.export", Some(params));
        let response = {
            let mut client = self.ipc_client.lock().await;
            client.send_request(request).await?
        };

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

        let params = serde_json::json!({ "session_id": session_id });
        let request = Request::new(request_id, "session.delete", Some(params));
        let response = {
            let mut client = self.ipc_client.lock().await;
            client.send_request(request).await?
        };

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
    #[allow(dead_code)]  // May be useful for future cache management
    pub fn clear_cache(&mut self) {
        self.sessions_cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: SessionManager tests require a mock IpcClient
    // These tests are disabled until we implement a mock

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

    // Disabled: requires IpcClient
    // #[test]
    // fn test_clear_cache() { ... }

    // Disabled: requires IpcClient
    // #[test]
    // fn test_get_cached_metadata() { ... }
}
