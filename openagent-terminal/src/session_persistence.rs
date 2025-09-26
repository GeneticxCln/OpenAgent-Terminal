//! Session Persistence System
//!
//! Provides comprehensive session persistence that maintains terminal state,
//! conversation history, command history, user preferences, and workspace state
//! across terminal restarts. Integrates seamlessly with existing block management
//! and conversation systems.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, Duration};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn, debug, error};
use uuid::Uuid;

use crate::blocks_v2::{BlockRecord, BlockId, ShellType};
use crate::conversation_management::{ConversationId, ConversationMessage, MessageType};
use crate::ai_context_provider::{PtyAiContext, TerminalContext, ProjectInfo};

/// Unique identifier for terminal sessions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Complete session state that can be persisted and restored
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Session identification
    pub session_id: SessionId,
    
    /// When this session was created
    pub created_at: DateTime<Utc>,
    
    /// Last time this session was active
    pub last_active: DateTime<Utc>,
    
    /// Terminal state
    pub terminal_state: PersistedTerminalState,
    
    /// Command history
    pub command_history: Vec<PersistedCommand>,
    
    /// Active conversations
    pub conversations: Vec<PersistedConversation>,
    
    /// Current conversation ID
    pub active_conversation: Option<ConversationId>,
    
    /// User preferences
    pub preferences: UserPreferences,
    
    /// Workspace state
    pub workspace: WorkspaceState,
    
    /// Environment snapshot
    pub environment: EnvironmentSnapshot,
    
    /// AI configuration
    pub ai_config: AiConfiguration,
    
    /// Custom session data
    pub custom_data: HashMap<String, serde_json::Value>,
}

/// Terminal state that can be persisted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedTerminalState {
    /// Current working directory
    pub working_directory: PathBuf,
    
    /// Shell type
    pub shell_type: ShellType,
    
    /// Terminal dimensions
    pub dimensions: (u16, u16), // (width, height)
    
    /// Current Git branch if in repository
    pub git_branch: Option<String>,
    
    /// Git status if available
    pub git_status: Option<String>,
    
    /// Project information
    pub project_info: Option<ProjectInfo>,
    
    /// Active terminal tabs/panes
    pub tabs: Vec<TerminalTab>,
    
    /// Current active tab
    pub active_tab: Option<usize>,
}

/// Persisted command with execution details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedCommand {
    pub id: Option<BlockId>,
    pub command: String,
    pub output: String,
    pub error_output: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub executed_at: DateTime<Utc>,
    pub working_directory: PathBuf,
    pub shell: ShellType,
    pub tags: Vec<String>,
}

/// Persisted conversation with essential data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedConversation {
    pub conversation_id: ConversationId,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub last_message_at: DateTime<Utc>,
    pub message_count: usize,
    pub recent_messages: Vec<ConversationMessage>,
    pub context_summary: String,
}

/// Terminal tab/pane information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalTab {
    pub id: Uuid,
    pub title: String,
    pub working_directory: PathBuf,
    pub last_command: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// User preferences and settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    /// Theme and appearance
    pub theme: String,
    pub font_size: f32,
    pub color_scheme: String,
    
    /// AI settings
    pub preferred_ai_provider: String,
    pub ai_auto_suggestions: bool,
    pub ai_context_awareness: bool,
    
    /// Terminal behavior
    pub auto_save_commands: bool,
    pub persist_history: bool,
    pub max_history_items: usize,
    
    /// Notifications
    pub enable_notifications: bool,
    pub notification_types: Vec<String>,
    
    /// Block sharing preferences
    pub auto_sync_blocks: bool,
    pub trusted_repositories: Vec<String>,
    
    /// Custom preferences
    pub custom: HashMap<String, serde_json::Value>,
}

/// Workspace state including open projects and configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceState {
    /// Open projects
    pub projects: Vec<ProjectState>,
    
    /// Current active project
    pub active_project: Option<String>,
    
    /// Workspace-specific settings
    pub settings: HashMap<String, serde_json::Value>,
    
    /// Recent directories
    pub recent_directories: Vec<PathBuf>,
    
    /// Bookmarked directories
    pub bookmarks: HashMap<String, PathBuf>,
}

/// Individual project state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectState {
    pub name: String,
    pub path: PathBuf,
    pub project_type: String,
    pub last_accessed: DateTime<Utc>,
    pub git_branch: Option<String>,
    pub dependencies: Vec<String>,
    pub recent_files: Vec<PathBuf>,
}

/// Environment variable snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSnapshot {
    /// Important environment variables
    pub variables: HashMap<String, String>,
    
    /// PATH components
    pub path_components: Vec<PathBuf>,
    
    /// Shell-specific variables
    pub shell_variables: HashMap<String, String>,
    
    /// When this snapshot was taken
    pub captured_at: DateTime<Utc>,
}

/// AI configuration and state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfiguration {
    /// Enabled AI providers
    pub providers: Vec<String>,
    
    /// Current provider
    pub current_provider: String,
    
    /// Provider configurations
    pub provider_configs: HashMap<String, serde_json::Value>,
    
    /// AI usage statistics
    pub usage_stats: AiUsageStats,
    
    /// Learning preferences
    pub learning_enabled: bool,
    pub learning_data: HashMap<String, serde_json::Value>,
}

/// AI usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiUsageStats {
    pub commands_assisted: u64,
    pub conversations_started: u64,
    pub suggestions_accepted: u64,
    pub errors_analyzed: u64,
    pub session_duration: Duration,
}

/// Session persistence configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    /// Directory to store session data
    pub session_dir: PathBuf,
    
    /// Maximum number of sessions to keep
    pub max_sessions: usize,
    
    /// Auto-save interval
    pub auto_save_interval: Duration,
    
    /// Session cleanup after inactivity
    pub cleanup_after: Duration,
    
    /// Compression settings
    pub compression: CompressionSettings,
    
    /// What to persist
    pub persist_commands: bool,
    pub persist_conversations: bool,
    pub persist_preferences: bool,
    pub persist_workspace: bool,
    
    /// Privacy settings
    pub sanitize_sensitive_data: bool,
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionSettings {
    pub enabled: bool,
    pub algorithm: String,
    pub level: u8,
}

/// Session persistence manager
pub struct SessionManager {
    config: PersistenceConfig,
    current_session: RwLock<Option<SessionState>>,
    session_storage: SessionStorage,
}

/// Storage interface for sessions
pub struct SessionStorage {
    base_path: PathBuf,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            font_size: 14.0,
            color_scheme: "default".to_string(),
            preferred_ai_provider: "ollama".to_string(),
            ai_auto_suggestions: true,
            ai_context_awareness: true,
            auto_save_commands: true,
            persist_history: true,
            max_history_items: 1000,
            enable_notifications: true,
            notification_types: vec!["errors".to_string(), "completions".to_string()],
            auto_sync_blocks: false,
            trusted_repositories: Vec::new(),
            custom: HashMap::new(),
        }
    }
}

impl Default for AiUsageStats {
    fn default() -> Self {
        Self {
            commands_assisted: 0,
            conversations_started: 0,
            suggestions_accepted: 0,
            errors_analyzed: 0,
            session_duration: Duration::ZERO,
        }
    }
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            session_dir: PathBuf::from("~/.openagent/sessions"),
            max_sessions: 10,
            auto_save_interval: Duration::from_secs(300), // 5 minutes
            cleanup_after: Duration::from_secs(86400 * 7), // 7 days
            compression: CompressionSettings {
                enabled: true,
                algorithm: "gzip".to_string(),
                level: 6,
            },
            persist_commands: true,
            persist_conversations: true,
            persist_preferences: true,
            persist_workspace: true,
            sanitize_sensitive_data: true,
            exclude_patterns: vec![
                "password".to_string(),
                "secret".to_string(),
                "token".to_string(),
                "key".to_string(),
            ],
        }
    }
}

impl SessionManager {
    /// Create a new session manager
    pub async fn new(config: PersistenceConfig) -> Result<Self> {
        let session_storage = SessionStorage::new(&config.session_dir).await?;
        
        Ok(Self {
            config,
            current_session: RwLock::new(None),
            session_storage,
        })
    }
    
    /// Start a new session
    pub async fn start_session(&self, context: &PtyAiContext) -> Result<SessionId> {
        let session_id = SessionId::new();
        let now = Utc::now();
        
        let session_state = SessionState {
            session_id,
            created_at: now,
            last_active: now,
            terminal_state: PersistedTerminalState {
                working_directory: context.terminal_context.working_directory.clone(),
                shell_type: ShellType::Bash, // Would be determined from context
                dimensions: (80, 24), // Default dimensions
                git_branch: context.terminal_context.git_branch.clone(),
                git_status: context.terminal_context.git_status.clone(),
                project_info: context.terminal_context.project_info.clone(),
                tabs: vec![TerminalTab {
                    id: Uuid::new_v4(),
                    title: "Main".to_string(),
                    working_directory: context.terminal_context.working_directory.clone(),
                    last_command: context.terminal_context.recent_commands.first().cloned(),
                    created_at: now,
                }],
                active_tab: Some(0),
            },
            command_history: Vec::new(),
            conversations: Vec::new(),
            active_conversation: None,
            preferences: UserPreferences::default(),
            workspace: WorkspaceState {
                projects: Vec::new(),
                active_project: None,
                settings: HashMap::new(),
                recent_directories: vec![context.terminal_context.working_directory.clone()],
                bookmarks: HashMap::new(),
            },
            environment: EnvironmentSnapshot {
                variables: HashMap::new(), // Would be populated from actual environment
                path_components: Vec::new(),
                shell_variables: HashMap::new(),
                captured_at: now,
            },
            ai_config: AiConfiguration {
                providers: vec!["ollama".to_string()],
                current_provider: "ollama".to_string(),
                provider_configs: HashMap::new(),
                usage_stats: AiUsageStats::default(),
                learning_enabled: true,
                learning_data: HashMap::new(),
            },
            custom_data: HashMap::new(),
        };
        
        // Set as current session
        {
            let mut current = self.current_session.write().await;
            *current = Some(session_state.clone());
        }
        
        // Save to storage
        self.session_storage.save_session(&session_state).await?;
        
        info!("Started new session: {}", session_id);
        Ok(session_id)
    }
    
    /// Restore a session from storage
    pub async fn restore_session(&self, session_id: SessionId) -> Result<()> {
        let session_state = self.session_storage.load_session(session_id).await?;
        
        // Update last active time
        let mut updated_session = session_state;
        updated_session.last_active = Utc::now();
        
        // Set as current session
        {
            let mut current = self.current_session.write().await;
            *current = Some(updated_session.clone());
        }
        
        // Save updated state
        self.session_storage.save_session(&updated_session).await?;
        
        info!("Restored session: {}", session_id);
        Ok(())
    }
    
    /// Get the current session
    pub async fn get_current_session(&self) -> Option<SessionState> {
        let current = self.current_session.read().await;
        current.clone()
    }
    
    /// Update current session state
    pub async fn update_session<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut SessionState),
    {
        let mut current = self.current_session.write().await;
        
        if let Some(ref mut session) = *current {
            updater(session);
            session.last_active = Utc::now();
            
            // Save to storage
            self.session_storage.save_session(session).await?;
        }
        
        Ok(())
    }
    
    /// Add a command to session history
    pub async fn add_command(&self, command: PersistedCommand) -> Result<()> {
        self.update_session(|session| {
            session.command_history.push(command);
            
            // Keep only the most recent commands
            let max_commands = session.preferences.max_history_items;
            if session.command_history.len() > max_commands {
                session.command_history.drain(0..session.command_history.len() - max_commands);
            }
            
            // Update AI usage stats
            session.ai_config.usage_stats.commands_assisted += 1;
        }).await
    }
    
    /// Add a conversation to session
    pub async fn add_conversation(&self, conversation: PersistedConversation) -> Result<()> {
        self.update_session(|session| {
            // Remove existing conversation with same ID
            session.conversations.retain(|c| c.conversation_id != conversation.conversation_id);
            
            // Add updated conversation
            session.conversations.push(conversation);
            
            // Update AI usage stats
            session.ai_config.usage_stats.conversations_started += 1;
        }).await
    }
    
    /// Update user preferences
    pub async fn update_preferences<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut UserPreferences),
    {
        self.update_session(|session| {
            updater(&mut session.preferences);
        }).await
    }
    
    /// Update workspace state
    pub async fn update_workspace<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut WorkspaceState),
    {
        self.update_session(|session| {
            updater(&mut session.workspace);
        }).await
    }
    
    /// Save current session
    pub async fn save_current_session(&self) -> Result<()> {
        let current = self.current_session.read().await;
        
        if let Some(ref session) = *current {
            self.session_storage.save_session(session).await?;
            debug!("Saved current session: {}", session.session_id);
        }
        
        Ok(())
    }
    
    /// List available sessions
    pub async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        self.session_storage.list_sessions().await
    }
    
    /// Delete a session
    pub async fn delete_session(&self, session_id: SessionId) -> Result<()> {
        self.session_storage.delete_session(session_id).await?;
        
        // Clear current session if it's the one being deleted
        {
            let mut current = self.current_session.write().await;
            if let Some(ref session) = *current {
                if session.session_id == session_id {
                    *current = None;
                }
            }
        }
        
        info!("Deleted session: {}", session_id);
        Ok(())
    }
    
    /// Clean up old sessions
    pub async fn cleanup_old_sessions(&self) -> Result<usize> {
        let cutoff_time = Utc::now() - chrono::Duration::from_std(self.config.cleanup_after)?;
        let sessions = self.list_sessions().await?;
        
        let mut deleted_count = 0;
        
        for summary in sessions {
            if summary.last_active < cutoff_time {
                self.delete_session(summary.session_id).await?;
                deleted_count += 1;
            }
        }
        
        info!("Cleaned up {} old sessions", deleted_count);
        Ok(deleted_count)
    }
    
    /// Export session for sharing or backup
    pub async fn export_session(&self, session_id: SessionId, path: &Path) -> Result<()> {
        let session = self.session_storage.load_session(session_id).await?;
        
        // Sanitize sensitive data if configured
        let sanitized_session = if self.config.sanitize_sensitive_data {
            self.sanitize_session_data(session)
        } else {
            session
        };
        
        let json_data = serde_json::to_string_pretty(&sanitized_session)?;
        tokio::fs::write(path, json_data).await?;
        
        info!("Exported session {} to {}", session_id, path.display());
        Ok(())
    }
    
    /// Import session from file
    pub async fn import_session(&self, path: &Path) -> Result<SessionId> {
        let json_data = tokio::fs::read_to_string(path).await?;
        let mut session: SessionState = serde_json::from_str(&json_data)?;
        
        // Generate new session ID to avoid conflicts
        session.session_id = SessionId::new();
        session.created_at = Utc::now();
        session.last_active = Utc::now();
        
        // Save imported session
        self.session_storage.save_session(&session).await?;
        
        info!("Imported session {} from {}", session.session_id, path.display());
        Ok(session.session_id)
    }
    
    // Private helper methods
    
    fn sanitize_session_data(&self, mut session: SessionState) -> SessionState {
        // Remove sensitive environment variables
        session.environment.variables.retain(|key, _| {
            !self.config.exclude_patterns.iter().any(|pattern| {
                key.to_lowercase().contains(&pattern.to_lowercase())
            })
        });
        
        // Sanitize command history
        for command in &mut session.command_history {
            for pattern in &self.config.exclude_patterns {
                let pattern_lower = pattern.to_lowercase();
                if command.command.to_lowercase().contains(&pattern_lower) {
                    command.command = "[REDACTED SENSITIVE COMMAND]".to_string();
                    command.output = "[REDACTED]".to_string();
                }
            }
        }
        
        session
    }
}

/// Summary information about a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub command_count: usize,
    pub conversation_count: usize,
    pub working_directory: PathBuf,
}

impl SessionStorage {
    async fn new(base_path: &Path) -> Result<Self> {
        // Expand leading ~ manually to the user's home directory
        let mut path_str = base_path.to_string_lossy().to_string();
        if path_str.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                if path_str == "~" {
                    path_str = home.display().to_string();
                } else if path_str.starts_with("~/") {
                    path_str = home.join(&path_str[2..]).display().to_string();
                }
            }
        }
        let base_path = PathBuf::from(path_str);
        
        // Create storage directory
        tokio::fs::create_dir_all(&base_path).await?;
        
        Ok(Self { base_path })
    }
    
    async fn save_session(&self, session: &SessionState) -> Result<()> {
        let session_file = self.base_path.join(format!("{}.json", session.session_id));
        let json_data = serde_json::to_string_pretty(session)?;
        
        tokio::fs::write(&session_file, json_data).await?;
        Ok(())
    }
    
    async fn load_session(&self, session_id: SessionId) -> Result<SessionState> {
        let session_file = self.base_path.join(format!("{}.json", session_id));
        let json_data = tokio::fs::read_to_string(&session_file).await?;
        let session: SessionState = serde_json::from_str(&json_data)?;
        
        Ok(session)
    }
    
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        let mut summaries = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.base_path).await?;
        
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Some(ext) = entry.path().extension() {
                if ext == "json" {
                    if let Ok(session) = self.load_session_summary(&entry.path()).await {
                        summaries.push(session);
                    }
                }
            }
        }
        
        // Sort by last active time (most recent first)
        summaries.sort_by(|a, b| b.last_active.cmp(&a.last_active));
        
        Ok(summaries)
    }
    
    async fn delete_session(&self, session_id: SessionId) -> Result<()> {
        let session_file = self.base_path.join(format!("{}.json", session_id));
        
        if session_file.exists() {
            tokio::fs::remove_file(&session_file).await?;
        }
        
        Ok(())
    }
    
    async fn load_session_summary(&self, path: &Path) -> Result<SessionSummary> {
        let json_data = tokio::fs::read_to_string(path).await?;
        let session: SessionState = serde_json::from_str(&json_data)?;
        
        Ok(SessionSummary {
            session_id: session.session_id,
            title: format!("Session {}", session.session_id.0.to_string()[..8].to_string()),
            created_at: session.created_at,
            last_active: session.last_active,
            command_count: session.command_history.len(),
            conversation_count: session.conversations.len(),
            working_directory: session.terminal_state.working_directory,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_session_creation_and_restoration() {
        let temp_dir = TempDir::new().unwrap();
        let config = PersistenceConfig {
            session_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let manager = SessionManager::new(config).await.unwrap();
        
        // Create test context
        let context = PtyAiContext::default();
        
        // Start new session
        let session_id = manager.start_session(&context).await.unwrap();
        
        // Verify session was created
        let current = manager.get_current_session().await.unwrap();
        assert_eq!(current.session_id, session_id);
        
        // Clear current session
        {
            let mut current = manager.current_session.write().await;
            *current = None;
        }
        
        // Restore session
        manager.restore_session(session_id).await.unwrap();
        
        // Verify session was restored
        let restored = manager.get_current_session().await.unwrap();
        assert_eq!(restored.session_id, session_id);
    }
    
    #[tokio::test]
    async fn test_session_export_import() {
        let temp_dir = TempDir::new().unwrap();
        let config = PersistenceConfig {
            session_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let manager = SessionManager::new(config).await.unwrap();
        let context = PtyAiContext::default();
        
        // Create and populate session
        let session_id = manager.start_session(&context).await.unwrap();
        
        // Add some data
        manager.add_command(PersistedCommand {
            id: None,
            command: "echo test".to_string(),
            output: "test".to_string(),
            error_output: String::new(),
            exit_code: 0,
            duration_ms: 100,
            executed_at: Utc::now(),
            working_directory: PathBuf::from("/tmp"),
            shell: ShellType::Bash,
            tags: vec!["test".to_string()],
        }).await.unwrap();
        
        // Export session
        let export_path = temp_dir.path().join("exported_session.json");
        manager.export_session(session_id, &export_path).await.unwrap();
        
        assert!(export_path.exists());
        
        // Import session
        let imported_id = manager.import_session(&export_path).await.unwrap();
        assert_ne!(imported_id, session_id); // Should have new ID
        
        // Verify imported session has the data
        let imported_session = manager.session_storage.load_session(imported_id).await.unwrap();
        assert_eq!(imported_session.command_history.len(), 1);
        assert_eq!(imported_session.command_history[0].command, "echo test");
    }
}