//! AI Event Integration System
//!
//! This module provides enterprise-ready event-driven architecture for connecting
//! AI agents to terminal events, enabling real-time AI assistance and context-aware
//! responses to user actions and system state changes.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, broadcast, RwLock, Mutex};
use tracing::{debug, info, warn, error};
use chrono::Timelike;

use crate::ai_runtime::{AiRuntime, AiProvider, AiProposal, AgentRequest, AgentResponse};
use crate::ai_context_provider::{PtyAiContext, TerminalContext};
use crate::blocks_v2::{BlockId, BlockRecord, ShellType};
use crate::event::{Event, EventType};

/// Types of terminal events that AI agents can respond to
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminalEventType {
    /// Command was executed
    CommandExecuted {
        command: String,
        exit_code: i32,
        output: String,
        error_output: String,
        duration_ms: u64,
        working_directory: PathBuf,
        shell: ShellType,
    },
    /// Command failed with error
    CommandFailed {
        command: String,
        error: String,
        exit_code: i32,
        working_directory: PathBuf,
    },
    /// Directory changed
    DirectoryChanged {
        old_path: PathBuf,
        new_path: PathBuf,
    },
    /// User typed a command (before execution)
    CommandTyped {
        partial_command: String,
        cursor_position: usize,
        working_directory: PathBuf,
    },
    /// Terminal session started
    SessionStarted {
        shell: ShellType,
        working_directory: PathBuf,
    },
    /// File content changed in workspace
    FileChanged {
        path: PathBuf,
        change_type: FileChangeType,
    },
    /// Git operation detected
    GitOperation {
        operation: GitOperationType,
        branch: Option<String>,
        working_directory: PathBuf,
    },
    /// User requested AI assistance explicitly
    AiAssistanceRequested {
        context: String,
        assistance_type: AssistanceType,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
    Renamed { old_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GitOperationType {
    Commit,
    Push,
    Pull,
    Checkout,
    Merge,
    Rebase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssistanceType {
    Explain,
    Fix,
    Suggest,
    Complete,
    Optimize,
}

/// AI agent trigger conditions
#[derive(Debug, Clone)]
pub struct AgentTrigger {
    /// Types of events this agent responds to
    pub event_types: Vec<TerminalEventType>,
    /// Minimum time between activations (debouncing)
    pub debounce_duration: Duration,
    /// Maximum number of events to process per minute
    pub rate_limit: u32,
    /// Conditions that must be met for activation
    pub activation_conditions: Vec<ActivationCondition>,
    /// Priority level (higher = more important)
    pub priority: u8,
}

#[derive(Debug, Clone)]
pub enum ActivationCondition {
    /// Only activate if exit code matches
    ExitCodeEquals(i32),
    /// Only activate if command contains pattern
    CommandContains(String),
    /// Only activate if error output contains pattern
    ErrorContains(String),
    /// Only activate if working directory matches pattern
    DirectoryMatches(String),
    /// Only activate if file extension matches
    FileExtensionMatches(String),
    /// Only activate during certain times
    TimeWindow { start: u8, end: u8 }, // Hours 0-23
    /// Only activate if system resources are available
    ResourcesAvailable,
}

/// AI agent that can respond to terminal events
#[derive(Debug, Clone)]
pub struct AiAgent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub provider: AiProvider,
    pub model: String,
    pub trigger: AgentTrigger,
    pub system_prompt: String,
    pub enabled: bool,
    pub last_activation: Option<Instant>,
    pub activation_count: u32,
}

impl AiAgent {
    /// Create a new AI agent with default configuration
    pub fn new(id: String, name: String, provider: AiProvider) -> Self {
        Self {
            id,
            name,
            description: String::new(),
            provider,
            model: "default".to_string(),
            trigger: AgentTrigger {
                event_types: Vec::new(),
                debounce_duration: Duration::from_secs(5),
                rate_limit: 60,
                activation_conditions: Vec::new(),
                priority: 50,
            },
            system_prompt: "You are a helpful AI assistant for terminal operations.".to_string(),
            enabled: true,
            last_activation: None,
            activation_count: 0,
        }
    }

    /// Check if this agent should respond to the given event
    pub fn should_activate(&self, event: &TerminalEventType, context: &PtyAiContext) -> bool {
        if !self.enabled {
            return false;
        }

        // Check debouncing
        if let Some(last) = self.last_activation {
            if last.elapsed() < self.trigger.debounce_duration {
                return false;
            }
        }

        // Check rate limiting (simple per-minute check)
        // In a real implementation, this would use a sliding window
        
        // Check event type matching
        let event_matches = self.trigger.event_types.iter().any(|trigger_event| {
            std::mem::discriminant(trigger_event) == std::mem::discriminant(event)
        });

        if !event_matches {
            return false;
        }

        // Check activation conditions
        for condition in &self.trigger.activation_conditions {
            if !self.check_condition(condition, event, context) {
                return false;
            }
        }

        true
    }

    /// Check if a specific condition is met
    fn check_condition(&self, condition: &ActivationCondition, event: &TerminalEventType, _context: &PtyAiContext) -> bool {
        match condition {
            ActivationCondition::ExitCodeEquals(expected) => {
                match event {
                    TerminalEventType::CommandExecuted { exit_code, .. } |
                    TerminalEventType::CommandFailed { exit_code, .. } => *exit_code == *expected,
                    _ => false,
                }
            }
            ActivationCondition::CommandContains(pattern) => {
                match event {
                    TerminalEventType::CommandExecuted { command, .. } |
                    TerminalEventType::CommandFailed { command, .. } |
                    TerminalEventType::CommandTyped { partial_command: command, .. } => {
                        command.contains(pattern)
                    }
                    _ => false,
                }
            }
            ActivationCondition::ErrorContains(pattern) => {
                match event {
                    TerminalEventType::CommandExecuted { error_output, .. } => {
                        error_output.contains(pattern)
                    }
                    TerminalEventType::CommandFailed { error, .. } => {
                        error.contains(pattern)
                    }
                    _ => false,
                }
            }
            ActivationCondition::DirectoryMatches(pattern) => {
                match event {
                    TerminalEventType::CommandExecuted { working_directory, .. } |
                    TerminalEventType::CommandFailed { working_directory, .. } |
                    TerminalEventType::CommandTyped { working_directory, .. } |
                    TerminalEventType::DirectoryChanged { new_path: working_directory, .. } => {
                        working_directory.to_string_lossy().contains(pattern)
                    }
                    _ => false,
                }
            }
            ActivationCondition::FileExtensionMatches(ext) => {
                match event {
                    TerminalEventType::FileChanged { path, .. } => {
                        path.extension().and_then(|e| e.to_str()).map_or(false, |e| e == ext)
                    }
                    _ => false,
                }
            }
            ActivationCondition::TimeWindow { start, end } => {
                let now = chrono::Local::now().hour() as u8;
                if start <= end {
                    now >= *start && now <= *end
                } else {
                    // Handle overnight window (e.g., 22-6)
                    now >= *start || now <= *end
                }
            }
            ActivationCondition::ResourcesAvailable => {
                // Simple check - in production would check CPU, memory, etc.
                true
            }
        }
    }

    /// Generate AI response for the given event
    pub async fn process_event(&mut self, event: &TerminalEventType, context: &PtyAiContext, ai_runtime: &mut AiRuntime) -> Result<AgentResponse> {
        self.last_activation = Some(Instant::now());
        self.activation_count += 1;

        // Build prompt based on event and context
        let prompt = self.build_event_prompt(event, context)?;
        
        // Create agent request
        let request = AgentRequest {
            prompt,
            context: Some(serde_json::to_string(context)?),
        };

        // Process through AI runtime (this is a simplified version)
        // In a real implementation, this would route to the appropriate AI provider
        let response_content = self.generate_response(&request, event).await?;

        Ok(AgentResponse {
            content: response_content,
            metadata: HashMap::from([
                ("agent_id".to_string(), self.id.clone()),
                ("event_type".to_string(), format!("{:?}", event)),
                ("timestamp".to_string(), chrono::Utc::now().to_rfc3339()),
            ]),
        })
    }

    /// Build a contextual prompt for the given event
    fn build_event_prompt(&self, event: &TerminalEventType, context: &PtyAiContext) -> Result<String> {
        let mut prompt = format!("{}\n\n", self.system_prompt);
        
        // Add context information
        prompt.push_str(&format!("Current working directory: {}\n", 
            context.terminal_context.working_directory.display()));
        
        if let Some(ref branch) = context.terminal_context.git_branch {
            prompt.push_str(&format!("Git branch: {}\n", branch));
        }
        
        // Add recent command history (limited for context)
        if !context.terminal_context.recent_commands.is_empty() {
            prompt.push_str("Recent commands:\n");
            for cmd in context.terminal_context.recent_commands.iter().take(3) {
                prompt.push_str(&format!("  {}\n", cmd));
            }
        }
        
        prompt.push_str("\n");

        // Add event-specific context
        match event {
            TerminalEventType::CommandExecuted { command, exit_code, output, error_output, .. } => {
                prompt.push_str(&format!("A command was executed:\n"));
                prompt.push_str(&format!("Command: {}\n", command));
                prompt.push_str(&format!("Exit code: {}\n", exit_code));
                if !output.is_empty() {
                    prompt.push_str(&format!("Output:\n{}\n", output));
                }
                if !error_output.is_empty() {
                    prompt.push_str(&format!("Error output:\n{}\n", error_output));
                }
                
                if *exit_code != 0 {
                    prompt.push_str("The command failed. Please analyze the error and suggest a fix or explanation.\n");
                } else {
                    prompt.push_str("The command executed successfully. Provide helpful insights if relevant.\n");
                }
            }
            TerminalEventType::CommandFailed { command, error, exit_code, .. } => {
                prompt.push_str(&format!("A command failed:\n"));
                prompt.push_str(&format!("Command: {}\n", command));
                prompt.push_str(&format!("Error: {}\n", error));
                prompt.push_str(&format!("Exit code: {}\n", exit_code));
                prompt.push_str("Please explain what went wrong and suggest how to fix it.\n");
            }
            TerminalEventType::CommandTyped { partial_command, .. } => {
                prompt.push_str(&format!("User is typing a command:\n"));
                prompt.push_str(&format!("Partial command: {}\n", partial_command));
                prompt.push_str("Suggest completions or provide helpful hints about this command.\n");
            }
            TerminalEventType::DirectoryChanged { old_path, new_path } => {
                prompt.push_str(&format!("Directory changed:\n"));
                prompt.push_str(&format!("From: {}\n", old_path.display()));
                prompt.push_str(&format!("To: {}\n", new_path.display()));
                prompt.push_str("Provide relevant information about the new directory or suggest useful commands.\n");
            }
            TerminalEventType::FileChanged { path, change_type } => {
                prompt.push_str(&format!("File changed:\n"));
                prompt.push_str(&format!("Path: {}\n", path.display()));
                prompt.push_str(&format!("Change type: {:?}\n", change_type));
                prompt.push_str("Provide relevant insights about this file change.\n");
            }
            TerminalEventType::GitOperation { operation, branch, .. } => {
                prompt.push_str(&format!("Git operation detected:\n"));
                prompt.push_str(&format!("Operation: {:?}\n", operation));
                if let Some(ref b) = branch {
                    prompt.push_str(&format!("Branch: {}\n", b));
                }
                prompt.push_str("Provide helpful git-related suggestions or insights.\n");
            }
            TerminalEventType::AiAssistanceRequested { context: req_context, assistance_type } => {
                prompt.push_str(&format!("User requested AI assistance:\n"));
                prompt.push_str(&format!("Type: {:?}\n", assistance_type));
                prompt.push_str(&format!("Context: {}\n", req_context));
                prompt.push_str("Provide the requested assistance.\n");
            }
            TerminalEventType::SessionStarted { shell, .. } => {
                prompt.push_str(&format!("New terminal session started:\n"));
                prompt.push_str(&format!("Shell: {:?}\n", shell));
                prompt.push_str("Welcome the user and provide helpful startup information.\n");
            }
        }

        Ok(prompt)
    }

    /// Generate AI response (simplified implementation)
    async fn generate_response(&self, request: &AgentRequest, event: &TerminalEventType) -> Result<String> {
        // In a real implementation, this would call the actual AI provider
        // For now, provide contextual responses based on event type
        
        tokio::time::sleep(Duration::from_millis(100)).await; // Simulate AI processing
        
        let response = match event {
            TerminalEventType::CommandFailed { command, error, exit_code, .. } => {
                if command.contains("git") && error.contains("not a git repository") {
                    format!("It looks like you're trying to run a Git command, but this directory isn't a Git repository. Try running `git init` to initialize a new repository, or `cd` into an existing Git repository.")
                } else if command.contains("npm") && error.contains("not found") {
                    format!("NPM command failed because Node.js/npm is not installed or not in your PATH. Try installing Node.js from https://nodejs.org/ or using your system package manager.")
                } else if command.contains("python") && error.contains("No module named") {
                    format!("Python module not found. You may need to install it with `pip install <module-name>` or activate the correct virtual environment.")
                } else {
                    format!("Command '{}' failed with exit code {}. The error suggests: {}. Try checking the command syntax or required dependencies.", command, exit_code, error)
                }
            }
            TerminalEventType::CommandTyped { partial_command, .. } => {
                if partial_command.starts_with("git ") {
                    format!("Git commands: `git status`, `git add .`, `git commit -m \"message\"`, `git push`, `git pull`")
                } else if partial_command.starts_with("docker ") {
                    format!("Docker commands: `docker ps`, `docker build -t name .`, `docker run`, `docker-compose up`")
                } else if partial_command.contains("cd ") {
                    format!("Use `cd <directory>` to change directories, `cd ..` to go up one level, `cd ~` for home directory")
                } else {
                    format!("Type `man <command>` for help with specific commands, or `--help` flag for most commands")
                }
            }
            TerminalEventType::DirectoryChanged { new_path, .. } => {
                let path_str = new_path.to_string_lossy();
                if path_str.contains("git") || new_path.join(".git").exists() {
                    format!("You're now in a Git repository. Try `git status` to see the current state, or `git log --oneline` to see recent commits.")
                } else if new_path.join("package.json").exists() {
                    format!("This looks like a Node.js project. Try `npm install` to install dependencies, or `npm run` to see available scripts.")
                } else if new_path.join("Cargo.toml").exists() {
                    format!("This is a Rust project. Try `cargo build` to compile, `cargo run` to run, or `cargo test` to run tests.")
                } else {
                    format!("Now in directory: {}. Use `ls -la` to see contents, `pwd` to confirm location.", new_path.display())
                }
            }
            _ => {
                format!("I'm monitoring your terminal activity and ready to help with any questions or issues you encounter.")
            }
        };

        Ok(response)
    }
}

/// Central AI event integration system
pub struct AiEventIntegrator {
    /// Registered AI agents
    agents: Arc<RwLock<HashMap<String, AiAgent>>>,
    
    /// Event channel for receiving terminal events
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<TerminalEventType>>>,
    event_sender: mpsc::UnboundedSender<TerminalEventType>,
    
    /// Response channel for sending AI responses back to terminal
    response_sender: broadcast::Sender<AgentResponse>,
    
    /// AI runtime instance
    ai_runtime: Arc<RwLock<AiRuntime>>,
    
    /// Context provider for terminal state
    context_provider: Arc<crate::ai_context_provider::ContextProvider>,
    
    /// Event processing statistics
    stats: Arc<RwLock<EventStats>>,
    
    /// Processing task handle
    task_handle: Option<tokio::task::JoinHandle<()>>,
}


impl AiEventIntegrator {
    /// Create a new AI event integrator
    pub fn new(ai_runtime: Arc<RwLock<AiRuntime>>) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let (response_sender, _) = broadcast::channel(1000);
        
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            event_sender,
            response_sender,
            ai_runtime,
            context_provider: Arc::new(crate::ai_context_provider::ContextProvider::new()),
            stats: Arc::new(RwLock::new(EventStats::default())),
            task_handle: None,
        }
    }
    
    /// Register a new AI agent
    pub async fn register_agent(&self, agent: AiAgent) -> Result<()> {
        let mut agents = self.agents.write().await;
        let id = agent.id.clone();
        agents.insert(id.clone(), agent);
        info!("Registered AI agent: {}", id);
        Ok(())
    }
    
    /// Unregister an AI agent
    pub async fn unregister_agent(&self, agent_id: &str) -> Result<()> {
        let mut agents = self.agents.write().await;
        if agents.remove(agent_id).is_some() {
            info!("Unregistered AI agent: {}", agent_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Agent not found: {}", agent_id))
        }
    }
    
    /// Get event sender for external systems to send events
    pub fn get_event_sender(&self) -> mpsc::UnboundedSender<TerminalEventType> {
        self.event_sender.clone()
    }
    
    /// Get response receiver for consuming AI responses
    pub fn get_response_receiver(&self) -> broadcast::Receiver<AgentResponse> {
        self.response_sender.subscribe()
    }
    
    /// Start the event processing loop
    pub async fn start_processing(&mut self) -> Result<()> {
        if self.task_handle.is_some() {
            return Err(anyhow::anyhow!("Event processing already started"));
        }
        
        let agents = Arc::clone(&self.agents);
        let event_receiver = Arc::clone(&self.event_receiver);
        let response_sender = self.response_sender.clone();
        let ai_runtime = Arc::clone(&self.ai_runtime);
        let context_provider = Arc::clone(&self.context_provider);
        let stats = Arc::clone(&self.stats);
        
        let handle = tokio::spawn(async move {
            info!("AI event processing started");
            
            loop {
                let event = {
                    let mut receiver = event_receiver.lock().await;
                    match receiver.recv().await {
                        Some(event) => event,
                        None => {
                            warn!("Event channel closed, stopping processing");
                            break;
                        }
                    }
                };
                
                let start_time = Instant::now();
                
                // Update statistics
                {
                    let mut stats_lock = stats.write().await;
                    stats_lock.events_processed += 1;
                    stats_lock.last_event_time = Some(start_time);
                }
                
                // Extract context
                let context = match Self::extract_context(&context_provider, &event).await {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        error!("Failed to extract context for event: {}", e);
                        continue;
                    }
                };
                
                // Find agents that should respond to this event
                let mut activated_agents = Vec::new();
                {
                    let agents_lock = agents.read().await;
                    for (id, agent) in agents_lock.iter() {
                        if agent.should_activate(&event, &context) {
                            activated_agents.push((id.clone(), agent.clone()));
                        }
                    }
                }
                
                // Sort by priority (higher priority first)
                activated_agents.sort_by(|a, b| b.1.trigger.priority.cmp(&a.1.trigger.priority));
                
                // Process responses from activated agents
                for (agent_id, mut agent) in activated_agents {
                    {
                        let mut runtime = ai_runtime.write().await;
                        match agent.process_event(&event, &context, &mut runtime).await {
                            Ok(response) => {
                                // Send response
                                if let Err(e) = response_sender.send(response) {
                                    warn!("Failed to send agent response: {}", e);
                                }
                                
                                // Update statistics
                                {
                                    let mut stats_lock = stats.write().await;
                                    stats_lock.agents_activated += 1;
                                    stats_lock.responses_generated += 1;
                                }
                                
                                // Update agent in collection
                                {
                                    let mut agents_lock = agents.write().await;
                                    agents_lock.insert(agent_id, agent);
                                }
                            }
                            Err(e) => {
                                error!("Agent {} failed to process event: {}", agent_id, e);
                                let mut stats_lock = stats.write().await;
                                stats_lock.errors_encountered += 1;
                            }
                        }
                    }
                }
                
                // Update processing time statistics
                let processing_time = start_time.elapsed().as_millis() as f64;
                {
                    let mut stats_lock = stats.write().await;
                    let total_events = stats_lock.events_processed as f64;
                    stats_lock.average_processing_time_ms = 
                        (stats_lock.average_processing_time_ms * (total_events - 1.0) + processing_time) / total_events;
                }
                
                debug!("Processed event in {:.2}ms", processing_time);
            }
        });
        
        self.task_handle = Some(handle);
        Ok(())
    }
    
    /// Stop event processing
    pub async fn stop_processing(&mut self) {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
            info!("AI event processing stopped");
        }
    }
    
    /// Extract terminal context for event processing
    async fn extract_context(context_provider: &crate::ai_context_provider::ContextProvider, event: &TerminalEventType) -> Result<PtyAiContext> {
        let terminal_context = context_provider.extract_context()?;
        
        // Add event-specific context
        let (current_input, last_output, error_context) = match event {
            TerminalEventType::CommandTyped { partial_command, .. } => {
                (Some(partial_command.clone()), None, None)
            }
            TerminalEventType::CommandExecuted { command, output, error_output, .. } => {
                (
                    Some(command.clone()),
                    if !output.is_empty() { Some(output.clone()) } else { None },
                    if !error_output.is_empty() { Some(error_output.clone()) } else { None },
                )
            }
            TerminalEventType::CommandFailed { command, error, .. } => {
                (Some(command.clone()), None, Some(error.clone()))
            }
            _ => (None, None, None),
        };
        
        Ok(PtyAiContext {
            terminal_context,
            current_input,
            last_output,
            error_context,
        })
    }
    
    /// Get current processing statistics
    pub async fn get_stats(&self) -> EventStats {
        self.stats.read().await.clone()
    }
    
    /// Send a terminal event for processing
    pub fn send_event(&self, event: TerminalEventType) -> Result<()> {
        self.event_sender.send(event)
            .map_err(|e| anyhow::anyhow!("Failed to send event: {}", e))?;
        Ok(())
    }
}

/// Pre-configured AI agents for common terminal scenarios
pub struct DefaultAgents;

impl DefaultAgents {
    /// Create a command error analysis agent
    pub fn error_analyzer() -> AiAgent {
        let mut agent = AiAgent::new(
            "error_analyzer".to_string(),
            "Error Analyzer".to_string(),
            AiProvider::Ollama,
        );
        
        agent.description = "Analyzes command errors and suggests fixes".to_string();
        agent.system_prompt = "You are an expert system administrator and developer. When commands fail, analyze the error and provide clear, actionable solutions. Focus on common issues and practical fixes.".to_string();
        
        agent.trigger.event_types = vec![
            TerminalEventType::CommandFailed { command: String::new(), error: String::new(), exit_code: 0, working_directory: PathBuf::new() }
        ];
        
        agent.trigger.activation_conditions = vec![
            ActivationCondition::ExitCodeEquals(1),  // Most command failures
        ];
        
        agent.trigger.priority = 90;
        agent.trigger.debounce_duration = Duration::from_secs(2);
        
        agent
    }
    
    /// Create a command completion agent
    pub fn command_completer() -> AiAgent {
        let mut agent = AiAgent::new(
            "command_completer".to_string(),
            "Command Completer".to_string(),
            AiProvider::Ollama,
        );
        
        agent.description = "Provides command completions and suggestions".to_string();
        agent.system_prompt = "You are a command-line expert. Provide helpful command completions, usage examples, and suggestions based on what the user is typing. Keep responses concise and practical.".to_string();
        
        agent.trigger.event_types = vec![
            TerminalEventType::CommandTyped { partial_command: String::new(), cursor_position: 0, working_directory: PathBuf::new() }
        ];
        
        agent.trigger.priority = 60;
        agent.trigger.debounce_duration = Duration::from_millis(800);
        
        agent
    }
    
    /// Create a directory context agent
    pub fn directory_advisor() -> AiAgent {
        let mut agent = AiAgent::new(
            "directory_advisor".to_string(),
            "Directory Advisor".to_string(),
            AiProvider::Ollama,
        );
        
        agent.description = "Provides context-aware advice when changing directories".to_string();
        agent.system_prompt = "You are a project navigation expert. When users change directories, provide helpful context about the directory they entered, suggest relevant commands, and highlight important files or patterns you notice.".to_string();
        
        agent.trigger.event_types = vec![
            TerminalEventType::DirectoryChanged { old_path: PathBuf::new(), new_path: PathBuf::new() }
        ];
        
        agent.trigger.priority = 40;
        agent.trigger.debounce_duration = Duration::from_secs(1);
        
        agent
    }
    
    /// Create a git operations agent
    pub fn git_assistant() -> AiAgent {
        let mut agent = AiAgent::new(
            "git_assistant".to_string(),
            "Git Assistant".to_string(),
            AiProvider::Ollama,
        );
        
        agent.description = "Provides Git-related assistance and best practices".to_string();
        agent.system_prompt = "You are a Git expert. Help users with version control workflows, explain Git concepts, suggest best practices, and help resolve Git-related issues. Provide clear, step-by-step guidance.".to_string();
        
        agent.trigger.event_types = vec![
            TerminalEventType::GitOperation { operation: GitOperationType::Commit, branch: None, working_directory: PathBuf::new() },
            TerminalEventType::CommandFailed { command: String::new(), error: String::new(), exit_code: 0, working_directory: PathBuf::new() }
        ];
        
        agent.trigger.activation_conditions = vec![
            ActivationCondition::CommandContains("git".to_string()),
        ];
        
        agent.trigger.priority = 70;
        agent.trigger.debounce_duration = Duration::from_secs(3);
        
        agent
    }
    
    /// Get all default agents
    pub fn all() -> Vec<AiAgent> {
        vec![
            Self::error_analyzer(),
            Self::command_completer(),
            Self::directory_advisor(),
            Self::git_assistant(),
        ]
    }
}

#[derive(Debug, Clone)]
pub struct EventStats {
    pub events_processed: u64,
    pub agents_activated: u64,
    pub responses_generated: u64,
    pub errors_encountered: u64,
    pub average_processing_time_ms: f64,
    pub last_event_time: Option<Instant>,
}

impl Default for EventStats {
    fn default() -> Self {
        Self {
            events_processed: 0,
            agents_activated: 0,
            responses_generated: 0,
            errors_encountered: 0,
            average_processing_time_ms: 0.0,
            last_event_time: None,
        }
    }
}