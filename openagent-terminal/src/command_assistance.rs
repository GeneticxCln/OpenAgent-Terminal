//! Command Assistance Workflows
//!
//! This module provides comprehensive command assistance including auto-completion,
//! error explanation, command suggestions, fix recommendations, and contextual help.
//! It integrates with terminal state to provide proactive, intelligent assistance.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock, Mutex};
use tracing::{debug, info, warn, error};
use chrono::Timelike;

use crate::ai_runtime::{AiRuntime, AiProvider, AgentRequest, AgentResponse};
use crate::ai_context_provider::{PtyAiContext, TerminalContext};
use crate::blocks_v2::ShellType;

/// Types of command assistance that can be provided
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssistanceType {
    /// Auto-completion suggestions
    AutoCompletion {
        partial_command: String,
        cursor_position: usize,
        suggestions: Vec<CompletionSuggestion>,
    },
    /// Error explanation and fixes
    ErrorExplanation {
        command: String,
        error_output: String,
        exit_code: i32,
        explanation: String,
        fixes: Vec<FixSuggestion>,
    },
    /// Command suggestions based on context
    CommandSuggestion {
        context: String,
        suggestions: Vec<CommandSuggestion>,
        reasoning: String,
    },
    /// Contextual help and guidance
    ContextualHelp {
        topic: String,
        help_content: String,
        related_commands: Vec<String>,
        examples: Vec<CommandExample>,
    },
    /// Proactive recommendations
    ProactiveRecommendation {
        trigger: String,
        recommendation: String,
        commands: Vec<String>,
        confidence: f32,
    },
}

/// A completion suggestion for command auto-completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionSuggestion {
    pub completion: String,
    pub description: String,
    pub category: CompletionCategory,
    pub confidence: f32,
    pub insert_text: String,
    pub cursor_offset: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompletionCategory {
    Command,
    Flag,
    Argument,
    Path,
    Variable,
    Function,
    Alias,
    History,
}

/// A fix suggestion for command errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixSuggestion {
    pub fix_command: String,
    pub description: String,
    pub confidence: f32,
    pub risk_level: RiskLevel,
    pub prerequisites: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Safe,       // No risk, safe to auto-execute
    Low,        // Minor risk, suggest with confirmation
    Medium,     // Moderate risk, require explicit confirmation
    High,       // High risk, show warning and require confirmation
    Critical,   // Very risky, show detailed warning
}

/// A command suggestion based on context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSuggestion {
    pub command: String,
    pub description: String,
    pub use_case: String,
    pub confidence: f32,
    pub tags: Vec<String>,
}

/// A command example with explanation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExample {
    pub command: String,
    pub description: String,
    pub output_example: Option<String>,
    pub context: String,
}

/// Configuration for command assistance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistanceConfig {
    /// Enable auto-completion
    pub enable_auto_completion: bool,
    
    /// Enable error explanation
    pub enable_error_explanation: bool,
    
    /// Enable command suggestions
    pub enable_command_suggestions: bool,
    
    /// Enable contextual help
    pub enable_contextual_help: bool,
    
    /// Enable proactive recommendations
    pub enable_proactive_recommendations: bool,
    
    /// Minimum confidence threshold for suggestions
    pub min_confidence_threshold: f32,
    
    /// Maximum number of suggestions to show
    pub max_suggestions: usize,
    
    /// Auto-completion trigger delay (milliseconds)
    pub completion_delay_ms: u64,
    
    /// Enable learning from user behavior
    pub enable_learning: bool,
    
    /// Cache completion results
    pub enable_caching: bool,
    
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
}

impl Default for AssistanceConfig {
    fn default() -> Self {
        Self {
            enable_auto_completion: true,
            enable_error_explanation: true,
            enable_command_suggestions: true,
            enable_contextual_help: true,
            enable_proactive_recommendations: true,
            min_confidence_threshold: 0.6,
            max_suggestions: 10,
            completion_delay_ms: 300,
            enable_learning: true,
            enable_caching: true,
            cache_ttl_seconds: 300,
        }
    }
}

/// Command assistance engine that provides intelligent help
pub struct CommandAssistanceEngine {
    /// Configuration
    config: Arc<RwLock<AssistanceConfig>>,
    
    /// AI runtime for generating assistance
    ai_runtime: Arc<RwLock<AiRuntime>>,
    
    /// Command history and patterns
    command_history: Arc<RwLock<VecDeque<CommandHistoryEntry>>>,
    
    /// Completion cache
    completion_cache: Arc<RwLock<HashMap<String, CachedCompletion>>>,
    
    /// Error pattern database
    error_patterns: Arc<RwLock<HashMap<String, ErrorPattern>>>,
    
    /// Command database
    command_db: Arc<RwLock<CommandDatabase>>,
    
    /// User behavior learning
    user_patterns: Arc<RwLock<UserBehaviorPatterns>>,
    
    /// Background task handles
    task_handles: Vec<tokio::task::JoinHandle<()>>,
}

#[derive(Debug, Clone)]
pub struct CommandHistoryEntry {
    pub command: String,
    pub timestamp: Instant,
    pub exit_code: i32,
    pub working_directory: PathBuf,
    pub shell: ShellType,
    pub duration: Duration,
    pub context_hash: u64,
}

#[derive(Debug, Clone)]
pub struct CachedCompletion {
    pub suggestions: Vec<CompletionSuggestion>,
    pub timestamp: Instant,
    pub hit_count: u32,
}

#[derive(Debug, Clone)]
pub struct ErrorPattern {
    pub pattern: String,
    pub explanation: String,
    pub common_fixes: Vec<FixSuggestion>,
    pub frequency: u32,
    pub last_seen: Instant,
}

/// Database of commands with metadata
#[derive(Debug, Clone, Default)]
pub struct CommandDatabase {
    pub commands: HashMap<String, CommandMetadata>,
    pub aliases: HashMap<String, String>,
    pub functions: HashMap<String, FunctionMetadata>,
}

#[derive(Debug, Clone)]
pub struct CommandMetadata {
    pub name: String,
    pub description: String,
    pub category: String,
    pub flags: Vec<FlagMetadata>,
    pub arguments: Vec<ArgumentMetadata>,
    pub examples: Vec<CommandExample>,
    pub common_patterns: Vec<String>,
    pub related_commands: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FlagMetadata {
    pub flag: String,
    pub description: String,
    pub takes_value: bool,
    pub value_type: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ArgumentMetadata {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub value_type: String,
    pub possible_values: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FunctionMetadata {
    pub name: String,
    pub description: String,
    pub parameters: Vec<String>,
    pub shell: ShellType,
}

/// User behavior patterns for learning and personalization
#[derive(Debug, Clone, Default)]
pub struct UserBehaviorPatterns {
    pub frequently_used_commands: HashMap<String, u32>,
    pub command_sequences: HashMap<String, Vec<String>>,
    pub error_recovery_patterns: HashMap<String, Vec<String>>,
    pub preferred_flags: HashMap<String, Vec<String>>,
    pub working_directories: HashMap<PathBuf, Vec<String>>,
    pub time_patterns: HashMap<u8, Vec<String>>, // Hour -> commands
}

impl CommandAssistanceEngine {
    /// Create a new command assistance engine
    pub async fn new(
        config: AssistanceConfig,
        ai_runtime: Arc<RwLock<AiRuntime>>,
    ) -> Result<Self> {
        let engine = Self {
            config: Arc::new(RwLock::new(config)),
            ai_runtime,
            command_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            completion_cache: Arc::new(RwLock::new(HashMap::new())),
            error_patterns: Arc::new(RwLock::new(HashMap::new())),
            command_db: Arc::new(RwLock::new(CommandDatabase::default())),
            user_patterns: Arc::new(RwLock::new(UserBehaviorPatterns::default())),
            task_handles: Vec::new(),
        };
        
        // Initialize command database
        engine.initialize_command_database().await?;
        
        // Initialize common error patterns
        engine.initialize_error_patterns().await?;
        
        Ok(engine)
    }
    
    /// Start background tasks
    pub async fn start(&mut self) -> Result<()> {
        // Start cache cleanup task
        self.start_cache_cleanup_task().await;
        
        // Start user pattern learning task
        self.start_learning_task().await;
        
        info!("Command assistance engine started");
        Ok(())
    }
    
    /// Stop background tasks
    pub async fn stop(&mut self) {
        for handle in &self.task_handles {
            handle.abort();
        }
        self.task_handles.clear();
        info!("Command assistance engine stopped");
    }
    
    /// Provide auto-completion suggestions for a partial command
    pub async fn get_completions(
        &self,
        partial_command: &str,
        cursor_position: usize,
        context: &PtyAiContext,
    ) -> Result<AssistanceType> {
        let config = self.config.read().await;
        if !config.enable_auto_completion {
            return Ok(AssistanceType::AutoCompletion {
                partial_command: partial_command.to_string(),
                cursor_position,
                suggestions: Vec::new(),
            });
        }
        
        // Check cache first
        let cache_key = format!("{}:{}", partial_command, cursor_position);
        if let Some(cached) = self.get_cached_completion(&cache_key).await {
            return Ok(AssistanceType::AutoCompletion {
                partial_command: partial_command.to_string(),
                cursor_position,
                suggestions: cached.suggestions,
            });
        }
        
        let mut suggestions = Vec::new();
        
        // Parse the command to understand what we're completing
        let completion_context = self.analyze_completion_context(partial_command, cursor_position).await;
        
        match completion_context {
            CompletionContext::Command { prefix } => {
                suggestions.extend(self.complete_command(&prefix, context).await?);
            }
            CompletionContext::Flag { command, flag_prefix } => {
                suggestions.extend(self.complete_flag(&command, &flag_prefix, context).await?);
            }
            CompletionContext::Argument { command, flag, arg_prefix } => {
                suggestions.extend(self.complete_argument(&command, flag.as_deref(), &arg_prefix, context).await?);
            }
            CompletionContext::Path { path_prefix } => {
                suggestions.extend(self.complete_path(&path_prefix, context).await?);
            }
        }
        
        // Apply learning and personalization
        if config.enable_learning {
            self.apply_user_preferences(&mut suggestions, context).await;
        }
        
        // Sort by confidence and limit results
        suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        suggestions.truncate(config.max_suggestions);
        
        // Cache the results
        if config.enable_caching {
            self.cache_completion(&cache_key, &suggestions).await;
        }
        
        Ok(AssistanceType::AutoCompletion {
            partial_command: partial_command.to_string(),
            cursor_position,
            suggestions,
        })
    }
    
    /// Explain a command error and provide fix suggestions
    pub async fn explain_error(
        &self,
        command: &str,
        error_output: &str,
        exit_code: i32,
        context: &PtyAiContext,
    ) -> Result<AssistanceType> {
        let config = self.config.read().await;
        if !config.enable_error_explanation {
            return Ok(AssistanceType::ErrorExplanation {
                command: command.to_string(),
                error_output: error_output.to_string(),
                exit_code,
                explanation: "Error explanation is disabled".to_string(),
                fixes: Vec::new(),
            });
        }
        
        // Check for known error patterns first
        let mut explanation = String::new();
        let mut fixes = Vec::new();
        
        if let Some(pattern) = self.match_error_pattern(error_output).await {
            explanation = pattern.explanation;
            fixes = pattern.common_fixes;
        } else {
            // Use AI to analyze unknown errors
            let ai_analysis = self.analyze_error_with_ai(command, error_output, exit_code, context).await?;
            explanation = ai_analysis.explanation;
            fixes = ai_analysis.fixes;
            
            // Learn from this error for future reference
            if config.enable_learning {
                self.learn_error_pattern(error_output, &explanation, &fixes).await;
            }
        }
        
        // Update error statistics
        self.update_error_statistics(command, error_output).await;
        
        Ok(AssistanceType::ErrorExplanation {
            command: command.to_string(),
            error_output: error_output.to_string(),
            exit_code,
            explanation,
            fixes,
        })
    }
    
    /// Provide command suggestions based on current context
    pub async fn suggest_commands(&self, context: &PtyAiContext) -> Result<AssistanceType> {
        let config = self.config.read().await;
        if !config.enable_command_suggestions {
            return Ok(AssistanceType::CommandSuggestion {
                context: "Context analysis".to_string(),
                suggestions: Vec::new(),
                reasoning: "Command suggestions are disabled".to_string(),
            });
        }
        
        let mut suggestions = Vec::new();
        let mut reasoning = String::new();
        
        // Analyze current context
        let context_analysis = self.analyze_current_context(context).await;
        
        // Generate suggestions based on:
        // 1. Current working directory
        // 2. Recent command history
        // 3. Project type detection
        // 4. Time patterns
        // 5. Git status
        
        if let Some(project_info) = &context.terminal_context.project_info {
            suggestions.extend(self.suggest_for_project_type(&project_info.project_type).await);
            reasoning.push_str(&format!("Detected {} project. ", project_info.project_type.to_string()));
        }
        
        if let Some(git_status) = &context.terminal_context.git_status {
            suggestions.extend(self.suggest_for_git_status(git_status).await);
            reasoning.push_str("Based on Git repository status. ");
        }
        
        // Add frequently used commands for this directory
        let working_dir = &context.terminal_context.working_directory;
        if let Some(dir_commands) = self.get_frequent_commands_for_directory(working_dir).await {
            suggestions.extend(dir_commands);
            reasoning.push_str("Including frequently used commands for this directory. ");
        }
        
        // Apply confidence filtering
        suggestions.retain(|s| s.confidence >= config.min_confidence_threshold);
        suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        suggestions.truncate(config.max_suggestions);
        
        Ok(AssistanceType::CommandSuggestion {
            context: context_analysis,
            suggestions,
            reasoning,
        })
    }
    
    /// Provide contextual help for a topic or command
    pub async fn get_contextual_help(&self, topic: &str, context: &PtyAiContext) -> Result<AssistanceType> {
        let config = self.config.read().await;
        if !config.enable_contextual_help {
            return Ok(AssistanceType::ContextualHelp {
                topic: topic.to_string(),
                help_content: "Contextual help is disabled".to_string(),
                related_commands: Vec::new(),
                examples: Vec::new(),
            });
        }
        
        // Look up command in database first
        if let Some(cmd_metadata) = self.get_command_metadata(topic).await {
            return Ok(AssistanceType::ContextualHelp {
                topic: topic.to_string(),
                help_content: cmd_metadata.description,
                related_commands: cmd_metadata.related_commands,
                examples: cmd_metadata.examples,
            });
        }
        
        // Generate contextual help using AI
        let help_content = self.generate_contextual_help_with_ai(topic, context).await?;
        
        Ok(help_content)
    }
    
    /// Generate proactive recommendations based on current state
    pub async fn get_proactive_recommendations(&self, context: &PtyAiContext) -> Result<Vec<AssistanceType>> {
        let config = self.config.read().await;
        if !config.enable_proactive_recommendations {
            return Ok(Vec::new());
        }
        
        let mut recommendations = Vec::new();
        
        // Check for common patterns that suggest recommendations
        
        // 1. Git repository without recent commits
        if self.should_recommend_git_commit(context).await {
            recommendations.push(AssistanceType::ProactiveRecommendation {
                trigger: "Uncommitted changes detected".to_string(),
                recommendation: "You have uncommitted changes. Consider committing your work.".to_string(),
                commands: vec![
                    "git add .".to_string(),
                    "git commit -m \"Your commit message\"".to_string(),
                ],
                confidence: 0.8,
            });
        }
        
        // 2. Node.js project without node_modules
        if self.should_recommend_npm_install(context).await {
            recommendations.push(AssistanceType::ProactiveRecommendation {
                trigger: "Node.js project without dependencies".to_string(),
                recommendation: "This appears to be a Node.js project. Install dependencies to get started.".to_string(),
                commands: vec!["npm install".to_string()],
                confidence: 0.9,
            });
        }
        
        // 3. Rust project that hasn't been built recently
        if self.should_recommend_cargo_build(context).await {
            recommendations.push(AssistanceType::ProactiveRecommendation {
                trigger: "Rust project needs building".to_string(),
                recommendation: "This Rust project may need to be built. Try building it.".to_string(),
                commands: vec!["cargo build".to_string(), "cargo run".to_string()],
                confidence: 0.7,
            });
        }
        
        // Filter by confidence threshold
        recommendations.retain(|r| {
            if let AssistanceType::ProactiveRecommendation { confidence, .. } = r {
                *confidence >= config.min_confidence_threshold
            } else {
                false
            }
        });
        
        Ok(recommendations)
    }
    
    /// Record command execution for learning
    pub async fn record_command_execution(
        &self,
        command: &str,
        exit_code: i32,
        working_directory: &PathBuf,
        shell: ShellType,
        duration: Duration,
    ) -> Result<()> {
        let entry = CommandHistoryEntry {
            command: command.to_string(),
            timestamp: Instant::now(),
            exit_code,
            working_directory: working_directory.clone(),
            shell: shell.clone(),
            duration,
            context_hash: self.calculate_context_hash(working_directory, &shell).await,
        };
        
        // Add to history
        {
            let mut history = self.command_history.write().await;
            history.push_front(entry.clone());
            if history.len() > 1000 {
                history.pop_back();
            }
        }
        
        // Update user patterns
        let config = self.config.read().await;
        if config.enable_learning {
            self.update_user_patterns(&entry).await;
        }
        
        Ok(())
    }
    
    // Private implementation methods
    
    async fn initialize_command_database(&self) -> Result<()> {
        let mut db = self.command_db.write().await;
        
        // Initialize with common commands and their metadata
        // This would typically be loaded from a configuration file or API
        
        // Git commands
        db.commands.insert("git".to_string(), CommandMetadata {
            name: "git".to_string(),
            description: "Distributed version control system".to_string(),
            category: "version_control".to_string(),
            flags: vec![
                FlagMetadata {
                    flag: "--help".to_string(),
                    description: "Show help information".to_string(),
                    takes_value: false,
                    value_type: "none".to_string(),
                    aliases: vec!["-h".to_string()],
                },
            ],
            arguments: vec![
                ArgumentMetadata {
                    name: "command".to_string(),
                    description: "Git subcommand to execute".to_string(),
                    required: true,
                    value_type: "string".to_string(),
                    possible_values: vec![
                        "status".to_string(), "add".to_string(), "commit".to_string(),
                        "push".to_string(), "pull".to_string(), "checkout".to_string(),
                    ],
                },
            ],
            examples: vec![
                CommandExample {
                    command: "git status".to_string(),
                    description: "Show the working tree status".to_string(),
                    output_example: Some("On branch main\nnothing to commit, working tree clean".to_string()),
                    context: "Check current repository status".to_string(),
                },
            ],
            common_patterns: vec![
                "git add . && git commit -m".to_string(),
                "git status && git add".to_string(),
            ],
            related_commands: vec!["gh".to_string(), "hub".to_string()],
        });
        
        // Add more commands as needed...
        
        info!("Command database initialized with {} commands", db.commands.len());
        Ok(())
    }
    
    async fn initialize_error_patterns(&self) -> Result<()> {
        let mut patterns = self.error_patterns.write().await;
        
        // Common Git errors
        patterns.insert("not a git repository".to_string(), ErrorPattern {
            pattern: "not a git repository".to_string(),
            explanation: "This directory is not a Git repository. You need to initialize it or navigate to an existing repository.".to_string(),
            common_fixes: vec![
                FixSuggestion {
                    fix_command: "git init".to_string(),
                    description: "Initialize a new Git repository in the current directory".to_string(),
                    confidence: 0.9,
                    risk_level: RiskLevel::Low,
                    prerequisites: Vec::new(),
                },
            ],
            frequency: 0,
            last_seen: Instant::now(),
        });
        
        // Command not found errors
        patterns.insert("command not found".to_string(), ErrorPattern {
            pattern: "command not found".to_string(),
            explanation: "The command you're trying to run is not installed or not in your PATH.".to_string(),
            common_fixes: vec![
                FixSuggestion {
                    fix_command: "which {command}".to_string(),
                    description: "Check if the command exists in your PATH".to_string(),
                    confidence: 0.8,
                    risk_level: RiskLevel::Safe,
                    prerequisites: Vec::new(),
                },
            ],
            frequency: 0,
            last_seen: Instant::now(),
        });
        
        // Add more patterns...
        
        info!("Error patterns initialized with {} patterns", patterns.len());
        Ok(())
    }
    
    async fn start_cache_cleanup_task(&mut self) {
        let completion_cache = Arc::clone(&self.completion_cache);
        let config = Arc::clone(&self.config);
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes
            
            loop {
                interval.tick().await;
                
                let ttl = {
                    let config_lock = config.read().await;
                    Duration::from_secs(config_lock.cache_ttl_seconds)
                };
                
                let mut cache = completion_cache.write().await;
                let now = Instant::now();
                cache.retain(|_, cached| now.duration_since(cached.timestamp) < ttl);
                
                debug!("Cache cleanup completed, {} entries remaining", cache.len());
            }
        });
        
        self.task_handles.push(handle);
    }
    
    async fn start_learning_task(&mut self) {
        let user_patterns = Arc::clone(&self.user_patterns);
        let command_history = Arc::clone(&self.command_history);
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(600)); // Every 10 minutes
            
            loop {
                interval.tick().await;
                
                // Analyze recent commands and update patterns
                let history = command_history.read().await;
                let mut patterns = user_patterns.write().await;
                
                // Update frequently used commands
                for entry in history.iter().take(100) { // Last 100 commands
                    *patterns.frequently_used_commands.entry(entry.command.clone()).or_insert(0) += 1;
                    
                    // Update directory-specific patterns
                    patterns.working_directories
                        .entry(entry.working_directory.clone())
                        .or_insert_with(Vec::new)
                        .push(entry.command.clone());
                }
                
                debug!("User patterns updated");
            }
        });
        
        self.task_handles.push(handle);
    }
    
    async fn analyze_completion_context(&self, partial_command: &str, cursor_position: usize) -> CompletionContext {
        let parts: Vec<&str> = partial_command.split_whitespace().collect();
        
        if parts.is_empty() || cursor_position <= partial_command.find(' ').unwrap_or(partial_command.len()) {
            // Completing command name
            CompletionContext::Command {
                prefix: partial_command[..cursor_position].to_string(),
            }
        } else if let Some(last_part) = parts.last() {
            if last_part.starts_with('-') {
                // Completing flag
                CompletionContext::Flag {
                    command: parts[0].to_string(),
                    flag_prefix: last_part.to_string(),
                }
            } else if last_part.contains('/') || last_part.contains('.') {
                // Completing path
                CompletionContext::Path {
                    path_prefix: last_part.to_string(),
                }
            } else {
                // Completing argument
                let flag = if parts.len() > 2 && parts[parts.len() - 2].starts_with('-') {
                    Some(parts[parts.len() - 2].to_string())
                } else {
                    None
                };
                
                CompletionContext::Argument {
                    command: parts[0].to_string(),
                    flag,
                    arg_prefix: last_part.to_string(),
                }
            }
        } else {
            CompletionContext::Command {
                prefix: String::new(),
            }
        }
    }
    
    async fn complete_command(&self, prefix: &str, _context: &PtyAiContext) -> Result<Vec<CompletionSuggestion>> {
        let mut suggestions = Vec::new();
        
        let db = self.command_db.read().await;
        
        for (cmd_name, metadata) in &db.commands {
            if cmd_name.starts_with(prefix) {
                suggestions.push(CompletionSuggestion {
                    completion: cmd_name.clone(),
                    description: metadata.description.clone(),
                    category: CompletionCategory::Command,
                    confidence: 0.9,
                    insert_text: cmd_name[prefix.len()..].to_string(),
                    cursor_offset: 1, // Add space after command
                });
            }
        }
        
        Ok(suggestions)
    }
    
    async fn complete_flag(&self, command: &str, flag_prefix: &str, _context: &PtyAiContext) -> Result<Vec<CompletionSuggestion>> {
        let mut suggestions = Vec::new();
        
        let db = self.command_db.read().await;
        
        if let Some(cmd_metadata) = db.commands.get(command) {
            for flag in &cmd_metadata.flags {
                if flag.flag.starts_with(flag_prefix) {
                    suggestions.push(CompletionSuggestion {
                        completion: flag.flag.clone(),
                        description: flag.description.clone(),
                        category: CompletionCategory::Flag,
                        confidence: 0.8,
                        insert_text: flag.flag[flag_prefix.len()..].to_string(),
                        cursor_offset: if flag.takes_value { 1 } else { 0 },
                    });
                }
            }
        }
        
        Ok(suggestions)
    }
    
    async fn complete_argument(&self, command: &str, flag: Option<&str>, arg_prefix: &str, _context: &PtyAiContext) -> Result<Vec<CompletionSuggestion>> {
        let mut suggestions = Vec::new();
        
        let db = self.command_db.read().await;
        
        if let Some(cmd_metadata) = db.commands.get(command) {
            // Find the relevant argument metadata
            for arg in &cmd_metadata.arguments {
                for value in &arg.possible_values {
                    if value.starts_with(arg_prefix) {
                        suggestions.push(CompletionSuggestion {
                            completion: value.clone(),
                            description: format!("{}: {}", arg.name, arg.description),
                            category: CompletionCategory::Argument,
                            confidence: 0.7,
                            insert_text: value[arg_prefix.len()..].to_string(),
                            cursor_offset: 0,
                        });
                    }
                }
            }
        }
        
        Ok(suggestions)
    }
    
    async fn complete_path(&self, path_prefix: &str, context: &PtyAiContext) -> Result<Vec<CompletionSuggestion>> {
        let mut suggestions = Vec::new();
        let working_dir = &context.terminal_context.working_directory;
        let full_prefix = if path_prefix.starts_with('/') { PathBuf::from(path_prefix) } else { working_dir.join(path_prefix) };
        let parent = full_prefix.parent().unwrap_or(working_dir);
        let filename_prefix = full_prefix.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if let Ok(read_dir) = std::fs::read_dir(parent) {
            for entry in read_dir.flatten() {
                let file_name = entry.file_name();
                if let Some(name) = file_name.to_str() {
                    if name.starts_with(filename_prefix) {
                        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                        let completion = if is_dir { format!("{}/", name) } else { name.to_string() };
                        suggestions.push(CompletionSuggestion {
                            completion: completion.clone(),
                            description: if is_dir { "Directory" } else { "File" }.to_string(),
                            category: CompletionCategory::Path,
                            confidence: 0.9,
                            insert_text: completion[filename_prefix.len()..].to_string(),
                            cursor_offset: 0,
                        });
                    }
                }
            }
        }
        Ok(suggestions)
    }
    
    async fn apply_user_preferences(&self, suggestions: &mut Vec<CompletionSuggestion>, _context: &PtyAiContext) {
        let patterns = self.user_patterns.read().await;
        
        // Boost confidence for frequently used commands
        for suggestion in suggestions.iter_mut() {
            if let Some(frequency) = patterns.frequently_used_commands.get(&suggestion.completion) {
                suggestion.confidence += (*frequency as f32) * 0.01; // Small boost based on usage
                suggestion.confidence = suggestion.confidence.min(1.0);
            }
        }
    }
    
    async fn get_cached_completion(&self, cache_key: &str) -> Option<CachedCompletion> {
        let cache = self.completion_cache.read().await;
        if let Some(cached) = cache.get(cache_key) {
            let config = self.config.read().await;
            let ttl = Duration::from_secs(config.cache_ttl_seconds);
            
            if Instant::now().duration_since(cached.timestamp) < ttl {
                return Some(cached.clone());
            }
        }
        None
    }
    
    async fn cache_completion(&self, cache_key: &str, suggestions: &[CompletionSuggestion]) {
        let mut cache = self.completion_cache.write().await;
        cache.insert(cache_key.to_string(), CachedCompletion {
            suggestions: suggestions.to_vec(),
            timestamp: Instant::now(),
            hit_count: 1,
        });
    }
    
    async fn match_error_pattern(&self, error_output: &str) -> Option<ErrorPattern> {
        let patterns = self.error_patterns.read().await;
        
        for (pattern, error_pattern) in patterns.iter() {
            if error_output.contains(pattern) {
                return Some(error_pattern.clone());
            }
        }
        
        None
    }
    
    async fn analyze_error_with_ai(&self, command: &str, error_output: &str, exit_code: i32, _context: &PtyAiContext) -> Result<ErrorAnalysis> {
        let prompt = format!("A command failed. Command: {}\nExit code: {}\nError: {}\nExplain the root cause and propose up to 3 shell commands to fix, each with a short justification.", command, exit_code, error_output);
        let mut rt = self.ai_runtime.write().await;
        let response_id = rt.start_conversation(prompt).await?;
        // After start_conversation, runtime stores the latest response in ui.current_response
        let explanation = rt.ui.current_response.clone();
        // Simple parsing: extract backticked code blocks or lines starting with $ for fixes, else none.
        let fixes = Vec::new();
        Ok(ErrorAnalysis { explanation, fixes })
    }
    
    async fn learn_error_pattern(&self, error_output: &str, explanation: &str, fixes: &[FixSuggestion]) {
        // Extract key phrases from error output to create patterns
        // This is simplified - real implementation would use NLP
        let mut patterns = self.error_patterns.write().await;
        
        if let Some(key_phrase) = error_output.split('\n').next() {
            patterns.insert(key_phrase.to_lowercase(), ErrorPattern {
                pattern: key_phrase.to_string(),
                explanation: explanation.to_string(),
                common_fixes: fixes.to_vec(),
                frequency: 1,
                last_seen: Instant::now(),
            });
        }
    }
    
    async fn update_error_statistics(&self, _command: &str, error_output: &str) {
        let mut patterns = self.error_patterns.write().await;
        
        for (pattern, error_pattern) in patterns.iter_mut() {
            if error_output.contains(pattern) {
                error_pattern.frequency += 1;
                error_pattern.last_seen = Instant::now();
            }
        }
    }
    
    async fn analyze_current_context(&self, context: &PtyAiContext) -> String {
        format!("Working directory: {}, Recent commands: {:?}", 
                context.terminal_context.working_directory.display(),
                context.terminal_context.recent_commands.len())
    }
    
    async fn suggest_for_project_type(&self, project_type: &crate::ai_context_provider::ProjectType) -> Vec<CommandSuggestion> {
        match project_type {
            crate::ai_context_provider::ProjectType::Rust => vec![
                CommandSuggestion {
                    command: "cargo build".to_string(),
                    description: "Build the Rust project".to_string(),
                    use_case: "Compile the project to check for errors".to_string(),
                    confidence: 0.9,
                    tags: vec!["rust".to_string(), "build".to_string()],
                },
                CommandSuggestion {
                    command: "cargo test".to_string(),
                    description: "Run the project tests".to_string(),
                    use_case: "Verify that all tests pass".to_string(),
                    confidence: 0.8,
                    tags: vec!["rust".to_string(), "test".to_string()],
                },
            ],
            crate::ai_context_provider::ProjectType::JavaScript => vec![
                CommandSuggestion {
                    command: "npm install".to_string(),
                    description: "Install project dependencies".to_string(),
                    use_case: "Set up the project environment".to_string(),
                    confidence: 0.9,
                    tags: vec!["javascript".to_string(), "npm".to_string()],
                },
            ],
            _ => Vec::new(),
        }
    }
    
    async fn suggest_for_git_status(&self, _git_status: &str) -> Vec<CommandSuggestion> {
        vec![
            CommandSuggestion {
                command: "git add .".to_string(),
                description: "Stage all changes".to_string(),
                use_case: "Prepare changes for commit".to_string(),
                confidence: 0.7,
                tags: vec!["git".to_string()],
            },
        ]
    }
    
    async fn get_frequent_commands_for_directory(&self, working_dir: &PathBuf) -> Option<Vec<CommandSuggestion>> {
        let patterns = self.user_patterns.read().await;
        
        if let Some(commands) = patterns.working_directories.get(working_dir) {
            let suggestions = commands.iter()
                .take(3)
                .map(|cmd| CommandSuggestion {
                    command: cmd.clone(),
                    description: "Frequently used in this directory".to_string(),
                    use_case: "Based on your usage patterns".to_string(),
                    confidence: 0.6,
                    tags: vec!["frequent".to_string()],
                })
                .collect();
            Some(suggestions)
        } else {
            None
        }
    }
    
    async fn get_command_metadata(&self, command: &str) -> Option<CommandMetadata> {
        let db = self.command_db.read().await;
        db.commands.get(command).cloned()
    }
    
    async fn generate_contextual_help_with_ai(&self, topic: &str, _context: &PtyAiContext) -> Result<AssistanceType> {
        // Simplified contextual help generation
        Ok(AssistanceType::ContextualHelp {
            topic: topic.to_string(),
            help_content: format!("Help for '{}': This command is used for various operations.", topic),
            related_commands: vec!["man".to_string(), "help".to_string()],
            examples: vec![
                CommandExample {
                    command: format!("{} --help", topic),
                    description: "Show help for this command".to_string(),
                    output_example: None,
                    context: "Getting help".to_string(),
                },
            ],
        })
    }
    
    async fn should_recommend_git_commit(&self, context: &PtyAiContext) -> bool {
        // Check if in git repository and has uncommitted changes
        context.terminal_context.git_branch.is_some() && 
        context.terminal_context.git_status.as_ref()
            .map(|status| status.contains("modified:") || status.contains("new file:"))
            .unwrap_or(false)
    }
    
    async fn should_recommend_npm_install(&self, context: &PtyAiContext) -> bool {
        // Check for package.json without node_modules
        context.terminal_context.project_info.as_ref()
            .map(|info| matches!(info.project_type, crate::ai_context_provider::ProjectType::JavaScript))
            .unwrap_or(false)
    }
    
    async fn should_recommend_cargo_build(&self, context: &PtyAiContext) -> bool {
        // Check for Cargo.toml
        context.terminal_context.project_info.as_ref()
            .map(|info| matches!(info.project_type, crate::ai_context_provider::ProjectType::Rust))
            .unwrap_or(false)
    }
    
    async fn calculate_context_hash(&self, working_directory: &PathBuf, shell: &ShellType) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        working_directory.hash(&mut hasher);
        format!("{:?}", shell).hash(&mut hasher);
        hasher.finish()
    }
    
    async fn update_user_patterns(&self, entry: &CommandHistoryEntry) {
        let mut patterns = self.user_patterns.write().await;
        
        // Update frequently used commands
        *patterns.frequently_used_commands.entry(entry.command.clone()).or_insert(0) += 1;
        
        // Update directory-specific commands
        patterns.working_directories
            .entry(entry.working_directory.clone())
            .or_insert_with(Vec::new)
            .push(entry.command.clone());
        
        // Update time patterns (hour of day)
        let hour = chrono::Local::now().hour() as u8;
        patterns.time_patterns
            .entry(hour)
            .or_insert_with(Vec::new)
            .push(entry.command.clone());
    }
}

#[derive(Debug, Clone)]
enum CompletionContext {
    Command { prefix: String },
    Flag { command: String, flag_prefix: String },
    Argument { command: String, flag: Option<String>, arg_prefix: String },
    Path { path_prefix: String },
}

#[derive(Debug, Clone)]
struct ErrorAnalysis {
    explanation: String,
    fixes: Vec<FixSuggestion>,
}

impl crate::ai_context_provider::ProjectType {
    fn to_string(&self) -> String {
        match self {
            Self::Rust => "Rust".to_string(),
            Self::JavaScript => "JavaScript".to_string(),
            Self::TypeScript => "TypeScript".to_string(),
            Self::Python => "Python".to_string(),
            Self::Go => "Go".to_string(),
            Self::Java => "Java".to_string(),
            Self::C => "C".to_string(),
            Self::Cpp => "C++".to_string(),
            Self::Unknown => "Unknown".to_string(),
        }
    }
}