//! Session Service
//!
//! Provides the service layer for session management, integrating session persistence
//! with the terminal, conversation management, and block systems. Handles session
//! lifecycle, automatic saving, restoration, and event coordination.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use tokio::sync::{broadcast, RwLock};
use tokio::time::interval;
use tracing::{info, debug, error};
use serde::Serialize;

use crate::session_persistence::{
    SessionManager, SessionId, SessionState, PersistedCommand, PersistedConversation,
    UserPreferences, WorkspaceState, PersistenceConfig
};
use crate::blocks_v2::{BlockRecord, BlockId};
use crate::conversation_management::{
    ConversationManager, ConversationId, ConversationMessage
};
use crate::ai_context_provider::PtyAiContext;

/// Events that can be emitted by the session service
#[derive(Debug, Clone, Serialize)]
pub enum SessionEvent {
    /// New session started
    SessionStarted {
        session_id: SessionId,
        created_at: DateTime<Utc>,
    },
    
    /// Session restored from storage
    SessionRestored {
        session_id: SessionId,
        restored_at: DateTime<Utc>,
    },
    
    /// Session saved
    SessionSaved {
        session_id: SessionId,
        saved_at: DateTime<Utc>,
    },
    
    /// Session deleted
    SessionDeleted {
        session_id: SessionId,
        deleted_at: DateTime<Utc>,
    },
    
    /// Command added to session
    CommandAdded {
        session_id: SessionId,
        command_id: Option<BlockId>,
        command: String,
    },
    
    /// Conversation updated in session
    ConversationUpdated {
        session_id: SessionId,
        conversation_id: ConversationId,
    },
    
    /// User preferences updated
    PreferencesUpdated {
        session_id: SessionId,
    },
    
    /// Workspace state updated
    WorkspaceUpdated {
        session_id: SessionId,
    },
}

/// Session restoration options
#[derive(Debug, Clone)]
pub struct RestoreOptions {
    /// Whether to restore command history
    pub restore_commands: bool,
    
    /// Whether to restore conversations
    pub restore_conversations: bool,
    
    /// Whether to restore user preferences
    pub restore_preferences: bool,
    
    /// Whether to restore workspace state
    pub restore_workspace: bool,
    
    /// Whether to restore environment
    pub restore_environment: bool,
    
    /// Maximum age of data to restore
    pub max_restore_age: Option<Duration>,
}

impl Default for RestoreOptions {
    fn default() -> Self {
        Self {
            restore_commands: true,
            restore_conversations: true,
            restore_preferences: true,
            restore_workspace: true,
            restore_environment: false, // Don't restore env by default for security
            max_restore_age: Some(Duration::from_secs(86400 * 30)), // 30 days
        }
    }
}

/// Session service for managing terminal sessions
pub struct SessionService {
    session_manager: Arc<SessionManager>,
    conversation_manager: Arc<RwLock<Option<ConversationManager>>>,
    event_sender: broadcast::Sender<SessionEvent>,
    _event_receiver: broadcast::Receiver<SessionEvent>,
    auto_save_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
}

impl SessionService {
    /// Create a new session service
    pub async fn new(config: PersistenceConfig) -> Result<Self> {
        let session_manager = Arc::new(SessionManager::new(config.clone()).await?);
        let (event_sender, event_receiver) = broadcast::channel(1000);
        
        let service = Self {
            session_manager,
            conversation_manager: Arc::new(RwLock::new(None)),
            event_sender,
            _event_receiver: event_receiver,
            auto_save_handle: RwLock::new(None),
        };
        
        // Start auto-save background task
        service.start_auto_save(config.auto_save_interval).await;
        
        Ok(service)
    }
    
    /// Set the conversation manager for integration
    pub async fn set_conversation_manager(&self, manager: ConversationManager) {
        let mut conv_manager = self.conversation_manager.write().await;
        *conv_manager = Some(manager);
    }
    
    /// Subscribe to session events
    pub fn subscribe_events(&self) -> broadcast::Receiver<SessionEvent> {
        self.event_sender.subscribe()
    }
    
    /// Start a new session
    pub async fn start_new_session(&self, context: &PtyAiContext) -> Result<SessionId> {
        let session_id = self.session_manager.start_session(context).await?;
        
        // Emit event
        let _ = self.event_sender.send(SessionEvent::SessionStarted {
            session_id,
            created_at: Utc::now(),
        });
        
        info!("Started new session: {}", session_id);
        Ok(session_id)
    }
    
    /// Restore a session with specific options
    pub async fn restore_session(
        &self,
        session_id: SessionId,
        options: RestoreOptions,
    ) -> Result<RestorationSummary> {
        // Load session from storage
        self.session_manager.restore_session(session_id).await?;
        
        let session = self.session_manager.get_current_session().await
            .ok_or_else(|| anyhow::anyhow!("Failed to load restored session"))?;
        
        let mut summary = RestorationSummary {
            session_id,
            commands_restored: 0,
            conversations_restored: 0,
            preferences_restored: false,
            workspace_restored: false,
            environment_restored: false,
        };
        
        // Apply restoration options
        if let Some(max_age) = options.max_restore_age {
            let cutoff_time = Utc::now() - chrono::Duration::from_std(max_age)?;
            
            // Filter old commands if restoring commands
            if options.restore_commands {
                let valid_commands: Vec<_> = session.command_history.iter()
                    .filter(|cmd| cmd.executed_at > cutoff_time)
                    .cloned()
                    .collect();
                summary.commands_restored = valid_commands.len();
                
                // Update session with filtered commands
                self.session_manager.update_session(|session| {
                    session.command_history = valid_commands;
                }).await?;
            }
            
            // Filter old conversations if restoring conversations
            if options.restore_conversations {
                let valid_conversations: Vec<_> = session.conversations.iter()
                    .filter(|conv| conv.last_message_at > cutoff_time)
                    .cloned()
                    .collect();
                summary.conversations_restored = valid_conversations.len();
                
                // Restore conversations to conversation manager
                if let Some(ref conv_manager) = *self.conversation_manager.read().await {
                    for conv in &valid_conversations {
                        // Restore each conversation into the active conversation manager
                        let _ = conv_manager.restore_conversation_from_persisted(conv).await?;
                    }
                }
                
                // Update session with filtered conversations
                self.session_manager.update_session(|session| {
                    session.conversations = valid_conversations;
                }).await?;
            }
        } else {
            summary.commands_restored = session.command_history.len();
            summary.conversations_restored = session.conversations.len();
        }
        
        // Restore preferences
        if options.restore_preferences {
            summary.preferences_restored = true;
            // Preferences are already loaded with the session
        }
        
        // Restore workspace
        if options.restore_workspace {
            summary.workspace_restored = true;
            // Workspace is already loaded with the session
        }
        
        // Restore environment (if requested)
        if options.restore_environment {
            // Best-effort restoration of select environment context stored in session preferences
            // We will not mutate global process env for safety; instead, we emit a summary that
            // callers can use to rehydrate their environment-aware components.
            // Re-apply the captured environment snapshot into the current session state
            let env = session.environment.clone();
            self.session_manager.update_session(|s| {
                s.environment = env.clone();
            }).await?;
            summary.environment_restored = true;
            debug!("Environment snapshot reapplied to current session");
        }
        
        // Emit event
        let _ = self.event_sender.send(SessionEvent::SessionRestored {
            session_id,
            restored_at: Utc::now(),
        });
        
        info!("Restored session {} with summary: {:?}", session_id, summary);
        Ok(summary)
    }
    
    /// Get the current session state
    pub async fn get_current_session(&self) -> Option<SessionState> {
        self.session_manager.get_current_session().await
    }
    
    /// Add a command to the current session
    pub async fn add_command(
        &self,
        block_record: &BlockRecord,
        execution_result: &CommandExecutionResult,
    ) -> Result<()> {
        let command = PersistedCommand {
            id: Some(block_record.id),
            command: block_record.command.clone(),
            output: execution_result.output.clone(),
            error_output: execution_result.error_output.clone(),
            exit_code: execution_result.exit_code,
            duration_ms: execution_result.duration.as_millis() as u64,
            executed_at: block_record.created_at,
working_directory: block_record.directory.clone(),
            shell: block_record.shell.clone(),
            tags: block_record.tags.clone(),
        };
        
        self.session_manager.add_command(command).await?;
        
        // Emit event
        if let Some(session) = self.get_current_session().await {
            let _ = self.event_sender.send(SessionEvent::CommandAdded {
                session_id: session.session_id,
                command_id: Some(block_record.id),
                command: block_record.command.clone(),
            });
        }
        
        Ok(())
    }
    
    /// Update conversation in session
    pub async fn update_conversation(
        &self,
        conversation_id: ConversationId,
        title: String,
        messages: Vec<ConversationMessage>,
    ) -> Result<()> {
        let conversation = PersistedConversation {
            conversation_id,
            title,
            created_at: Utc::now(), // This should come from the actual conversation
            last_message_at: messages
                .last()
                .map(|m| chrono::DateTime::<Utc>::from(m.timestamp))
                .unwrap_or_else(Utc::now),
            message_count: messages.len(),
            recent_messages: messages.into_iter().take(10).collect(), // Keep only recent messages
            context_summary: "Conversation summary".to_string(), // Would be generated
        };
        
        self.session_manager.add_conversation(conversation).await?;
        
        // Emit event
        if let Some(session) = self.get_current_session().await {
            let _ = self.event_sender.send(SessionEvent::ConversationUpdated {
                session_id: session.session_id,
                conversation_id,
            });
        }
        
        Ok(())
    }
    
    /// Update user preferences
    pub async fn update_preferences<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut UserPreferences) + Send + 'static,
    {
        self.session_manager.update_preferences(updater).await?;
        
        // Emit event
        if let Some(session) = self.get_current_session().await {
            let _ = self.event_sender.send(SessionEvent::PreferencesUpdated {
                session_id: session.session_id,
            });
        }
        
        Ok(())
    }
    
    /// Update workspace state
    pub async fn update_workspace<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut WorkspaceState) + Send + 'static,
    {
        self.session_manager.update_workspace(updater).await?;
        
        // Emit event
        if let Some(session) = self.get_current_session().await {
            let _ = self.event_sender.send(SessionEvent::WorkspaceUpdated {
                session_id: session.session_id,
            });
        }
        
        Ok(())
    }
    
    /// Save current session manually
    pub async fn save_current_session(&self) -> Result<()> {
        self.session_manager.save_current_session().await?;
        
        // Emit event
        if let Some(session) = self.get_current_session().await {
            let _ = self.event_sender.send(SessionEvent::SessionSaved {
                session_id: session.session_id,
                saved_at: Utc::now(),
            });
        }
        
        Ok(())
    }
    
    /// List all available sessions
    pub async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        self.session_manager.list_sessions().await
    }
    
    /// Delete a session
    pub async fn delete_session(&self, session_id: SessionId) -> Result<()> {
        self.session_manager.delete_session(session_id).await?;
        
        // Emit event
        let _ = self.event_sender.send(SessionEvent::SessionDeleted {
            session_id,
            deleted_at: Utc::now(),
        });
        
        Ok(())
    }
    
    /// Clean up old sessions
    pub async fn cleanup_old_sessions(&self) -> Result<usize> {
        self.session_manager.cleanup_old_sessions().await
    }
    
    /// Export session for backup or sharing
    pub async fn export_session(
        &self,
        session_id: SessionId,
        path: &std::path::Path,
    ) -> Result<()> {
        self.session_manager.export_session(session_id, path).await
    }
    
    /// Import session from file
    pub async fn import_session(&self, path: &std::path::Path) -> Result<SessionId> {
        let session_id = self.session_manager.import_session(path).await?;
        
        // Emit event
        let _ = self.event_sender.send(SessionEvent::SessionStarted {
            session_id,
            created_at: Utc::now(),
        });
        
        Ok(session_id)
    }
    
    /// Get session statistics
    pub async fn get_session_stats(&self) -> Result<SessionStats> {
        let sessions = self.list_sessions().await?;
        let current_session = self.get_current_session().await;
        
        let total_commands = current_session.as_ref()
            .map(|s| s.command_history.len())
            .unwrap_or(0);
        
        let total_conversations = current_session.as_ref()
            .map(|s| s.conversations.len())
            .unwrap_or(0);
        
        let session_duration = current_session.as_ref()
            .map(|s| s.ai_config.usage_stats.session_duration)
            .unwrap_or(Duration::ZERO);
        
        Ok(SessionStats {
            total_sessions: sessions.len(),
            active_session: current_session.map(|s| s.session_id),
            total_commands,
            total_conversations,
            session_duration,
            oldest_session: sessions.iter()
                .min_by_key(|s| s.created_at)
                .map(|s| s.created_at),
            newest_session: sessions.iter()
                .max_by_key(|s| s.created_at)
                .map(|s| s.created_at),
        })
    }
    
    // Private methods
    
    /// Start the auto-save background task
    async fn start_auto_save(&self, interval_duration: Duration) {
        let session_manager = Arc::clone(&self.session_manager);
        let event_sender = self.event_sender.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(interval_duration);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = session_manager.save_current_session().await {
                    error!("Auto-save failed: {}", e);
                } else {
                    debug!("Auto-save completed");
                    
                    // Emit save event
                    if let Some(session) = session_manager.get_current_session().await {
                        let _ = event_sender.send(SessionEvent::SessionSaved {
                            session_id: session.session_id,
                            saved_at: Utc::now(),
                        });
                    }
                }
            }
        });
        
        let mut auto_save_handle = self.auto_save_handle.write().await;
        *auto_save_handle = Some(handle);
    }
    
    /// Stop the auto-save background task
    pub async fn stop_auto_save(&self) {
        let mut handle_guard = self.auto_save_handle.write().await;
        
        if let Some(handle) = handle_guard.take() {
            handle.abort();
            debug!("Auto-save task stopped");
        }
    }
}

impl Drop for SessionService {
    fn drop(&mut self) {
        // Try to stop auto-save when service is dropped
        if let Ok(handle_guard) = self.auto_save_handle.try_write() {
            if let Some(handle) = &*handle_guard {
                handle.abort();
            }
        }
    }
}

/// Summary of what was restored from a session
#[derive(Debug, Clone, Serialize)]
pub struct RestorationSummary {
    pub session_id: SessionId,
    pub commands_restored: usize,
    pub conversations_restored: usize,
    pub preferences_restored: bool,
    pub workspace_restored: bool,
    pub environment_restored: bool,
}

/// Session statistics
#[derive(Debug, Clone, Serialize)]
pub struct SessionStats {
    pub total_sessions: usize,
    pub active_session: Option<SessionId>,
    pub total_commands: usize,
    pub total_conversations: usize,
    pub session_duration: Duration,
    pub oldest_session: Option<DateTime<Utc>>,
    pub newest_session: Option<DateTime<Utc>>,
}

/// Command execution result for session recording
#[derive(Debug, Clone)]
pub struct CommandExecutionResult {
    pub output: String,
    pub error_output: String,
    pub exit_code: i32,
    pub duration: Duration,
}

/// Re-export session summary from persistence layer
pub use crate::session_persistence::SessionSummary;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::blocks_v2::BlockId;
    
    use crate::blocks_v2::ShellType;

    #[tokio::test]
    async fn test_session_service_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let config = PersistenceConfig {
            session_dir: temp_dir.path().to_path_buf(),
            auto_save_interval: Duration::from_millis(100), // Fast for testing
            ..Default::default()
        };
        
        let service = SessionService::new(config).await.unwrap();
        let context = PtyAiContext::default();
        
        // Start new session
        let session_id = service.start_new_session(&context).await.unwrap();
        
        // Verify session is active
        let session = service.get_current_session().await.unwrap();
        assert_eq!(session.session_id, session_id);
        
        // Add a command
        let block_record = BlockRecord {
            id: BlockId(1),
            command: "echo test".to_string(),
            output: "test".to_string(),
            error_output: String::new(),
            directory: std::path::PathBuf::from("/tmp"),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            exit_code: 0,
            duration_ms: 100,
            starred: false,
            tags: vec!["test".to_string()],
            shell: ShellType::Bash,
            status: "completed".to_string(),
        };
        
        let exec_result = CommandExecutionResult {
            output: "test".to_string(),
            error_output: String::new(),
            exit_code: 0,
            duration: Duration::from_millis(100),
        };
        
        service.add_command(&block_record, &exec_result).await.unwrap();
        
        // Verify command was added
        let updated_session = service.get_current_session().await.unwrap();
        assert_eq!(updated_session.command_history.len(), 1);
        assert_eq!(updated_session.command_history[0].command, "echo test");
        
        // Test session stats
        let stats = service.get_session_stats().await.unwrap();
        assert_eq!(stats.total_sessions, 1);
        assert_eq!(stats.total_commands, 1);
        assert_eq!(stats.active_session, Some(session_id));
    }
    
    #[tokio::test]
    async fn test_session_events() {
        let temp_dir = TempDir::new().unwrap();
        let config = PersistenceConfig {
            session_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let service = SessionService::new(config).await.unwrap();
        let mut event_receiver = service.subscribe_events();
        let context = PtyAiContext::default();
        
        // Start session and check for event
        let session_id = service.start_new_session(&context).await.unwrap();
        
        // Should receive session started event
        let event = event_receiver.recv().await.unwrap();
        match event {
            SessionEvent::SessionStarted { session_id: id, .. } => {
                assert_eq!(id, session_id);
            }
            _ => panic!("Expected SessionStarted event"),
        }
        
        // Save session and check for event
        service.save_current_session().await.unwrap();
        
        let event = event_receiver.recv().await.unwrap();
        match event {
            SessionEvent::SessionSaved { session_id: id, .. } => {
                assert_eq!(id, session_id);
            }
            _ => panic!("Expected SessionSaved event"),
        }
    }

    #[tokio::test]
    async fn test_environment_restoration() {
        let temp_dir = TempDir::new().unwrap();
        let config = PersistenceConfig { session_dir: temp_dir.path().to_path_buf(), ..Default::default() };
        let service = SessionService::new(config).await.unwrap();
        let context = PtyAiContext::default();
        let session_id = service.start_new_session(&context).await.unwrap();

        // Update environment snapshot
        {
            let vars = vec![("FOO".to_string(), "BAR".to_string())].into_iter().collect();
            service.session_manager.update_session(|s| {
                s.environment.variables = vars;
            }).await.unwrap();
            service.save_current_session().await.unwrap();
        }

        // Restore with environment option enabled
        let opts = RestoreOptions { restore_environment: true, ..Default::default() };
        let summary = service.restore_session(session_id, opts).await.unwrap();
        assert!(summary.environment_restored);
        // Verify environment present in current session
        let sess = service.get_current_session().await.unwrap();
        assert_eq!(sess.environment.variables.get("FOO").map(|s| s.as_str()), Some("BAR"));
    }
}
