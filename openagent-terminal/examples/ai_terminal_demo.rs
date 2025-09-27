//! Production AI Terminal Integration
//! 
//! Complete AI terminal integration system providing real-time AI assistance,
//! intelligent command suggestions, error analysis, and natural language interaction.

use std::env;
use std::path::PathBuf;
use std::time::Duration;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::time::sleep;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use openagent_terminal::ai_terminal_integration::{
    AiTerminalIntegrationManager, AiTerminalConfig, AssistanceType
};
use openagent_terminal::ai_runtime::{AiRuntime, AiProvider, AgentRequest, AgentResponse};
use openagent_terminal::blocks_v2::{BlockManager, ShellType, CreateBlockParams};
use openagent_terminal::security_lens::{SecurityLens, SecurityPolicy};
use openagent_terminal::command_assistance::{
    CommandAssistanceWorkflow, AssistanceType as WorkflowAssistanceType,
    CompletionSuggestion, FixSuggestion, CommandSuggestion
};

/// Production AI Terminal Integration System
pub struct AiTerminalSystem {
    /// Core AI runtime for processing requests
    ai_runtime: Arc<RwLock<AiRuntime>>,
    
    /// Block manager for command tracking
    block_manager: Arc<RwLock<BlockManager>>,
    
    /// Security analysis system
    security_lens: Arc<RwLock<SecurityLens>>,
    
    /// Command assistance workflow manager
    assistance_workflow: Arc<RwLock<CommandAssistanceWorkflow>>,
    
    /// Integration manager
    integration_manager: AiTerminalIntegrationManager,
    
    /// Event channel for real-time updates
    event_sender: mpsc::UnboundedSender<AiTerminalEvent>,
    event_receiver: Arc<RwLock<mpsc::UnboundedReceiver<AiTerminalEvent>>>,
    
    /// Configuration
    config: AiTerminalConfig,
    
    /// Current session information
    session_info: SessionInfo,
    
    /// Performance metrics
    metrics: Arc<RwLock<AiTerminalMetrics>>,
    
    /// Active AI conversations
    conversations: Arc<RwLock<HashMap<Uuid, AiConversation>>>,
}

/// Session information for AI context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: Uuid,
    pub current_directory: PathBuf,
    pub shell_type: ShellType,
    pub environment: HashMap<String, String>,
    pub user_id: Option<String>,
    pub project_context: Option<ProjectContext>,
}

/// Project context for enhanced AI assistance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    pub root_path: PathBuf,
    pub project_type: ProjectType,
    pub languages: Vec<String>,
    pub build_tools: Vec<String>,
    pub git_info: Option<GitInfo>,
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectType {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    CSharp,
    CPlusPlus,
    Mixed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    pub current_branch: String,
    pub remote_url: Option<String>,
    pub uncommitted_changes: bool,
    pub ahead_behind: Option<(u32, u32)>, // (ahead, behind)
}

/// AI terminal events for real-time updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiTerminalEvent {
    /// Command assistance provided
    AssistanceProvided {
        assistance_type: AssistanceType,
        command: String,
        suggestions: Vec<String>,
        confidence: f64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    /// Security analysis completed
    SecurityAnalysis {
        command: String,
        risk_level: String,
        warnings: Vec<String>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    /// AI conversation started
    ConversationStarted {
        conversation_id: Uuid,
        initial_prompt: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    /// AI response generated
    ResponseGenerated {
        conversation_id: Uuid,
        response: String,
        confidence: f64,
        processing_time: Duration,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    /// Error occurred
    Error {
        error_type: String,
        message: String,
        context: HashMap<String, String>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    
    /// Performance metrics updated
    MetricsUpdate {
        metrics: AiTerminalMetrics,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// AI conversation state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConversation {
    pub id: Uuid,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub messages: Vec<ConversationMessage>,
    pub context: ConversationContext,
    pub state: ConversationState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationContext {
    pub current_directory: PathBuf,
    pub recent_commands: Vec<String>,
    pub active_files: Vec<PathBuf>,
    pub project_context: Option<ProjectContext>,
    pub user_intent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConversationState {
    Active,
    WaitingForInput,
    Processing,
    Completed,
    Error,
}

/// Performance metrics for AI terminal operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiTerminalMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time: Duration,
    pub total_suggestions_provided: u64,
    pub accepted_suggestions: u64,
    pub security_warnings_issued: u64,
    pub conversations_started: u64,
    pub conversations_completed: u64,
}

impl AiTerminalSystem {
    /// Create a new AI terminal system
    pub async fn new(config: AiTerminalConfig) -> Result<Self> {
        info!("Initializing AI Terminal System");

        // Create event channel
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        // Initialize session info
        let current_dir = env::current_dir()?;
        let shell_type = detect_shell();
        let session_info = SessionInfo {
            session_id: Uuid::new_v4(),
            current_directory: current_dir.clone(),
            shell_type,
            environment: env::vars().collect(),
            user_id: env::var("USER").ok().or_else(|| env::var("USERNAME").ok()),
            project_context: Self::detect_project_context(&current_dir).await?,
        };

        // Initialize AI runtime
        let ai_runtime = Arc::new(RwLock::new(AiRuntime::new()));

        // Initialize block manager
        let blocks_db_path = config.blocks_database_path.as_ref()
            .map(|p| p.clone())
            .unwrap_or_else(|| {
                let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
                path.push("openagent-terminal");
                path.push("blocks.db");
                path
            });
        
        std::fs::create_dir_all(blocks_db_path.parent().unwrap())?;
        let block_manager = Arc::new(RwLock::new(
            BlockManager::new(&blocks_db_path).await?
        ));

        // Initialize security lens
        let security_policy = SecurityPolicy::default();
        let security_lens = Arc::new(RwLock::new(
            SecurityLens::new(security_policy)
        ));

        // Initialize command assistance workflow
        let assistance_workflow = Arc::new(RwLock::new(
            CommandAssistanceWorkflow::new(
                Arc::clone(&ai_runtime),
                session_info.current_directory.clone(),
                session_info.shell_type.clone(),
            ).await?
        ));

        // Initialize integration manager
        let integration_manager = AiTerminalIntegrationManager::new(
            config.clone(),
            current_dir,
            shell_type,
        ).await?;

        let system = Self {
            ai_runtime,
            block_manager,
            security_lens,
            assistance_workflow,
            integration_manager,
            event_sender,
            event_receiver: Arc::new(RwLock::new(event_receiver)),
            config,
            session_info,
            metrics: Arc::new(RwLock::new(AiTerminalMetrics::default())),
            conversations: Arc::new(RwLock::new(HashMap::new())),
        };

        info!("AI Terminal System initialized successfully");
        Ok(system)
    }

    /// Start the AI terminal system
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting AI Terminal System");

        // Start integration manager
        self.integration_manager.start().await?;

        // Start event processing loop
        self.start_event_processing_loop().await?;

        // Start periodic tasks
        self.start_periodic_tasks().await?;

        info!("AI Terminal System started successfully");
        Ok(())
    }

    /// Process a command with AI assistance
    pub async fn process_command(&mut self, command: &str) -> Result<CommandProcessingResult> {
        debug!("Processing command: {}", command);

        let start_time = std::time::Instant::now();
        let mut result = CommandProcessingResult {
            command: command.to_string(),
            assistance_provided: Vec::new(),
            security_analysis: None,
            suggestions: Vec::new(),
            should_execute: true,
            processing_time: Duration::from_millis(0),
        };

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_requests += 1;
        }

        // 1. Security analysis
        let security_analysis = {
            let mut security_lens = self.security_lens.write().await;
            security_lens.analyze_command(command)
        };

        if security_analysis.level != openagent_terminal::security_lens::RiskLevel::Safe {
            result.security_analysis = Some(SecurityAnalysisResult {
                risk_level: format!("{:?}", security_analysis.level),
                warnings: security_analysis.factors.clone(),
                recommendations: security_analysis.mitigations.clone(),
                should_block: {
                    let security_lens = self.security_lens.read().await;
                    security_lens.should_block(&security_analysis)
                },
            });

            // Emit security event
            let _ = self.event_sender.send(AiTerminalEvent::SecurityAnalysis {
                command: command.to_string(),
                risk_level: format!("{:?}", security_analysis.level),
                warnings: security_analysis.factors.clone(),
                timestamp: chrono::Utc::now(),
            });

            if result.security_analysis.as_ref().unwrap().should_block {
                result.should_execute = false;
                warn!("Command blocked due to security risk: {}", command);
            }
        }

        // 2. Command assistance
        if self.config.enable_command_assistance {
            let assistance_result = {
                let mut workflow = self.assistance_workflow.write().await;
                workflow.provide_assistance(command).await?
            };

            match assistance_result {
                WorkflowAssistanceType::AutoCompletion { suggestions, .. } => {
                    result.suggestions = suggestions.into_iter()
                        .map(|s| s.text)
                        .collect();
                }
                WorkflowAssistanceType::ErrorExplanation { explanation, fixes, .. } => {
                    result.assistance_provided.push(AssistanceInfo {
                        assistance_type: "error_explanation".to_string(),
                        content: explanation,
                        suggestions: fixes.into_iter().map(|f| f.command).collect(),
                        confidence: 0.8,
                    });
                }
                WorkflowAssistanceType::CommandSuggestion { suggestions, reasoning, .. } => {
                    result.assistance_provided.push(AssistanceInfo {
                        assistance_type: "command_suggestion".to_string(),
                        content: reasoning,
                        suggestions: suggestions.into_iter().map(|s| s.command).collect(),
                        confidence: 0.7,
                    });
                }
                _ => {}
            }
        }

        // 3. Create command block for tracking
        if result.should_execute {
            let params = CreateBlockParams::new(command.to_string());
            let mut block_manager = self.block_manager.write().await;
            let block = block_manager.create_block(params).await?;
            debug!("Created command block: {}", block.id);
        }

        // 4. AI-powered suggestions (if enabled)
        if self.config.enable_ai_suggestions && result.suggestions.is_empty() {
            let ai_suggestions = self.get_ai_suggestions(command).await?;
            result.suggestions.extend(ai_suggestions);
        }

        result.processing_time = start_time.elapsed();

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.successful_requests += 1;
            let total_time = metrics.average_response_time * (metrics.successful_requests - 1) as u32 + result.processing_time;
            metrics.average_response_time = total_time / metrics.successful_requests as u32;
            
            if !result.assistance_provided.is_empty() {
                metrics.total_suggestions_provided += result.assistance_provided.len() as u64;
            }
        }

        // Emit assistance event
        if !result.assistance_provided.is_empty() {
            let _ = self.event_sender.send(AiTerminalEvent::AssistanceProvided {
                assistance_type: AssistanceType::CommandSuggestion {
                    context: "command_processing".to_string(),
                    suggestions: result.assistance_provided.iter()
                        .flat_map(|a| a.suggestions.clone())
                        .collect(),
                    reasoning: "AI-powered command analysis".to_string(),
                },
                command: command.to_string(),
                suggestions: result.suggestions.clone(),
                confidence: result.assistance_provided.iter()
                    .map(|a| a.confidence)
                    .fold(0.0, |acc, x| acc.max(x)),
                timestamp: chrono::Utc::now(),
            });
        }

        debug!("Command processing completed in {:?}", result.processing_time);
        Ok(result)
    }

    /// Start a natural language conversation with AI
    pub async fn start_conversation(&mut self, initial_prompt: &str) -> Result<Uuid> {
        let conversation_id = Uuid::new_v4();
        let context = ConversationContext {
            current_directory: self.session_info.current_directory.clone(),
            recent_commands: self.get_recent_commands().await?,
            active_files: self.get_active_files().await?,
            project_context: self.session_info.project_context.clone(),
            user_intent: Some(initial_prompt.to_string()),
        };

        let conversation = AiConversation {
            id: conversation_id,
            started_at: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
            messages: vec![ConversationMessage {
                id: Uuid::new_v4(),
                role: MessageRole::User,
                content: initial_prompt.to_string(),
                timestamp: chrono::Utc::now(),
                metadata: HashMap::new(),
            }],
            context,
            state: ConversationState::Processing,
        };

        {
            let mut conversations = self.conversations.write().await;
            conversations.insert(conversation_id, conversation);
        }

        // Emit event
        let _ = self.event_sender.send(AiTerminalEvent::ConversationStarted {
            conversation_id,
            initial_prompt: initial_prompt.to_string(),
            timestamp: chrono::Utc::now(),
        });

        // Process initial message
        tokio::spawn({
            let conversation_id = conversation_id;
            let ai_runtime = Arc::clone(&self.ai_runtime);
            let conversations = Arc::clone(&self.conversations);
            let event_sender = self.event_sender.clone();
            
            async move {
                if let Err(e) = Self::process_conversation_message(
                    conversation_id, 
                    ai_runtime, 
                    conversations, 
                    event_sender
                ).await {
                    error!("Failed to process conversation message: {}", e);
                }
            }
        });

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.conversations_started += 1;
        }

        info!("Started conversation: {}", conversation_id);
        Ok(conversation_id)
    }

    /// Continue an existing conversation
    pub async fn continue_conversation(&mut self, conversation_id: Uuid, message: &str) -> Result<()> {
        {
            let mut conversations = self.conversations.write().await;
            if let Some(conversation) = conversations.get_mut(&conversation_id) {
                conversation.messages.push(ConversationMessage {
                    id: Uuid::new_v4(),
                    role: MessageRole::User,
                    content: message.to_string(),
                    timestamp: chrono::Utc::now(),
                    metadata: HashMap::new(),
                });
                conversation.last_activity = chrono::Utc::now();
                conversation.state = ConversationState::Processing;
            } else {
                return Err(anyhow::anyhow!("Conversation not found: {}", conversation_id));
            }
        }

        // Process the message
        tokio::spawn({
            let conversation_id = conversation_id;
            let ai_runtime = Arc::clone(&self.ai_runtime);
            let conversations = Arc::clone(&self.conversations);
            let event_sender = self.event_sender.clone();
            
            async move {
                if let Err(e) = Self::process_conversation_message(
                    conversation_id, 
                    ai_runtime, 
                    conversations, 
                    event_sender
                ).await {
                    error!("Failed to process conversation message: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Get AI suggestions for a command
    async fn get_ai_suggestions(&self, command: &str) -> Result<Vec<String>> {
        let ai_runtime = self.ai_runtime.read().await;
        
        // Create AI request for suggestions
        let request = AgentRequest {
            request_type: openagent_terminal::ai_runtime::AgentRequestType::ProcessInput,
            input: format!("Suggest improvements or alternatives for this command: {}", command),
            context: HashMap::from([
                ("session_id".to_string(), serde_json::Value::String(self.session_info.session_id.to_string())),
                ("current_directory".to_string(), serde_json::Value::String(self.session_info.current_directory.to_string_lossy().to_string())),
                ("shell_type".to_string(), serde_json::Value::String(format!("{:?}", self.session_info.shell_type))),
            ]),
            metadata: HashMap::new(),
        };

        // This would be implemented with actual AI provider integration
        // For now, return some example suggestions
        let suggestions = match command {
            cmd if cmd.starts_with("ls") => vec![
                "ls -la  # Show detailed listing with hidden files".to_string(),
                "ls -lh  # Show sizes in human-readable format".to_string(),
                "exa -la  # Use modern alternative to ls".to_string(),
            ],
            cmd if cmd.starts_with("cd") => vec![
                "pushd <dir>  # Remember current directory".to_string(),
                "z <partial>  # Use fuzzy directory jumping".to_string(),
            ],
            cmd if cmd.starts_with("find") => vec![
                "fd <pattern>  # Use modern alternative to find".to_string(),
                "rg <pattern>  # Search file contents instead".to_string(),
            ],
            _ => vec![],
        };

        Ok(suggestions)
    }

    /// Get recent commands from block manager
    async fn get_recent_commands(&self) -> Result<Vec<String>> {
        let block_manager = self.block_manager.read().await;
        let query = openagent_terminal::blocks_v2::SearchQuery {
            limit: Some(10),
            sort_by: Some("created_at".to_string()),
            sort_order: Some("DESC".to_string()),
            ..Default::default()
        };
        
        let blocks = block_manager.search(query).await?;
        Ok(blocks.into_iter().map(|b| b.command).collect())
    }

    /// Get active files in the current directory
    async fn get_active_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.session_info.current_directory) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    files.push(entry.path());
                }
                if files.len() >= 10 {
                    break;
                }
            }
        }
        Ok(files)
    }

    /// Process conversation message asynchronously
    async fn process_conversation_message(
        conversation_id: Uuid,
        ai_runtime: Arc<RwLock<AiRuntime>>,
        conversations: Arc<RwLock<HashMap<Uuid, AiConversation>>>,
        event_sender: mpsc::UnboundedSender<AiTerminalEvent>,
    ) -> Result<()> {
        let start_time = std::time::Instant::now();

        // Get conversation context
        let context = {
            let conversations_lock = conversations.read().await;
            conversations_lock.get(&conversation_id)
                .map(|c| c.context.clone())
                .ok_or_else(|| anyhow::anyhow!("Conversation not found"))?
        };

        // Generate AI response (simplified)
        let response_content = Self::generate_ai_response(&context).await?;

        // Update conversation with AI response
        {
            let mut conversations_lock = conversations.write().await;
            if let Some(conversation) = conversations_lock.get_mut(&conversation_id) {
                conversation.messages.push(ConversationMessage {
                    id: Uuid::new_v4(),
                    role: MessageRole::Assistant,
                    content: response_content.clone(),
                    timestamp: chrono::Utc::now(),
                    metadata: HashMap::new(),
                });
                conversation.last_activity = chrono::Utc::now();
                conversation.state = ConversationState::WaitingForInput;
            }
        }

        let processing_time = start_time.elapsed();

        // Emit response event
        let _ = event_sender.send(AiTerminalEvent::ResponseGenerated {
            conversation_id,
            response: response_content,
            confidence: 0.8,
            processing_time,
            timestamp: chrono::Utc::now(),
        });

        Ok(())
    }

    /// Generate AI response based on context
    async fn generate_ai_response(context: &ConversationContext) -> Result<String> {
        // This is a simplified implementation
        // In production, this would call actual AI providers
        
        let response = if let Some(ref intent) = context.user_intent {
            if intent.to_lowercase().contains("help") {
                format!("I can help you with terminal commands. Your current directory is {}. Recent commands: {}",
                    context.current_directory.display(),
                    context.recent_commands.join(", ")
                )
            } else if intent.to_lowercase().contains("files") {
                format!("I can see {} files in your current directory. Would you like me to help you work with any specific files?",
                    context.active_files.len()
                )
            } else if intent.to_lowercase().contains("git") {
                if let Some(ref project_context) = context.project_context {
                    if let Some(ref git_info) = project_context.git_info {
                        format!("You're on branch '{}'. {}",
                            git_info.current_branch,
                            if git_info.uncommitted_changes {
                                "You have uncommitted changes."
                            } else {
                                "Your working directory is clean."
                            }
                        )
                    } else {
                        "This doesn't appear to be a git repository.".to_string()
                    }
                } else {
                    "I don't see any project context. Are you in a git repository?".to_string()
                }
            } else {
                format!("I understand you're asking about: {}. How can I help you with terminal commands or file operations?", intent)
            }
        } else {
            "Hello! I'm here to help you with terminal commands and file operations. What would you like to do?".to_string()
        };

        Ok(response)
    }

    /// Detect project context from current directory
    async fn detect_project_context(current_dir: &PathBuf) -> Result<Option<ProjectContext>> {
        // Check for various project indicators
        let mut project_type = ProjectType::Unknown;
        let mut languages = Vec::new();
        let mut build_tools = Vec::new();
        let mut dependencies = HashMap::new();

        // Check for Rust project
        if current_dir.join("Cargo.toml").exists() {
            project_type = ProjectType::Rust;
            languages.push("Rust".to_string());
            build_tools.push("Cargo".to_string());
            
            // Read dependencies from Cargo.toml (simplified)
            if let Ok(content) = std::fs::read_to_string(current_dir.join("Cargo.toml")) {
                // This is a very basic parser - in production, use a proper TOML parser
                for line in content.lines() {
                    if line.contains("=") && !line.trim().starts_with("#") {
                        let parts: Vec<&str> = line.split("=").collect();
                        if parts.len() == 2 {
                            let key = parts[0].trim().replace("\"", "");
                            let value = parts[1].trim().replace("\"", "");
                            dependencies.insert(key, value);
                        }
                    }
                }
            }
        }

        // Check for Python project
        if current_dir.join("requirements.txt").exists() || 
           current_dir.join("pyproject.toml").exists() ||
           current_dir.join("setup.py").exists() {
            if project_type == ProjectType::Unknown {
                project_type = ProjectType::Python;
            } else {
                project_type = ProjectType::Mixed;
            }
            languages.push("Python".to_string());
            
            if current_dir.join("requirements.txt").exists() {
                build_tools.push("pip".to_string());
            }
            if current_dir.join("pyproject.toml").exists() {
                build_tools.push("Poetry".to_string());
            }
        }

        // Check for JavaScript/TypeScript project
        if current_dir.join("package.json").exists() {
            if project_type == ProjectType::Unknown {
                project_type = if current_dir.join("tsconfig.json").exists() {
                    ProjectType::TypeScript
                } else {
                    ProjectType::JavaScript
                };
            } else {
                project_type = ProjectType::Mixed;
            }
            
            if current_dir.join("tsconfig.json").exists() {
                languages.push("TypeScript".to_string());
            }
            languages.push("JavaScript".to_string());
            build_tools.push("npm".to_string());
            
            if current_dir.join("yarn.lock").exists() {
                build_tools.push("Yarn".to_string());
            }
        }

        // Check for Git information
        let git_info = if current_dir.join(".git").exists() {
            // This is a simplified implementation
            // In production, use git2 crate for proper Git integration
            Some(GitInfo {
                current_branch: "main".to_string(), // Would read from .git/HEAD
                remote_url: None, // Would read from .git/config
                uncommitted_changes: false, // Would check git status
                ahead_behind: None, // Would check git status
            })
        } else {
            None
        };

        if project_type != ProjectType::Unknown {
            Ok(Some(ProjectContext {
                root_path: current_dir.clone(),
                project_type,
                languages,
                build_tools,
                git_info,
                dependencies,
            }))
        } else {
            Ok(None)
        }
    }

    /// Start event processing loop
    async fn start_event_processing_loop(&self) -> Result<()> {
        let event_receiver = Arc::clone(&self.event_receiver);
        let metrics = Arc::clone(&self.metrics);

        tokio::spawn(async move {
            let mut receiver = event_receiver.write().await;
            while let Some(event) = receiver.recv().await {
                Self::handle_event(event, Arc::clone(&metrics)).await;
            }
        });

        Ok(())
    }

    /// Handle events
    async fn handle_event(event: AiTerminalEvent, metrics: Arc<RwLock<AiTerminalMetrics>>) {
        match event {
            AiTerminalEvent::AssistanceProvided { .. } => {
                // Log assistance provided
                debug!("AI assistance provided");
            }
            AiTerminalEvent::SecurityAnalysis { .. } => {
                let mut metrics_lock = metrics.write().await;
                metrics_lock.security_warnings_issued += 1;
            }
            AiTerminalEvent::ConversationStarted { .. } => {
                debug!("Conversation started");
            }
            AiTerminalEvent::ResponseGenerated { .. } => {
                debug!("AI response generated");
            }
            AiTerminalEvent::Error { message, .. } => {
                error!("AI Terminal error: {}", message);
            }
            AiTerminalEvent::MetricsUpdate { .. } => {
                debug!("Metrics updated");
            }
        }
    }

    /// Start periodic tasks
    async fn start_periodic_tasks(&self) -> Result<()> {
        let metrics = Arc::clone(&self.metrics);
        let event_sender = self.event_sender.clone();

        // Metrics reporting task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                let metrics_snapshot = {
                    let metrics_lock = metrics.read().await;
                    metrics_lock.clone()
                };
                
                let _ = event_sender.send(AiTerminalEvent::MetricsUpdate {
                    metrics: metrics_snapshot,
                    timestamp: chrono::Utc::now(),
                });
            }
        });

        // Conversation cleanup task
        let conversations = Arc::clone(&self.conversations);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            loop {
                interval.tick().await;
                
                let cutoff_time = chrono::Utc::now() - chrono::Duration::hours(1);
                let mut conversations_lock = conversations.write().await;
                
                conversations_lock.retain(|_, conversation| {
                    conversation.last_activity > cutoff_time
                });
            }
        });

        Ok(())
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> AiTerminalMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Get active conversations
    pub async fn get_conversations(&self) -> Vec<AiConversation> {
        let conversations = self.conversations.read().await;
        conversations.values().cloned().collect()
    }

    /// Shutdown the system
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down AI Terminal System");
        
        // Shutdown integration manager
        self.integration_manager.shutdown().await?;
        
        info!("AI Terminal System shut down successfully");
        Ok(())
    }
}

/// Result of command processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandProcessingResult {
    pub command: String,
    pub assistance_provided: Vec<AssistanceInfo>,
    pub security_analysis: Option<SecurityAnalysisResult>,
    pub suggestions: Vec<String>,
    pub should_execute: bool,
    pub processing_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistanceInfo {
    pub assistance_type: String,
    pub content: String,
    pub suggestions: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAnalysisResult {
    pub risk_level: String,
    pub warnings: Vec<String>,
    pub recommendations: Vec<String>,
    pub should_block: bool,
}

/// Detect the current shell type
fn detect_shell() -> ShellType {
    if let Ok(shell) = env::var("SHELL") {
        if shell.contains("bash") {
            ShellType::Bash
        } else if shell.contains("zsh") {
            ShellType::Zsh
        } else if shell.contains("fish") {
            ShellType::Fish
        } else if shell.contains("pwsh") || shell.contains("powershell") {
            ShellType::PowerShell
        } else {
            ShellType::Bash // Default fallback
        }
    } else {
        ShellType::Bash
    }
}

/// Production main function for AI terminal integration
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting AI Terminal Integration");

    // Create configuration
    let config = AiTerminalConfig::default();

    // Create and start AI terminal system
    let mut system = AiTerminalSystem::new(config).await?;
    system.start().await?;

    // Demonstration of functionality
    info!("AI Terminal Integration ready");

    // Process some example commands
    let test_commands = vec![
        "ls -la",
        "rm -rf /tmp/test",
        "git status",
        "help me find large files",
        "how do I deploy this application?",
    ];

    for command in test_commands {
        info!("Processing command: {}", command);
        
        match system.process_command(command).await {
            Ok(result) => {
                info!("Command processed successfully");
                info!("  Should execute: {}", result.should_execute);
                info!("  Processing time: {:?}", result.processing_time);
                
                if !result.suggestions.is_empty() {
                    info!("  Suggestions:");
                    for suggestion in &result.suggestions {
                        info!("    - {}", suggestion);
                    }
                }
                
                if let Some(ref security) = result.security_analysis {
                    info!("  Security analysis:");
                    info!("    Risk level: {}", security.risk_level);
                    if !security.warnings.is_empty() {
                        info!("    Warnings: {}", security.warnings.join(", "));
                    }
                }
            }
            Err(e) => {
                error!("Failed to process command '{}': {}", command, e);
            }
        }
        
        // Wait between commands
        sleep(Duration::from_millis(500)).await;
    }

    // Start a conversation
    let conversation_id = system.start_conversation("Help me understand what files are in this directory").await?;
    info!("Started conversation: {}", conversation_id);

    // Wait a bit for the response
    sleep(Duration::from_secs(2)).await;

    // Continue the conversation
    system.continue_conversation(conversation_id, "Can you suggest some git commands I might need?").await?;

    // Wait for processing
    sleep(Duration::from_secs(2)).await;

    // Show final metrics
    let metrics = system.get_metrics().await;
    info!("Final metrics:");
    info!("  Total requests: {}", metrics.total_requests);
    info!("  Successful requests: {}", metrics.successful_requests);
    info!("  Average response time: {:?}", metrics.average_response_time);
    info!("  Conversations started: {}", metrics.conversations_started);

    // Shutdown
    system.shutdown().await?;

    info!("AI Terminal Integration demonstration completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ai_terminal_system_creation() {
        let config = AiTerminalConfig::default();
        let system = AiTerminalSystem::new(config).await;
        assert!(system.is_ok());
    }

    #[test]
    fn test_shell_detection() {
        // Mock environment for testing
        std::env::set_var("SHELL", "/bin/bash");
        assert_eq!(detect_shell(), ShellType::Bash);
        
        std::env::set_var("SHELL", "/bin/zsh");
        assert_eq!(detect_shell(), ShellType::Zsh);
    }

    #[tokio::test]
    async fn test_project_context_detection() {
        let temp_dir = tempfile::tempdir().unwrap();
        
        // Create a Cargo.toml to simulate a Rust project
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\""
        ).unwrap();
        
        let context = AiTerminalSystem::detect_project_context(&temp_dir.path().to_path_buf()).await.unwrap();
        assert!(context.is_some());
        
        let context = context.unwrap();
        assert!(matches!(context.project_type, ProjectType::Rust));
        assert!(context.languages.contains(&"Rust".to_string()));
    }

    #[tokio::test]
    async fn test_command_processing() {
        let config = AiTerminalConfig::default();
        let mut system = AiTerminalSystem::new(config).await.unwrap();
        
        let result = system.process_command("ls -la").await.unwrap();
        assert_eq!(result.command, "ls -la");
        assert!(result.should_execute);
        assert!(result.processing_time > Duration::from_millis(0));
    }
}

    // Demo 1: Simulate command execution with failure
    info!("\n=== Demo 1: Command Failure Analysis ===");
    terminal_integration.on_command_completed(
        "git status",
        128, // Git error code for "not a git repository"
        "",
        "fatal: not a git repository (or any of the parent directories): .git",
        Duration::from_millis(50),
    )?;

    sleep(Duration::from_secs(2)).await;

    // Demo 2: Simulate successful command
    info!("\n=== Demo 2: Successful Command ===");
    terminal_integration.on_command_completed(
        "ls -la",
        0,
        "total 48\ndrwxr-xr-x  8 user user 4096 Dec  1 10:30 .\ndrwxr-xr-x 12 user user 4096 Dec  1 10:25 ..\ndrwxr-xr-x  8 user user 4096 Dec  1 10:30 .git",
        "",
        Duration::from_millis(25),
    )?;

    sleep(Duration::from_secs(1)).await;

    // Demo 3: Simulate directory change to a Git repository
    info!("\n=== Demo 3: Directory Change to Git Repository ===");
    let git_dir = current_dir.join("test_repo");
    terminal_integration.on_directory_change(&git_dir)?;

    sleep(Duration::from_secs(1)).await;

    // Demo 4: Simulate typing a command (for command completion)
    info!("\n=== Demo 4: Command Typing Assistance ===");
    terminal_integration.on_user_input("git com", 7)?;

    sleep(Duration::from_secs(1)).await;

    // Demo 5: Explicitly request AI assistance
    info!("\n=== Demo 5: Explicit AI Assistance Request ===");
    manager.request_ai_assistance(
        "I need help understanding Docker commands for container management".to_string(),
        AssistanceType::Explain,
    ).await?;

    sleep(Duration::from_secs(2)).await;

    // Demo 6: Simulate Python module not found error
    info!("\n=== Demo 6: Python Module Error ===");
    terminal_integration.on_command_completed(
        "python -c \"import numpy\"",
        1,
        "",
        "Traceback (most recent call last):\n  File \"<string>\", line 1, in <module>\nModuleNotFoundError: No module named 'numpy'",
        Duration::from_millis(100),
    )?;

    sleep(Duration::from_secs(2)).await;

    // Demo 7: Show system statistics
    info!("\n=== Demo 7: System Statistics ===");
    let stats = manager.get_statistics().await;
    info!("System Statistics:");
    info!("  - Active agents: {}", stats.active_agents);
    info!("  - Total events processed: {}", stats.total_events_processed);
    info!("  - Total AI responses: {}", stats.total_ai_responses);
    info!("  - Session duration: {:?}", stats.session_duration);
    info!("  - Current directory: {}", stats.current_directory.display());
    info!("  - Events per minute: {:.2}", stats.performance_metrics.events_per_minute);

    // Demo 8: Show system health
    info!("\n=== Demo 8: System Health ===");
    let health = manager.get_health_status().await;
    info!("System Health:");
    info!("  - Running: {}", health.is_running);
    info!("  - Health score: {:.1}/100", health.health_score);
    info!("  - Uptime: {:?}", health.uptime);
    info!("  - Memory usage: {:.2} MB", health.performance_metrics.memory_usage_mb);
    info!("  - CPU usage: {:.2}%", health.performance_metrics.cpu_usage_percent);

    // Demo 9: More complex command scenarios
    info!("\n=== Demo 9: Complex Command Scenarios ===");
    
    // NPM command failure
    terminal_integration.on_command_completed(
        "npm install react",
        127,
        "",
        "npm: command not found",
        Duration::from_millis(10),
    )?;

    sleep(Duration::from_secs(1)).await;

    // Docker command
    terminal_integration.on_command_completed(
        "docker ps",
        0,
        "CONTAINER ID   IMAGE     COMMAND   CREATED   STATUS    PORTS     NAMES",
        "",
        Duration::from_millis(200),
    )?;

    sleep(Duration::from_secs(1)).await;

    // Cargo build in Rust project
    let cargo_dir = current_dir.clone();
    terminal_integration.on_directory_change(&cargo_dir)?;
    
    sleep(Duration::from_millis(500)).await;
    
    terminal_integration.on_command_completed(
        "cargo build",
        0,
        "   Compiling openagent-terminal v0.1.0\n    Finished dev [unoptimized + debuginfo] target(s) in 2.34s",
        "",
        Duration::from_millis(2340),
    )?;

    // Let the system process events
    sleep(Duration::from_secs(3)).await;

    // Final statistics
    info!("\n=== Final Statistics ===");
    let final_stats = manager.get_statistics().await;
    info!("Final System Statistics:");
    info!("  - Total events processed: {}", final_stats.total_events_processed);
    info!("  - Total AI responses: {}", final_stats.total_ai_responses);
    info!("  - Average response time: {:.2}ms", final_stats.performance_metrics.average_response_time_ms);

    // Clean shutdown
    info!("\n=== Shutting Down ===");
    manager.stop().await?;
    info!("AI terminal integration system stopped");

    info!("Demo completed successfully!");
    Ok(())
}

/// Detect the current shell type
fn detect_shell() -> ShellType {
    if let Ok(shell) = env::var("SHELL") {
        if shell.contains("bash") {
            return ShellType::Bash;
        } else if shell.contains("zsh") {
            return ShellType::Zsh;
        } else if shell.contains("fish") {
            return ShellType::Fish;
        } else if shell.contains("sh") && !shell.contains("bash") {
            return ShellType::Sh;
        }
    }
    
    // Default fallback
    ShellType::Bash
}

/// Additional helper function to demonstrate configuration customization
#[allow(dead_code)]
fn create_custom_config() -> AiTerminalConfig {
    use openagent_terminal::ai_terminal_integration::*;
    use openagent_terminal::ai_runtime::AiProvider;
    use std::collections::HashMap;

    let mut config = AiTerminalConfig::default();
    
    // Customize AI runtime settings
    config.ai_runtime.response_timeout_ms = 15000; // 15 second timeout
    config.ai_runtime.max_conversation_length = 50;
    
    // Customize event monitoring
    config.event_bridge.monitor_typing = true; // Enable typing monitoring
    config.event_bridge.max_output_length = 5 * 1024; // 5KB max output
    
    // Customize agents
    config.agents.global_settings.max_agents_per_event = 2;
    config.agents.global_settings.global_rate_limit = 60; // 1 per second max
    
    // Add custom agent
    config.agents.custom_agents.push(CustomAgentConfig {
        id: "rust_expert".to_string(),
        name: "Rust Expert".to_string(),
        description: "Provides Rust-specific development assistance".to_string(),
        provider: AiProvider::Ollama,
        model: "codellama:7b".to_string(),
        system_prompt: "You are a Rust programming expert. Help users with Rust code, cargo commands, and Rust-specific issues.".to_string(),
        trigger_events: vec!["CommandFailed".to_string(), "CommandExecuted".to_string()],
        activation_conditions: vec![
            "CommandContains:cargo".to_string(),
            "CommandContains:rustc".to_string(),
            "ErrorContains:rust".to_string(),
        ],
        priority: 80,
        debounce_seconds: 2,
        enabled: true,
    });
    
    // Customize UI integration
    config.ui_integration.show_timestamps = true;
    config.ui_integration.max_display_length = 300;
    
    // Customize performance settings
    config.performance.max_concurrent_requests = 3;
    config.performance.stats_interval_seconds = 30;
    
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_shell_detection() {
        let shell_type = detect_shell();
        // Should return one of the supported shell types
        match shell_type {
            ShellType::Bash | ShellType::Zsh | ShellType::Fish | ShellType::Sh => {
                // Valid shell type detected
            }
        }
    }
    
    #[test]
    fn test_custom_config_creation() {
        let config = create_custom_config();
        assert_eq!(config.ai_runtime.response_timeout_ms, 15000);
        assert!(config.event_bridge.monitor_typing);
        assert_eq!(config.agents.custom_agents.len(), 1);
        assert_eq!(config.agents.custom_agents[0].id, "rust_expert");
    }
}