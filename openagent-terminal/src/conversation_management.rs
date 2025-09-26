//! Conversation Management System
//!
//! This module provides comprehensive conversation management including conversation
//! history, context preservation, multi-turn interactions, conversation branching,
//! and seamless integration with terminal workflow. It maintains context across
//! interactions and enables sophisticated AI assistance workflows.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock, Mutex};
use tracing::{debug, info, warn, error};
use uuid::Uuid;

use crate::ai_runtime::{AiRuntime, AiProvider, AgentRequest, AgentResponse};
use crate::ai_context_provider::PtyAiContext;
use crate::command_assistance::{AssistanceType, CommandAssistanceEngine};

/// Unique identifier for conversations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConversationId(pub Uuid);

impl ConversationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl std::fmt::Display for ConversationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Types of conversation messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    /// User input or command
    User,
    /// AI assistant response
    Assistant,
    /// System message (context updates, notifications)
    System,
    /// Command execution result
    CommandResult,
    /// Error message
    Error,
    /// Suggestion or recommendation
    Suggestion,
}

/// A single message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub id: Uuid,
    pub message_type: MessageType,
    pub content: String,
    pub timestamp: SystemTime,
    pub metadata: HashMap<String, String>,
    /// Optional attachment data (command output, error details, etc.)
    pub attachments: Vec<MessageAttachment>,
    /// References to other messages this responds to
    pub references: Vec<Uuid>,
}

/// Attachment data for messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAttachment {
    pub attachment_type: AttachmentType,
    pub content: String,
    pub filename: Option<String>,
    pub size: usize,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttachmentType {
    CommandOutput,
    ErrorLog,
    FileContent,
    TerminalState,
    ContextSnapshot,
    AssistanceData,
}

/// Conversation context that persists across messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationContext {
    /// Working directory when conversation started
    pub initial_working_directory: PathBuf,
    
    /// Current working directory
    pub current_working_directory: PathBuf,
    
    /// Shell type
    pub shell_type: crate::blocks_v2::ShellType,
    
    /// Project information
    pub project_info: Option<crate::ai_context_provider::ProjectInfo>,
    
    /// Git repository state
    pub git_context: Option<GitContext>,
    
    /// Environment variables relevant to the conversation
    pub environment_variables: HashMap<String, String>,
    
    /// Commands executed during this conversation
    pub command_history: Vec<CommandExecution>,
    
    /// Active terminals or sessions
    pub active_sessions: Vec<String>,
    
    /// Custom context data
    pub custom_data: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitContext {
    pub repository_root: PathBuf,
    pub current_branch: String,
    pub status: String,
    pub recent_commits: Vec<String>,
    pub remote_info: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExecution {
    pub command: String,
    pub timestamp: SystemTime,
    pub exit_code: i32,
    pub output: String,
    pub error_output: String,
    pub working_directory: PathBuf,
    pub duration: Duration,
}

/// A complete conversation with history and context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: ConversationId,
    pub title: String,
    pub description: Option<String>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub messages: Vec<ConversationMessage>,
    pub context: ConversationContext,
    pub tags: Vec<String>,
    pub is_active: bool,
    pub parent_conversation: Option<ConversationId>,
    pub child_conversations: Vec<ConversationId>,
    pub settings: ConversationSettings,
}

/// Settings for conversation behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSettings {
    /// Maximum number of messages to keep in memory
    pub max_messages: usize,
    
    /// Auto-save interval
    pub auto_save_interval: Duration,
    
    /// Whether to persist to disk
    pub persist_to_disk: bool,
    
    /// Context preservation strategy
    pub context_strategy: ContextStrategy,
    
    /// AI provider preferences for this conversation
    pub preferred_providers: Vec<AiProvider>,
    
    /// Enable context compression for long conversations
    pub enable_compression: bool,
    
    /// Automatic cleanup after inactivity
    pub auto_cleanup_after: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextStrategy {
    /// Keep all context (memory intensive)
    Full,
    /// Keep recent messages and summarize older ones
    Summarized { keep_recent: usize },
    /// Keep only essential context
    Minimal,
    /// Custom strategy with specific rules
    Custom { rules: Vec<String> },
}

impl Default for ConversationSettings {
    fn default() -> Self {
        Self {
            max_messages: 1000,
            auto_save_interval: Duration::from_secs(300), // 5 minutes
            persist_to_disk: true,
            context_strategy: ContextStrategy::Summarized { keep_recent: 50 },
            preferred_providers: vec![AiProvider::Ollama],
            enable_compression: true,
            auto_cleanup_after: Some(Duration::from_secs(86400 * 7)), // 7 days
        }
    }
}

/// Conversation branch for handling multiple conversation paths
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationBranch {
    pub id: Uuid,
    pub parent_message_id: Uuid,
    pub branch_point: usize, // Message index where branch starts
    pub title: String,
    pub description: Option<String>,
    pub created_at: SystemTime,
    pub messages: Vec<ConversationMessage>,
}

/// Query parameters for conversation search and filtering
#[derive(Debug, Default)]
pub struct ConversationQuery {
    pub text_search: Option<String>,
    pub tags: Vec<String>,
    pub date_range: Option<(SystemTime, SystemTime)>,
    pub working_directory: Option<PathBuf>,
    pub has_errors: Option<bool>,
    pub message_type: Option<MessageType>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Configuration for the conversation management system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationConfig {
    /// Maximum number of active conversations
    pub max_active_conversations: usize,
    
    /// Default settings for new conversations
    pub default_settings: ConversationSettings,
    
    /// Persistence configuration
    pub persistence: PersistenceConfig,
    
    /// Context analysis settings
    pub context_analysis: ContextAnalysisConfig,
    
    /// Integration settings
    pub integration: IntegrationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    /// Directory to store conversation data
    pub storage_directory: PathBuf,
    
    /// Database file for conversation metadata
    pub database_file: PathBuf,
    
    /// Compression settings
    pub compression: CompressionConfig,
    
    /// Backup settings
    pub backup: BackupConfig,

    /// Auto-restore conversations from disk when the manager starts
    pub auto_restore_on_start: bool,

    /// Maximum number of conversations to restore at startup (0 = no limit)
    pub max_restore_count: usize,

    /// Only restore conversations updated within this many days (None = no limit)
    pub max_restore_age_days: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub enabled: bool,
    pub algorithm: String, // "gzip", "zstd", etc.
    pub level: u8,
    pub threshold_size: usize, // Compress if larger than this
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub enabled: bool,
    pub interval: Duration,
    pub retention_count: usize,
    pub backup_directory: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAnalysisConfig {
    /// Enable automatic context extraction
    pub auto_extract_context: bool,
    
    /// Context summarization settings
    pub summarization: SummarizationConfig,
    
    /// Relevance scoring settings
    pub relevance_scoring: RelevanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizationConfig {
    pub enabled: bool,
    pub trigger_message_count: usize,
    pub summary_length: usize,
    pub preserve_commands: bool,
    pub preserve_errors: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelevanceConfig {
    pub enabled: bool,
    pub time_decay_factor: f32,
    pub command_relevance_weight: f32,
    pub error_relevance_weight: f32,
    pub context_similarity_weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    /// Integration with command assistance
    pub command_assistance: bool,
    
    /// Integration with AI event system
    pub ai_events: bool,
    
    /// Integration with terminal state
    pub terminal_state: bool,
    
    /// External tool integrations
    pub external_tools: HashMap<String, serde_json::Value>,
}

impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
            max_active_conversations: 10,
            default_settings: ConversationSettings::default(),
            persistence: PersistenceConfig {
                storage_directory: PathBuf::from("~/.openagent/conversations"),
                database_file: PathBuf::from("~/.openagent/conversations.db"),
                compression: CompressionConfig {
                    enabled: true,
                    algorithm: "gzip".to_string(),
                    level: 6,
                    threshold_size: 1024,
                },
                backup: BackupConfig {
                    enabled: true,
                    interval: Duration::from_secs(86400), // Daily
                    retention_count: 7,
                    backup_directory: PathBuf::from("~/.openagent/backups"),
                },
                auto_restore_on_start: false,
                max_restore_count: 0,
                max_restore_age_days: None,
            },
            context_analysis: ContextAnalysisConfig {
                auto_extract_context: true,
                summarization: SummarizationConfig {
                    enabled: true,
                    trigger_message_count: 100,
                    summary_length: 500,
                    preserve_commands: true,
                    preserve_errors: true,
                },
                relevance_scoring: RelevanceConfig {
                    enabled: true,
                    time_decay_factor: 0.9,
                    command_relevance_weight: 1.2,
                    error_relevance_weight: 1.5,
                    context_similarity_weight: 1.0,
                },
            },
            integration: IntegrationConfig {
                command_assistance: true,
                ai_events: true,
                terminal_state: true,
                external_tools: HashMap::new(),
            },
        }
    }
}

/// Main conversation management system
pub struct ConversationManager {
    /// Configuration
    config: Arc<RwLock<ConversationConfig>>,
    
    /// Active conversations
    active_conversations: Arc<RwLock<HashMap<ConversationId, Conversation>>>,
    
    /// Current active conversation
    current_conversation: Arc<RwLock<Option<ConversationId>>>,
    
    /// AI runtime for processing
    ai_runtime: Arc<RwLock<AiRuntime>>,
    
    /// Command assistance integration
    command_assistance: Arc<RwLock<CommandAssistanceEngine>>,
    
    /// Message processing queue
    message_queue: Arc<Mutex<VecDeque<PendingMessage>>>,
    
    /// Event broadcasting
    event_sender: mpsc::UnboundedSender<ConversationEvent>,
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<ConversationEvent>>>,
    
    /// Background task handles
    task_handles: Vec<tokio::task::JoinHandle<()>>,
    
    /// Performance statistics
    stats: Arc<RwLock<ConversationStats>>,
}

#[derive(Debug, Clone)]
struct PendingMessage {
    conversation_id: ConversationId,
    message: ConversationMessage,
    context: Option<PtyAiContext>,
    priority: u8,
}

/// Events emitted by the conversation system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConversationEvent {
    ConversationCreated { id: ConversationId, title: String },
    ConversationUpdated { id: ConversationId },
    ConversationDeleted { id: ConversationId },
    MessageAdded { conversation_id: ConversationId, message_id: Uuid },
    MessageUpdated { conversation_id: ConversationId, message_id: Uuid },
    ContextUpdated { conversation_id: ConversationId },
    BranchCreated { conversation_id: ConversationId, branch_id: Uuid },
    ConversationActivated { id: ConversationId },
    ConversationDeactivated { id: ConversationId },
    Error { conversation_id: Option<ConversationId>, error: String },
}

/// Performance and usage statistics
#[derive(Debug, Clone, Default)]
pub struct ConversationStats {
    pub total_conversations: u64,
    pub active_conversations: u64,
    pub total_messages: u64,
    pub messages_processed_per_second: f64,
    pub average_conversation_length: f64,
    pub most_active_directory: Option<PathBuf>,
    pub common_command_patterns: HashMap<String, u32>,
    pub error_frequencies: HashMap<String, u32>,
    pub context_compression_ratio: f64,
    pub storage_usage_bytes: u64,
}

impl ConversationManager {
    /// Create a new conversation manager
    pub async fn new(
        config: ConversationConfig,
        ai_runtime: Arc<RwLock<AiRuntime>>,
        command_assistance: Arc<RwLock<CommandAssistanceEngine>>,
    ) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        let manager = Self {
            config: Arc::new(RwLock::new(config)),
            active_conversations: Arc::new(RwLock::new(HashMap::new())),
            current_conversation: Arc::new(RwLock::new(None)),
            ai_runtime,
            command_assistance,
            message_queue: Arc::new(Mutex::new(VecDeque::new())),
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            task_handles: Vec::new(),
            stats: Arc::new(RwLock::new(ConversationStats::default())),
        };
        
        // Initialize storage
        manager.initialize_storage().await?;
        
        Ok(manager)
    }
    
    /// Start the conversation management system
    pub async fn start(&mut self) -> Result<()> {
        // Start message processing
        self.start_message_processing().await;
        
        // Start auto-save task
        self.start_auto_save_task().await;
        
        // Start context analysis task
        self.start_context_analysis_task().await;

        // Auto-restore conversations from disk if configured
        if self.config.read().await.persistence.auto_restore_on_start {
            match self.restore_conversations_from_disk().await {
                Ok(restored) => {
                    info!("Restored {} conversations from disk", restored);
                }
                Err(e) => {
                    error!("Failed to restore conversations from disk: {}", e);
                }
            }
        }
        
        // Start statistics collection
        self.start_statistics_task().await;
        
        info!("Conversation management system started");
        Ok(())
    }
    
    /// Stop the conversation management system
    pub async fn stop(&mut self) {
        // Stop all background tasks
        for handle in &self.task_handles {
            handle.abort();
        }
        self.task_handles.clear();
        
        // Save all active conversations
        if let Err(e) = self.save_all_conversations().await {
            error!("Failed to save conversations during shutdown: {}", e);
        }
        
        info!("Conversation management system stopped");
    }
    
    /// Create a new conversation
    pub async fn create_conversation(
        &self,
        title: Option<String>,
        context: &PtyAiContext,
    ) -> Result<ConversationId> {
        let conversation_id = ConversationId::new();
        let now = SystemTime::now();
        // Precompute title to avoid use-after-move when inserting into map
        let title_value = title.clone().unwrap_or_else(|| format!("Conversation {}", conversation_id));
        
        let conversation_context = ConversationContext {
            initial_working_directory: context.terminal_context.working_directory.clone(),
            current_working_directory: context.terminal_context.working_directory.clone(),
            shell_type: crate::blocks_v2::ShellType::Bash, // Would be determined from context
            project_info: context.terminal_context.project_info.clone(),
            git_context: self.extract_git_context(context).await,
            environment_variables: HashMap::new(), // Would be filtered from context
            command_history: Vec::new(),
            active_sessions: Vec::new(),
            custom_data: HashMap::new(),
        };
        
        let conversation = Conversation {
            id: conversation_id,
            title: title_value.clone(),
            description: None,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
            context: conversation_context,
            tags: Vec::new(),
            is_active: true,
            parent_conversation: None,
            child_conversations: Vec::new(),
            settings: self.config.read().await.default_settings.clone(),
        };
        
        // Add to active conversations
        {
            let mut active = self.active_conversations.write().await;
            active.insert(conversation_id, conversation);
        }
        
        // Set as current if no current conversation
        {
            let mut current = self.current_conversation.write().await;
            if current.is_none() {
                *current = Some(conversation_id);
            }
        }
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_conversations += 1;
            stats.active_conversations += 1;
        }
        
        // Emit event
        let _ = self.event_sender.send(ConversationEvent::ConversationCreated {
            id: conversation_id,
            title: title_value,
        });
        
        info!("Created conversation: {}", conversation_id);
        Ok(conversation_id)
    }
    
    /// Add a message to a conversation
    pub async fn add_message(
        &self,
        conversation_id: ConversationId,
        message_type: MessageType,
        content: String,
        context: Option<PtyAiContext>,
    ) -> Result<Uuid> {
        let message_id = Uuid::new_v4();
        let now = SystemTime::now();
        
        let message = ConversationMessage {
            id: message_id,
            message_type,
            content,
            timestamp: now,
            metadata: self.extract_message_metadata(&context).await,
            attachments: self.extract_message_attachments(&context).await,
            references: Vec::new(),
        };
        
        // Add to processing queue
        {
            let mut queue = self.message_queue.lock().await;
            queue.push_back(PendingMessage {
                conversation_id,
                message: message.clone(),
                context,
                priority: self.calculate_message_priority(&message).await,
            });
        }
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_messages += 1;
        }
        
        // Emit event
        let _ = self.event_sender.send(ConversationEvent::MessageAdded {
            conversation_id,
            message_id,
        });
        
        debug!("Added message to conversation {}: {}", conversation_id, message_id);
        Ok(message_id)
    }
    
    /// Process a user input and generate AI response
    pub async fn process_user_input(
        &self,
        input: String,
        context: &PtyAiContext,
    ) -> Result<AgentResponse> {
        // Get or create current conversation
        let conversation_id = self.get_or_create_current_conversation(context).await?;
        
        // Add user message
        let user_message_id = self.add_message(
            conversation_id,
            MessageType::User,
            input.clone(),
            Some(context.clone()),
        ).await?;
        
        // Generate AI response
        let response = self.generate_ai_response(conversation_id, &input, context).await?;
        
        // Add assistant response message
        let _assistant_message_id = self.add_message(
            conversation_id,
            MessageType::Assistant,
            response.content.clone(),
            Some(context.clone()),
        ).await?;
        
        Ok(response)
    }
    
    /// Get conversation history for context
    pub async fn get_conversation_history(
        &self,
        conversation_id: ConversationId,
        limit: Option<usize>,
    ) -> Result<Vec<ConversationMessage>> {
        let active = self.active_conversations.read().await;
        
        if let Some(conversation) = active.get(&conversation_id) {
            let messages = if let Some(limit) = limit {
                conversation.messages
                    .iter()
                    .rev()
                    .take(limit)
                    .rev()
                    .cloned()
                    .collect()
            } else {
                conversation.messages.clone()
            };
            Ok(messages)
        } else {
            Err(anyhow::anyhow!("Conversation not found: {}", conversation_id))
        }
    }
    
    /// Create a branch from an existing conversation
    pub async fn create_branch(
        &self,
        conversation_id: ConversationId,
        branch_point: usize,
        title: String,
    ) -> Result<ConversationId> {
        let parent_conversation = {
            let active = self.active_conversations.read().await;
            active.get(&conversation_id).ok_or_else(|| {
                anyhow::anyhow!("Conversation not found: {}", conversation_id)
            })?.clone()
        };
        
        if branch_point >= parent_conversation.messages.len() {
            return Err(anyhow::anyhow!("Invalid branch point: {}", branch_point));
        }
        
        // Create new conversation as a branch
        let branch_id = ConversationId::new();
        let now = SystemTime::now();
        
        let mut branch_conversation = parent_conversation.clone();
        branch_conversation.id = branch_id;
        branch_conversation.title = title.clone();
        branch_conversation.created_at = now;
        branch_conversation.updated_at = now;
        branch_conversation.parent_conversation = Some(conversation_id);
        branch_conversation.child_conversations = Vec::new();
        
        // Copy messages up to branch point
        branch_conversation.messages = parent_conversation.messages
            .into_iter()
            .take(branch_point + 1)
            .collect();
        
        // Add to active conversations
        {
            let mut active = self.active_conversations.write().await;
            active.insert(branch_id, branch_conversation);
            
            // Update parent to include this branch
            if let Some(parent) = active.get_mut(&conversation_id) {
                parent.child_conversations.push(branch_id);
            }
        }
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_conversations += 1;
            stats.active_conversations += 1;
        }
        
        // Emit events
        let _ = self.event_sender.send(ConversationEvent::BranchCreated {
            conversation_id: branch_id,
            branch_id: Uuid::new_v4(),
        });
        
        info!("Created conversation branch: {} from {}", branch_id, conversation_id);
        Ok(branch_id)
    }
    
    /// Switch to a different conversation
    pub async fn switch_conversation(&self, conversation_id: ConversationId) -> Result<()> {
        // Verify conversation exists
        {
            let active = self.active_conversations.read().await;
            if !active.contains_key(&conversation_id) {
                return Err(anyhow::anyhow!("Conversation not found: {}", conversation_id));
            }
        }
        
        // Deactivate current conversation
        if let Some(current_id) = *self.current_conversation.read().await {
            let _ = self.event_sender.send(ConversationEvent::ConversationDeactivated {
                id: current_id,
            });
        }
        
        // Set new current conversation
        {
            let mut current = self.current_conversation.write().await;
            *current = Some(conversation_id);
        }
        
        // Emit activation event
        let _ = self.event_sender.send(ConversationEvent::ConversationActivated {
            id: conversation_id,
        });
        
        info!("Switched to conversation: {}", conversation_id);
        Ok(())
    }
    
    /// Search conversations based on query
    pub async fn search_conversations(&self, query: ConversationQuery) -> Result<Vec<ConversationId>> {
        let active = self.active_conversations.read().await;
        let mut results = Vec::new();
        
        for (id, conversation) in active.iter() {
            if self.conversation_matches_query(conversation, &query).await {
                results.push(*id);
            }
        }
        
        // Apply sorting and limits
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }
        
        Ok(results)
    }
    
    /// Get conversation statistics
    pub async fn get_statistics(&self) -> ConversationStats {
        self.stats.read().await.clone()
    }
    
    /// Archive old conversations
    pub async fn archive_old_conversations(&self) -> Result<usize> {
        let mut archived_count = 0;
        let config = self.config.read().await;
        
        if let Some(cleanup_after) = config.default_settings.auto_cleanup_after {
            let cutoff_time = SystemTime::now() - cleanup_after;
            let mut to_archive = Vec::new();
            
            {
                let active = self.active_conversations.read().await;
                for (id, conversation) in active.iter() {
                    if conversation.updated_at < cutoff_time && !conversation.is_active {
                        to_archive.push(*id);
                    }
                }
            }
            
            for id in to_archive {
                self.archive_conversation(id).await?;
                archived_count += 1;
            }
        }
        
        Ok(archived_count)
    }
    
    // Private implementation methods
    
    async fn initialize_storage(&self) -> Result<()> {
        let config = self.config.read().await;
        
        // Create storage directories
        tokio::fs::create_dir_all(&config.persistence.storage_directory).await?;
        
        if config.persistence.backup.enabled {
            tokio::fs::create_dir_all(&config.persistence.backup.backup_directory).await?;
        }
        
        info!("Conversation storage initialized");
        Ok(())
    }
    
    async fn start_message_processing(&mut self) {
        let message_queue = Arc::clone(&self.message_queue);
        let active_conversations = Arc::clone(&self.active_conversations);
        let command_assistance = Arc::clone(&self.command_assistance);
        let event_sender = self.event_sender.clone();
        
        let handle = tokio::spawn(async move {
            loop {
                let message = {
                    let mut queue = message_queue.lock().await;
                    queue.pop_front()
                };
                
                if let Some(pending) = message {
                    // Process the message
                    {
                        let mut active = active_conversations.write().await;
                        if let Some(conversation) = active.get_mut(&pending.conversation_id) {
                            conversation.messages.push(pending.message.clone());
                            conversation.updated_at = SystemTime::now();
                            
                            // Update context if available
                            if let Some(context) = &pending.context {
                                Self::update_conversation_context(conversation, context).await;
                            }
                            
                            // Integrate with command assistance if relevant
                            if matches!(pending.message.message_type, MessageType::CommandResult) {
                                let assistance = command_assistance.read().await;
                                // Could trigger error analysis or suggestions here
                            }
                        }
                    }
                    
                    let _ = event_sender.send(ConversationEvent::MessageAdded {
                        conversation_id: pending.conversation_id,
                        message_id: pending.message.id,
                    });
                } else {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        });
        
        self.task_handles.push(handle);
    }
    
    async fn start_auto_save_task(&mut self) {
        let active_conversations = Arc::clone(&self.active_conversations);
        let config = Arc::clone(&self.config);
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                let (auto_save_interval, storage_dir) = {
                    let cfg = config.read().await;
                    (cfg.default_settings.auto_save_interval, cfg.persistence.storage_directory.clone())
                };
                
                // Check which conversations need saving
                let conversations = active_conversations.read().await;
                let now = SystemTime::now();
                
                for (id, conversation) in conversations.iter() {
                    if conversation.settings.persist_to_disk {
                        let time_since_update = now.duration_since(conversation.updated_at)
                            .unwrap_or(Duration::ZERO);
                        
                        if time_since_update >= auto_save_interval {
                            debug!("Auto-saving conversation: {}", id);
                            if let Err(e) = save_conversation_to_disk(&storage_dir, conversation).await {
                                error!("Failed to auto-save conversation {}: {}", id, e);
                            }
                        }
                    }
                }
            }
        });
        
        self.task_handles.push(handle);
    }
    
    async fn start_context_analysis_task(&mut self) {
        let active_conversations = Arc::clone(&self.active_conversations);
        let config = Arc::clone(&self.config);
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes
            
            loop {
                interval.tick().await;
                
                let config_lock = config.read().await;
                if !config_lock.context_analysis.auto_extract_context {
                    continue;
                }
                
                // Analyze contexts for all active conversations
                let mut conversations = active_conversations.write().await;
                
                for (id, conversation) in conversations.iter_mut() {
                    // Check if conversation needs context compression
                    if config_lock.context_analysis.summarization.enabled &&
                       conversation.messages.len() >= config_lock.context_analysis.summarization.trigger_message_count {
                        
                        debug!("Analyzing context for conversation: {}", id);
                        // Simple summarization: keep last N messages, replace earlier with a summary system message
                        let keep_recent = match config_lock.default_settings.context_strategy {
                            ContextStrategy::Summarized { keep_recent } => keep_recent,
                            _ => config_lock.context_analysis.summarization.trigger_message_count / 2,
                        };
                        if conversation.messages.len() > keep_recent {
                            let old_len = conversation.messages.len();
                            let summary_text = build_summary(&conversation.messages[..old_len - keep_recent], config_lock.context_analysis.summarization.summary_length);
                            let summary_msg = ConversationMessage {
                                id: Uuid::new_v4(),
                                message_type: MessageType::System,
                                content: summary_text,
                                timestamp: SystemTime::now(),
                                metadata: HashMap::new(),
                                attachments: Vec::new(),
                                references: Vec::new(),
                            };
                            let mut new_msgs = Vec::with_capacity(keep_recent + 1);
                            new_msgs.push(summary_msg);
                            new_msgs.extend_from_slice(&conversation.messages[old_len - keep_recent..]);
                            conversation.messages = new_msgs;
                        }
                    }
                }
            }
        });
        
        self.task_handles.push(handle);
    }
    
    async fn start_statistics_task(&mut self) {
        let active_conversations = Arc::clone(&self.active_conversations);
        let stats = Arc::clone(&self.stats);
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                let conversations = active_conversations.read().await;
                let mut stats_lock = stats.write().await;
                
                // Update basic statistics
                stats_lock.active_conversations = conversations.len() as u64;
                
                // Calculate average conversation length
                if !conversations.is_empty() {
                    let total_messages: usize = conversations.values()
                        .map(|c| c.messages.len())
                        .sum();
                    stats_lock.average_conversation_length = 
                        total_messages as f64 / conversations.len() as f64;
                }
                
                // Find most active directory
                let mut directory_counts: HashMap<PathBuf, u32> = HashMap::new();
                for conversation in conversations.values() {
                    *directory_counts.entry(conversation.context.current_working_directory.clone())
                        .or_insert(0) += 1;
                }
                
                stats_lock.most_active_directory = directory_counts
                    .into_iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|(path, _)| path);
                
                debug!("Updated conversation statistics");
            }
        });
        
        self.task_handles.push(handle);
    }
    
    async fn get_or_create_current_conversation(&self, context: &PtyAiContext) -> Result<ConversationId> {
        // Check if there's a current conversation
        if let Some(current_id) = *self.current_conversation.read().await {
            return Ok(current_id);
        }
        
        // Create a new conversation
        self.create_conversation(None, context).await
    }
    
    async fn generate_ai_response(
        &self,
        conversation_id: ConversationId,
        input: &str,
        context: &PtyAiContext,
    ) -> Result<AgentResponse> {
        // Get conversation history for context
        let history = self.get_conversation_history(conversation_id, Some(10)).await?;
        
        // Build context-aware prompt
        let mut prompt = String::new();
        
        // Add conversation context
        prompt.push_str("Previous conversation:\n");
        for message in &history {
            match message.message_type {
                MessageType::User => prompt.push_str(&format!("User: {}\n", message.content)),
                MessageType::Assistant => prompt.push_str(&format!("Assistant: {}\n", message.content)),
                MessageType::CommandResult => prompt.push_str(&format!("Command Result: {}\n", message.content)),
                MessageType::Error => prompt.push_str(&format!("Error: {}\n", message.content)),
                _ => {}
            }
        }
        
        prompt.push_str(&format!("\nUser: {}\nAssistant: ", input));
        
        // Generate response using AI runtime
        let ai_runtime = self.ai_runtime.read().await;
        let response = ai_runtime.submit_prompt(prompt, Some(serde_json::to_string(context)?)).await?;
        
        Ok(response)
    }
    
    async fn extract_git_context(&self, context: &PtyAiContext) -> Option<GitContext> {
        if let Some(ref git_status) = context.terminal_context.git_status {
            Some(GitContext {
                repository_root: context.terminal_context.working_directory.clone(),
                current_branch: context.terminal_context.git_branch.clone().unwrap_or_default(),
                status: git_status.clone(),
                recent_commits: Vec::new(), // Would be populated from git log
                remote_info: None,
            })
        } else {
            None
        }
    }
    
    async fn extract_message_metadata(&self, context: &Option<PtyAiContext>) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        
        if let Some(ctx) = context {
            metadata.insert("working_directory".to_string(), 
                           ctx.terminal_context.working_directory.display().to_string());
            
            if let Some(ref branch) = ctx.terminal_context.git_branch {
                metadata.insert("git_branch".to_string(), branch.clone());
            }
            
            if let Some(ref project) = ctx.terminal_context.project_info {
                metadata.insert("project_type".to_string(), format!("{:?}", project.project_type));
            }
        }
        
        metadata
    }
    
    async fn extract_message_attachments(&self, context: &Option<PtyAiContext>) -> Vec<MessageAttachment> {
        let mut attachments = Vec::new();
        
        if let Some(ctx) = context {
            // Add terminal state as attachment
            if let Ok(terminal_state) = serde_json::to_string(&ctx.terminal_context) {
                attachments.push(MessageAttachment {
                    attachment_type: AttachmentType::TerminalState,
                    content: terminal_state.clone(),
                    filename: None,
                    size: terminal_state.len(),
                    metadata: HashMap::new(),
                });
            }
            
            // Add last output if available
            if let Some(ref output) = ctx.last_output {
                attachments.push(MessageAttachment {
                    attachment_type: AttachmentType::CommandOutput,
                    content: output.clone(),
                    filename: None,
                    size: output.len(),
                    metadata: HashMap::new(),
                });
            }
            
            // Add error context if available
            if let Some(ref error) = ctx.error_context {
                attachments.push(MessageAttachment {
                    attachment_type: AttachmentType::ErrorLog,
                    content: error.clone(),
                    filename: None,
                    size: error.len(),
                    metadata: HashMap::new(),
                });
            }
        }
        
        attachments
    }
    
    async fn calculate_message_priority(&self, message: &ConversationMessage) -> u8 {
        match message.message_type {
            MessageType::Error => 90,
            MessageType::User => 80,
            MessageType::CommandResult => 70,
            MessageType::Assistant => 60,
            MessageType::Suggestion => 50,
            MessageType::System => 40,
        }
    }
    
    async fn update_conversation_context(conversation: &mut Conversation, context: &PtyAiContext) {
        // Update current working directory
        conversation.context.current_working_directory = context.terminal_context.working_directory.clone();
        
        // Update git context
        if let Some(ref git_status) = context.terminal_context.git_status {
            if let Some(ref mut git_context) = conversation.context.git_context {
                git_context.status = git_status.clone();
                if let Some(ref branch) = context.terminal_context.git_branch {
                    git_context.current_branch = branch.clone();
                }
            }
        }
        
        // Update project info
        if let Some(ref project_info) = context.terminal_context.project_info {
            conversation.context.project_info = Some(project_info.clone());
        }
    }
    
    async fn conversation_matches_query(&self, conversation: &Conversation, query: &ConversationQuery) -> bool {
        // Text search
        if let Some(ref search_text) = query.text_search {
            let search_lower = search_text.to_lowercase();
            
            // Search in title and messages
            if !conversation.title.to_lowercase().contains(&search_lower) {
                let found_in_messages = conversation.messages.iter().any(|msg| {
                    msg.content.to_lowercase().contains(&search_lower)
                });
                if !found_in_messages {
                    return false;
                }
            }
        }
        
        // Tag filtering
        if !query.tags.is_empty() {
            let has_matching_tag = query.tags.iter().any(|tag| {
                conversation.tags.contains(tag)
            });
            if !has_matching_tag {
                return false;
            }
        }
        
        // Date range filtering
        if let Some((start, end)) = query.date_range {
            if conversation.created_at < start || conversation.created_at > end {
                return false;
            }
        }
        
        // Working directory filtering
        if let Some(ref dir) = query.working_directory {
            if &conversation.context.current_working_directory != dir {
                return false;
            }
        }
        
        // Error filtering
        if let Some(has_errors) = query.has_errors {
            let conversation_has_errors = conversation.messages.iter().any(|msg| {
                matches!(msg.message_type, MessageType::Error)
            });
            if conversation_has_errors != has_errors {
                return false;
            }
        }
        
        // Message type filtering
        if let Some(ref msg_type) = query.message_type {
            let has_message_type = conversation.messages.iter().any(|msg| {
                std::mem::discriminant(&msg.message_type) == std::mem::discriminant(msg_type)
            });
            if !has_message_type {
                return false;
            }
        }
        
        true
    }
    
    async fn save_all_conversations(&self) -> Result<()> {
        let conversations = self.active_conversations.read().await;
        let storage_dir = { self.config.read().await.persistence.storage_directory.clone() };
        
        let mut saved = 0usize;
        for (id, conversation) in conversations.iter() {
            if conversation.settings.persist_to_disk {
                if let Err(e) = save_conversation_to_disk(&storage_dir, conversation).await {
                    error!("Failed to save conversation {}: {}", id, e);
                } else {
                    saved += 1;
                }
            }
        }
        
        info!("Saved {} conversations", saved);
        Ok(())
    }
    
    async fn archive_conversation(&self, conversation_id: ConversationId) -> Result<()> {
        let mut active = self.active_conversations.write().await;
        
        if let Some(conversation) = active.remove(&conversation_id) {
            // In a real implementation, would move to archive storage
            debug!("Archived conversation: {}", conversation_id);
            
            // Update statistics
            let mut stats = self.stats.write().await;
            stats.active_conversations = stats.active_conversations.saturating_sub(1);
        }
        
        Ok(())
    }
}

async fn save_conversation_to_disk(dir: &PathBuf, conversation: &Conversation) -> Result<()> {
    tokio::fs::create_dir_all(dir).await.ok();
    let file = dir.join(format!("{}.json", conversation.id.0));
    let json = serde_json::to_string_pretty(conversation)?;
    tokio::fs::write(file, json).await?;
    Ok(())
}

fn expand_tilde(path: &PathBuf) -> PathBuf {
    let mut s = path.to_string_lossy().to_string();
    if s.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            if s == "~" {
                s = home.display().to_string();
            } else if s.starts_with("~/") {
                s = home.join(&s[2..]).display().to_string();
            }
        }
    }
    PathBuf::from(s)
}

fn chrono_to_system_time(dt: chrono::DateTime<chrono::Utc>) -> SystemTime {
    use std::time::{Duration as StdDuration, UNIX_EPOCH};
    if dt.timestamp() >= 0 {
        UNIX_EPOCH
            + StdDuration::from_secs(dt.timestamp() as u64)
            + StdDuration::from_nanos(dt.timestamp_subsec_nanos() as u64)
    } else {
        // For completeness; unlikely for our stored data
        UNIX_EPOCH
    }
}

impl ConversationManager {
    /// Restore conversations by reading saved conversation JSON files from the storage directory.
    /// Returns the number of conversations restored.
    pub async fn restore_conversations_from_disk(&self) -> Result<usize> {
        use tokio::fs;

        let cfg = self.config.read().await.clone();
        let storage_dir = expand_tilde(&cfg.persistence.storage_directory);
        fs::create_dir_all(&storage_dir).await.ok();

        let mut dir = match fs::read_dir(&storage_dir).await {
            Ok(d) => d,
            Err(e) => {
                warn!("Cannot read storage directory {}: {}", storage_dir.display(), e);
                return Ok(0);
            }
        };

        // Collect candidate files
        let mut conversations = Vec::new();
        while let Ok(Some(entry)) = dir.next_entry().await {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                // Read file
                match fs::read_to_string(&path).await {
                    Ok(contents) => {
                        match serde_json::from_str::<Conversation>(&contents) {
                            Ok(conv) => {
                                // Apply age filter if configured
                                if let Some(days) = cfg.persistence.max_restore_age_days {
                                    if let Ok(age) = SystemTime::now().duration_since(conv.updated_at) {
                                        let max = Duration::from_secs(days * 86400);
                                        if age > max {
                                            debug!("Skipping old conversation {}", path.display());
                                            continue;
                                        }
                                    }
                                }
                                conversations.push(conv);
                            }
                            Err(e) => {
                                warn!("Failed to parse conversation file {}: {}", path.display(), e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read conversation file {}: {}", path.display(), e);
                    }
                }
            }
        }

        // Sort by most recent updated_at
        conversations.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        // Apply limit and capacity constraints
        let limit = if cfg.persistence.max_restore_count == 0 {
            conversations.len()
        } else {
            cfg.persistence.max_restore_count.min(conversations.len())
        };

        let capacity = cfg.max_active_conversations.max(1);
        let restore_count = limit.min(capacity);

        // Insert into active map
        let mut active = self.active_conversations.write().await;
        let mut restored_ids = Vec::new();
        for conv in conversations.into_iter().take(restore_count) {
            let id = conv.id;
            active.insert(id, conv);
            restored_ids.push(id);
        }

        // Set current conversation to the most recent
        if let Some(first_id) = restored_ids.first().cloned() {
            let mut current = self.current_conversation.write().await;
            *current = Some(first_id);
        }

        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_conversations += restored_ids.len() as u64;
        stats.active_conversations = active.len() as u64;

        Ok(restored_ids.len())
    }

    /// Restore a single conversation from a persisted session summary entry
    pub async fn restore_conversation_from_persisted(
        &self,
        persisted: &crate::session_persistence::PersistedConversation,
    ) -> Result<ConversationId> {
        let created_at = chrono_to_system_time(persisted.created_at);
        let updated_at = chrono_to_system_time(persisted.last_message_at);

        let conversation = Conversation {
            id: persisted.conversation_id,
            title: persisted.title.clone(),
            description: Some(persisted.context_summary.clone()),
            created_at,
            updated_at,
            messages: persisted.recent_messages.clone(),
            context: ConversationContext {
                initial_working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                current_working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                shell_type: crate::blocks_v2::ShellType::Bash,
                project_info: None,
                git_context: None,
                environment_variables: HashMap::new(),
                command_history: Vec::new(),
                active_sessions: Vec::new(),
                custom_data: HashMap::new(),
            },
            tags: Vec::new(),
            is_active: true,
            parent_conversation: None,
            child_conversations: Vec::new(),
            settings: self.config.read().await.default_settings.clone(),
        };

        {
            let mut active = self.active_conversations.write().await;
            active.insert(persisted.conversation_id, conversation);
        }

        // Set as current if none
        {
            let mut current = self.current_conversation.write().await;
            if current.is_none() {
                *current = Some(persisted.conversation_id);
            }
        }

        // Emit event
        let _ = self.event_sender.send(ConversationEvent::ConversationCreated {
            id: persisted.conversation_id,
            title: persisted.title.clone(),
        });

        Ok(persisted.conversation_id)
    }
}

fn build_summary(messages: &[ConversationMessage], max_len: usize) -> String {
    // A minimal, deterministic summary: include markers and truncate to max_len
    let mut s = String::with_capacity(max_len.min(1024));
    s.push_str("[Summary of previous context]\n");
    for m in messages.iter().rev().take(20).rev() {
        match m.message_type {
            MessageType::User => {
                s.push_str("U: ");
                s.push_str(&m.content);
                s.push('\n');
            }
            MessageType::Assistant => {
                s.push_str("A: ");
                s.push_str(&m.content);
                s.push('\n');
            }
            MessageType::CommandResult => {
                s.push_str("R: ");
                s.push_str(&m.content);
                s.push('\n');
            }
            MessageType::Error => {
                s.push_str("E: ");
                s.push_str(&m.content);
                s.push('\n');
            }
            _ => {}
        }
        if s.len() >= max_len { break; }
    }
    if s.len() > max_len { s.truncate(max_len); }
    s
}

impl Drop for ConversationManager {
    fn drop(&mut self) {
        // Ensure background tasks are cleaned up
        for handle in &self.task_handles {
            handle.abort();
        }
    }
}