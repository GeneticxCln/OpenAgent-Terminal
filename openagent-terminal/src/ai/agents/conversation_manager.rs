use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use uuid::Uuid;

use super::code_generation::CodeStyle;
use super::natural_language::*;
use super::*;

/// Enhanced conversation manager for multi-turn conversations with context persistence
pub struct ConversationManager {
    id: String,
    conversations: Arc<RwLock<HashMap<Uuid, ConversationSession>>>,
    default_session: Arc<RwLock<Option<Uuid>>>,
    _context_store: Arc<RwLock<ConversationContextStore>>,
    config: ConversationConfig,
    is_initialized: bool,
}

/// A conversation session with persistent context and memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSession {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub turns: VecDeque<ConversationTurn>,
    pub context: PersistentConversationContext,
    pub preferences: ConversationPreferences,
    pub status: ConversationStatus,
    pub metadata: HashMap<String, String>,
}

/// Enhanced conversation context that persists across turns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentConversationContext {
    // Project context
    pub project_root: Option<String>,
    pub current_directory: String,
    pub git_branch: Option<String>,
    pub git_status: Option<String>,

    // File context
    pub open_files: Vec<FileContext>,
    pub recent_files: VecDeque<FileContext>,
    pub project_files: Vec<String>,

    // Command context
    pub recent_commands: VecDeque<CommandExecution>,
    pub command_history: Vec<String>,

    // Conversation memory
    pub topics: HashMap<String, TopicMemory>,
    pub entities: HashMap<String, Entity>,
    pub user_goals: Vec<UserGoal>,

    // Agent coordination
    pub active_workflows: Vec<Uuid>,
    pub agent_preferences: HashMap<String, serde_json::Value>,
}

/// File context information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContext {
    pub path: String,
    pub last_modified: DateTime<Utc>,
    pub size: u64,
    pub file_type: String,
    pub encoding: Option<String>,
    pub line_count: Option<u32>,
    pub summary: Option<String>, // AI-generated summary of file content
}

/// Command execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExecution {
    pub command: String,
    pub exit_code: i32,
    pub output: String,
    pub error: Option<String>,
    pub executed_at: DateTime<Utc>,
    pub duration_ms: u64,
}

/// Topic memory for conversation continuity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMemory {
    pub topic: String,
    pub first_mentioned: DateTime<Utc>,
    pub last_mentioned: DateTime<Utc>,
    pub importance: f32,
    pub related_entities: Vec<String>,
    pub summary: String,
    pub references: Vec<ConversationReference>,
}

/// Reference to a specific conversation turn
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationReference {
    pub turn_id: Uuid,
    pub relevance_score: f32,
    pub context_snippet: String,
}

/// User goal or objective
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserGoal {
    pub id: Uuid,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub status: GoalStatus,
    pub progress: f32, // 0.0 to 1.0
    pub related_turns: Vec<Uuid>,
    pub milestones: Vec<String>,
}

/// Status of user goals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GoalStatus {
    Active,
    Completed,
    Paused,
    Cancelled,
}

/// Conversation preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationPreferences {
    pub verbosity: VerbosityLevel,
    pub code_style: CodeStyle,
    pub preferred_languages: Vec<String>,
    pub explanation_style: ExplanationStyle,
    pub auto_context_gathering: bool,
    pub privacy_level: PrivacyLevel,
}

/// Verbosity levels for responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerbosityLevel {
    Minimal,
    Concise,
    Standard,
    Detailed,
    Exhaustive,
}

/// Explanation styles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExplanationStyle {
    Technical,
    Beginner,
    Interactive,
    StepByStep,
    Example,
}

/// Privacy levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrivacyLevel {
    Maximum,  // No external calls, local processing only
    High,     // Limited external calls with explicit consent
    Standard, // Normal external calls with privacy safeguards
    Open,     // Allow all external calls for best functionality
}

/// Conversation status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConversationStatus {
    Active,
    Paused,
    Archived,
    Deleted,
}

/// Configuration for conversation management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationConfig {
    pub max_turns_per_session: usize,
    pub max_context_size_kb: usize,
    pub auto_save_interval_seconds: u64,
    pub context_pruning_threshold: usize,
    pub enable_context_summarization: bool,
    pub enable_goal_tracking: bool,
    pub max_concurrent_workflows: usize,
}

/// Storage for conversation context
pub struct ConversationContextStore {
    // In-memory storage (could be backed by database in production)
    _sessions: HashMap<Uuid, ConversationSession>,
    _file_cache: HashMap<String, FileContext>,
    _command_cache: VecDeque<CommandExecution>,
}

impl Default for ConversationPreferences {
    fn default() -> Self {
        Self {
            verbosity: VerbosityLevel::Standard,
            code_style: CodeStyle::Hybrid,
            preferred_languages: vec!["rust".to_string(), "typescript".to_string()],
            explanation_style: ExplanationStyle::StepByStep,
            auto_context_gathering: true,
            privacy_level: PrivacyLevel::Standard,
        }
    }
}

impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
            max_turns_per_session: 1000,
            max_context_size_kb: 500,
            auto_save_interval_seconds: 30,
            context_pruning_threshold: 100,
            enable_context_summarization: true,
            enable_goal_tracking: true,
            max_concurrent_workflows: 5,
        }
    }
}

impl ConversationManager {
    pub fn new() -> Self {
        Self {
            id: "conversation-manager".to_string(),
            conversations: Arc::new(RwLock::new(HashMap::new())),
            default_session: Arc::new(RwLock::new(None)),
            _context_store: Arc::new(RwLock::new(ConversationContextStore::new())),
            config: ConversationConfig::default(),
            is_initialized: false,
        }
    }

    fn conversations_data_path() -> PathBuf {
        // ~/.local/share/openagent-terminal/ai/conversations.json
        let base = dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("openagent-terminal")
            .join("ai");
        std::fs::create_dir_all(&base).ok();
        base.join("conversations.json")
    }

    async fn save_all(&self) -> Result<()> {
        let sessions: Vec<ConversationSession> = {
            let conversations = self.conversations.read().await;
            conversations.values().cloned().collect()
        };
        let payload = serde_json::to_vec_pretty(&sessions)?;
        let path = Self::conversations_data_path();
        tokio::fs::write(path, payload).await?;
        Ok(())
    }

    async fn load_all(&self) -> Result<()> {
        let path = Self::conversations_data_path();
        if let Ok(bytes) = tokio::fs::read(&path).await {
            if let Ok(sessions) = serde_json::from_slice::<Vec<ConversationSession>>(&bytes) {
                let mut conversations = self.conversations.write().await;
                conversations.clear();
                for s in sessions {
                    conversations.insert(s.id, s);
                }
                // Reset default session if any session exists
                let mut default_session = self.default_session.write().await;
                if default_session.is_none() {
                    *default_session = conversations.keys().next().cloned();
                }
            }
        }
        Ok(())
    }

    pub fn with_config(mut self, config: ConversationConfig) -> Self {
        self.config = config;
        self
    }

    /// Create a new conversation session
    pub async fn create_session(&self, title: Option<String>) -> Result<Uuid> {
        let session_id = Uuid::new_v4();
        let session = ConversationSession {
            id: session_id,
            title: title.unwrap_or_else(|| {
                format!("Conversation {}", chrono::Utc::now().format("%Y-%m-%d %H:%M"))
            }),
            created_at: Utc::now(),
            last_active: Utc::now(),
            turns: VecDeque::new(),
            context: PersistentConversationContext::new(),
            preferences: ConversationPreferences::default(),
            status: ConversationStatus::Active,
            metadata: HashMap::new(),
        };

        let mut conversations = self.conversations.write().await;
        conversations.insert(session_id, session);

        // Set as default if no default exists
        let mut default_session = self.default_session.write().await;
        if default_session.is_none() {
            *default_session = Some(session_id);
        }

        tracing::info!("Created new conversation session: {}", session_id);
        Ok(session_id)
    }

    /// Get or create the default conversation session
    pub async fn get_default_session(&self) -> Result<Uuid> {
        let default_session = self.default_session.read().await;

        if let Some(session_id) = *default_session {
            Ok(session_id)
        } else {
            drop(default_session);
            self.create_session(Some("Default Conversation".to_string())).await
        }
    }

    /// Add a turn to a conversation session
    pub async fn add_turn(
        &self,
        session_id: Uuid,
        role: ConversationRole,
        content: String,
        intent: Option<Intent>,
        entities: Vec<Entity>,
    ) -> Result<Uuid> {
        let mut conversations = self.conversations.write().await;

        let session = conversations
            .get_mut(&session_id)
            .ok_or_else(|| anyhow!("Conversation session not found: {}", session_id))?;

        let turn_id = Uuid::new_v4();
        let turn = ConversationTurn {
            id: turn_id,
            timestamp: Utc::now(),
            role,
            content: content.clone(),
            intent: intent.clone(),
            entities: entities.clone(),
            confidence: 1.0,
        };

        // Add turn to session
        session.turns.push_back(turn);
        session.last_active = Utc::now();

        // Update context with new information
        self.update_context_from_turn(&mut session.context, &content, &intent, &entities).await?;

        // Prune old turns if needed
        if session.turns.len() > self.config.max_turns_per_session {
            session.turns.pop_front();
        }

        // Update topic memory
        self.update_topic_memory(&mut session.context, &content, turn_id).await?;

        // Persist conversations after each turn
        drop(conversations);
        self.save_all().await?;

        tracing::debug!("Added turn {} to session {}", turn_id, session_id);
        Ok(turn_id)
    }

    /// Get conversation context for a session
    pub async fn get_context(&self, session_id: Uuid) -> Result<PersistentConversationContext> {
        let conversations = self.conversations.read().await;

        let session = conversations
            .get(&session_id)
            .ok_or_else(|| anyhow!("Conversation session not found: {}", session_id))?;

        Ok(session.context.clone())
    }

    /// Update conversation context with file system information
    pub async fn update_file_context(&self, session_id: Uuid, files: Vec<String>) -> Result<()> {
        let mut conversations = self.conversations.write().await;

        let session = conversations
            .get_mut(&session_id)
            .ok_or_else(|| anyhow!("Conversation session not found: {}", session_id))?;

        // Update file context
        for file_path in files {
            if let Ok(metadata) = std::fs::metadata(&file_path) {
                let file_context = FileContext {
                    path: file_path.clone(),
                    last_modified: DateTime::from(
                        metadata.modified().unwrap_or(std::time::SystemTime::now()),
                    ),
                    size: metadata.len(),
                    file_type: self.detect_file_type(&file_path),
                    encoding: None,   // Could be detected
                    line_count: None, // Could be calculated
                    summary: None,    // Could be AI-generated
                };

                // Update or add file context
                if let Some(existing) =
                    session.context.open_files.iter_mut().find(|f| f.path == file_path)
                {
                    *existing = file_context;
                } else {
                    session.context.open_files.push(file_context);
                }
            }
        }

        Ok(())
    }

    /// Update conversation context with command execution
    pub async fn add_command_execution(
        &self,
        session_id: Uuid,
        command: String,
        exit_code: i32,
        output: String,
        error: Option<String>,
        duration_ms: u64,
    ) -> Result<()> {
        let mut conversations = self.conversations.write().await;

        let session = conversations
            .get_mut(&session_id)
            .ok_or_else(|| anyhow!("Conversation session not found: {}", session_id))?;

        let execution = CommandExecution {
            command: command.clone(),
            exit_code,
            output,
            error,
            executed_at: Utc::now(),
            duration_ms,
        };

        session.context.recent_commands.push_back(execution);
        session.context.command_history.push(command);

        // Keep only recent commands
        if session.context.recent_commands.len() > 50 {
            session.context.recent_commands.pop_front();
        }

        if session.context.command_history.len() > 100 {
            session.context.command_history.remove(0);
        }

        Ok(())
    }

    /// Get conversation summary for context
    pub async fn get_conversation_summary(
        &self,
        session_id: Uuid,
        max_turns: usize,
    ) -> Result<String> {
        let conversations = self.conversations.read().await;

        let session = conversations
            .get(&session_id)
            .ok_or_else(|| anyhow!("Conversation session not found: {}", session_id))?;

        let recent_turns: Vec<&ConversationTurn> =
            session.turns.iter().rev().take(max_turns).collect();

        let mut summary = String::new();
        summary.push_str(&format!("Conversation: {}\n", session.title));
        summary.push_str(&format!(
            "Active since: {}\n",
            session.created_at.format("%Y-%m-%d %H:%M UTC")
        ));

        if !recent_turns.is_empty() {
            summary.push_str(&format!("Recent turns ({}):\n", recent_turns.len()));

            for turn in recent_turns.iter().rev() {
                let role = match turn.role {
                    ConversationRole::User => "User",
                    ConversationRole::Assistant => "Assistant",
                    ConversationRole::System => "System",
                };

                let content = if turn.content.len() > 100 {
                    format!("{}...", &turn.content[..100])
                } else {
                    turn.content.clone()
                };

                summary.push_str(&format!("  {}: {}\n", role, content));
            }
        }

        Ok(summary)
    }

    /// Create a rich context string for AI agents
    pub async fn build_context_for_agent(&self, session_id: Uuid) -> Result<String> {
        let conversations = self.conversations.read().await;

        let session = conversations
            .get(&session_id)
            .ok_or_else(|| anyhow!("Conversation session not found: {}", session_id))?;

        let mut context = String::new();

        // Project context
        if let Some(project_root) = &session.context.project_root {
            context.push_str(&format!("Project: {}\n", project_root));
        }
        context.push_str(&format!("Directory: {}\n", session.context.current_directory));

        if let Some(branch) = &session.context.git_branch {
            context.push_str(&format!("Git branch: {}\n", branch));
        }

        // File context
        if !session.context.open_files.is_empty() {
            context.push_str("Open files:\n");
            for file in &session.context.open_files {
                context.push_str(&format!("  - {} ({})\n", file.path, file.file_type));
            }
        }

        // Recent commands
        if !session.context.recent_commands.is_empty() {
            context.push_str("Recent commands:\n");
            for cmd in session.context.recent_commands.iter().rev().take(5) {
                context.push_str(&format!("  $ {} (exit: {})\n", cmd.command, cmd.exit_code));
            }
        }

        // Active goals
        if !session.context.user_goals.is_empty() {
            context.push_str("User goals:\n");
            for goal in &session.context.user_goals {
                if matches!(goal.status, GoalStatus::Active) {
                    context.push_str(&format!(
                        "  - {} ({}% complete)\n",
                        goal.description,
                        (goal.progress * 100.0) as u8
                    ));
                }
            }
        }

        // Conversation preferences
        context.push_str(&format!(
            "Preferences: {:?} verbosity, {:?} explanations\n",
            session.preferences.verbosity, session.preferences.explanation_style
        ));

        Ok(context)
    }

    /// Update context from conversation turn
    async fn update_context_from_turn(
        &self,
        context: &mut PersistentConversationContext,
        _content: &str,
        intent: &Option<Intent>,
        entities: &[Entity],
    ) -> Result<()> {
        // Update entities
        for entity in entities {
            let key = format!("{:?}", entity.entity_type);
            context.entities.insert(key, entity.clone());
        }

        // Extract file paths mentioned
        if let Some(file_entity) =
            entities.iter().find(|e| matches!(e.entity_type, EntityType::FilePath))
        {
            // Add to recent files if not already open
            if !context.open_files.iter().any(|f| f.path == file_entity.value) {
                if let Ok(metadata) = std::fs::metadata(&file_entity.value) {
                    let file_context = FileContext {
                        path: file_entity.value.clone(),
                        last_modified: DateTime::from(
                            metadata.modified().unwrap_or(std::time::SystemTime::now()),
                        ),
                        size: metadata.len(),
                        file_type: self.detect_file_type(&file_entity.value),
                        encoding: None,
                        line_count: None,
                        summary: None,
                    };

                    context.recent_files.push_back(file_context);
                    if context.recent_files.len() > 20 {
                        context.recent_files.pop_front();
                    }
                }
            }
        }

        // Update goals based on intent
        if let Some(intent) = intent {
            self.update_goals_from_intent(context, intent).await?;
        }

        Ok(())
    }

    /// Update topic memory from content
    async fn update_topic_memory(
        &self,
        context: &mut PersistentConversationContext,
        content: &str,
        turn_id: Uuid,
    ) -> Result<()> {
        // Simple topic extraction (could be enhanced with NLP)
        let lowercase_content = content.to_lowercase();
        let words: Vec<&str> =
            lowercase_content.split_whitespace().filter(|w| w.len() > 4).take(10).collect();

        for word in words {
            let topic = word.to_string();

            if let Some(memory) = context.topics.get_mut(&topic) {
                memory.last_mentioned = Utc::now();
                memory.importance += 0.1;
                memory.references.push(ConversationReference {
                    turn_id,
                    relevance_score: 0.8,
                    context_snippet: content.chars().take(100).collect(),
                });
            } else {
                context.topics.insert(
                    topic.clone(),
                    TopicMemory {
                        topic: topic.clone(),
                        first_mentioned: Utc::now(),
                        last_mentioned: Utc::now(),
                        importance: 1.0,
                        related_entities: Vec::new(),
                        summary: content.chars().take(200).collect(),
                        references: vec![ConversationReference {
                            turn_id,
                            relevance_score: 0.8,
                            context_snippet: content.chars().take(100).collect(),
                        }],
                    },
                );
            }
        }

        Ok(())
    }

    /// Update user goals based on intent
    async fn update_goals_from_intent(
        &self,
        context: &mut PersistentConversationContext,
        intent: &Intent,
    ) -> Result<()> {
        // Look for goal-indicating intents
        match intent.name.as_str() {
            "code_generation" => {
                let goal_description = format!(
                    "Generate {} code",
                    intent.parameters.get("language").unwrap_or(&"code".to_string())
                );
                self.add_or_update_goal(context, goal_description, 0.0).await?;
            }
            "project_setup" => {
                let goal_description = "Set up new project".to_string();
                self.add_or_update_goal(context, goal_description, 0.0).await?;
            }
            "debugging" => {
                let goal_description = "Debug and fix issues".to_string();
                self.add_or_update_goal(context, goal_description, 0.0).await?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Add or update a user goal
    async fn add_or_update_goal(
        &self,
        context: &mut PersistentConversationContext,
        description: String,
        progress: f32,
    ) -> Result<()> {
        // Check if similar goal exists
        if let Some(existing_goal) = context
            .user_goals
            .iter_mut()
            .find(|g| g.description.contains(&description) || description.contains(&g.description))
        {
            existing_goal.progress = progress.max(existing_goal.progress);
        } else {
            let goal = UserGoal {
                id: Uuid::new_v4(),
                description,
                created_at: Utc::now(),
                status: GoalStatus::Active,
                progress,
                related_turns: Vec::new(),
                milestones: Vec::new(),
            };
            context.user_goals.push(goal);
        }

        Ok(())
    }

    /// Detect file type from path
    fn detect_file_type(&self, path: &str) -> String {
        if let Some(extension) = path.split('.').next_back() {
            match extension.to_lowercase().as_str() {
                "rs" => "Rust",
                "py" => "Python",
                "js" => "JavaScript",
                "ts" => "TypeScript",
                "go" => "Go",
                "java" => "Java",
                "cpp" | "cc" | "cxx" => "C++",
                "c" => "C",
                "h" | "hpp" => "Header",
                "md" => "Markdown",
                "txt" => "Text",
                "json" => "JSON",
                "yaml" | "yml" => "YAML",
                "toml" => "TOML",
                "xml" => "XML",
                "html" => "HTML",
                "css" => "CSS",
                _ => "Unknown",
            }
            .to_string()
        } else {
            "Unknown".to_string()
        }
    }
}

#[async_trait]
impl Agent for ConversationManager {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Conversation Manager"
    }

    fn description(&self) -> &str {
        "Manages multi-turn conversations with persistent context and memory"
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::ContextManagement,
            AgentCapability::Custom("ConversationManagement".to_string()),
            AgentCapability::Custom("ContextPersistence".to_string()),
            AgentCapability::Custom("GoalTracking".to_string()),
        ]
    }

    async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        let mut response = AgentResponse {
            request_id: request.id,
            agent_id: self.id.clone(),
            success: false,
            payload: serde_json::json!({}),
            artifacts: Vec::new(),
            next_actions: Vec::new(),
            metadata: HashMap::new(),
        };

        match request.request_type {
            AgentRequestType::Custom(ref custom_type) => {
                match custom_type.as_str() {
                    "CreateSession" => {
                        let title = request
                            .payload
                            .get("title")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string());

                        match self.create_session(title).await {
                            Ok(session_id) => {
                                response.success = true;
                                response.payload = serde_json::json!({
                                    "session_id": session_id
                                });
                            }
                            Err(e) => {
                                response.payload = serde_json::json!({
                                    "error": e.to_string()
                                });
                            }
                        }
                    }
                    "AddTurn" => {
                        // Handle adding a conversation turn
                        if let (Some(session_id), Some(content), Some(role)) = (
                            request
                                .payload
                                .get("session_id")
                                .and_then(|v| v.as_str())
                                .and_then(|s| Uuid::parse_str(s).ok()),
                            request.payload.get("content").and_then(|v| v.as_str()),
                            request.payload.get("role").and_then(|v| v.as_str()),
                        ) {
                            let conversation_role = match role {
                                "user" => ConversationRole::User,
                                "assistant" => ConversationRole::Assistant,
                                "system" => ConversationRole::System,
                                _ => ConversationRole::User,
                            };

                            match self
                                .add_turn(
                                    session_id,
                                    conversation_role,
                                    content.to_string(),
                                    None,
                                    Vec::new(),
                                )
                                .await
                            {
                                Ok(turn_id) => {
                                    response.success = true;
                                    response.payload = serde_json::json!({
                                        "turn_id": turn_id
                                    });
                                }
                                Err(e) => {
                                    response.payload = serde_json::json!({
                                        "error": e.to_string()
                                    });
                                }
                            }
                        }
                    }
                    "GetContext" => {
                        let session_id = self.get_default_session().await?;
                        match self.build_context_for_agent(session_id).await {
                            Ok(context) => {
                                response.success = true;
                                response.payload = serde_json::json!({
                                    "context": context,
                                    "session_id": session_id
                                });
                            }
                            Err(e) => {
                                response.payload = serde_json::json!({
                                    "error": e.to_string()
                                });
                            }
                        }
                    }
                    _ => {
                        return Err(anyhow!("Unknown custom request type: {}", custom_type));
                    }
                }
            }
            _ => {
                return Err(anyhow!(
                    "Conversation Manager cannot handle request type: {:?}",
                    request.request_type
                ));
            }
        }

        Ok(response)
    }

    fn can_handle(&self, request_type: &AgentRequestType) -> bool {
        matches!(
            request_type,
            AgentRequestType::Custom(custom_type)
            if custom_type == "CreateSession"
            || custom_type == "AddTurn"
            || custom_type == "GetContext"
        )
    }

    async fn status(&self) -> AgentStatus {
        let conversations = self.conversations.read().await;
        let active_conversations = conversations
            .values()
            .filter(|s| matches!(s.status, ConversationStatus::Active))
            .count();

        AgentStatus {
            is_healthy: self.is_initialized,
            is_busy: active_conversations > 0,
            last_activity: Utc::now(),
            current_task: if active_conversations > 0 {
                Some(format!("Managing {} active conversations", active_conversations))
            } else {
                None
            },
            error_message: None,
        }
    }

    async fn initialize(&mut self, _config: AgentConfig) -> Result<()> {
        self.is_initialized = true;
        // Attempt to load previous conversations from disk
        let _ = self.load_all().await;
        tracing::info!("Conversation Manager initialized");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Save conversation state
        let _ = self.save_all().await;
        self.is_initialized = false;
        tracing::info!("Conversation Manager shut down");
        Ok(())
    }
}

impl PersistentConversationContext {
    pub fn new() -> Self {
        Self {
            project_root: None,
            current_directory: std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            git_branch: None,
            git_status: None,
            open_files: Vec::new(),
            recent_files: VecDeque::new(),
            project_files: Vec::new(),
            recent_commands: VecDeque::new(),
            command_history: Vec::new(),
            topics: HashMap::new(),
            entities: HashMap::new(),
            user_goals: Vec::new(),
            active_workflows: Vec::new(),
            agent_preferences: HashMap::new(),
        }
    }
}

impl ConversationContextStore {
    pub fn new() -> Self {
        Self {
            _sessions: HashMap::new(),
            _file_cache: HashMap::new(),
            _command_cache: VecDeque::new(),
        }
    }
}

impl Default for ConversationManager {
    fn default() -> Self { Self::new() }
}

impl Default for PersistentConversationContext {
    fn default() -> Self { Self::new() }
}

impl Default for ConversationContextStore {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_conversation_manager_creation() {
        let manager = ConversationManager::new();
        assert_eq!(manager.id(), "conversation-manager");
        assert_eq!(manager.name(), "Conversation Manager");
    }

    #[tokio::test]
    async fn test_conversation_session_creation() {
        let manager = ConversationManager::new();
        let session_id = manager.create_session(Some("Test Session".to_string())).await.unwrap();

        let conversations = manager.conversations.read().await;
        let session = conversations.get(&session_id).unwrap();

        assert_eq!(session.title, "Test Session");
        assert!(matches!(session.status, ConversationStatus::Active));
    }

    #[tokio::test]
    async fn test_conversation_turns() {
        let manager = ConversationManager::new();
        let session_id = manager.create_session(None).await.unwrap();

        let turn_id = manager
            .add_turn(
                session_id,
                ConversationRole::User,
                "Hello, world!".to_string(),
                None,
                Vec::new(),
            )
            .await
            .unwrap();

        let conversations = manager.conversations.read().await;
        let session = conversations.get(&session_id).unwrap();

        assert_eq!(session.turns.len(), 1);
        assert_eq!(session.turns[0].id, turn_id);
        assert_eq!(session.turns[0].content, "Hello, world!");
    }
}
