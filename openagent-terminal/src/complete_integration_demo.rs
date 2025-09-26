//! Complete AI Terminal Integration Demo
//!
//! This module demonstrates the full integration of all three major AI systems:
//! 1. AI Event Integration - Real-time AI assistance based on terminal events
//! 2. Command Assistance - Auto-completion, error explanation, and command suggestions
//! 3. Conversation Management - Rich conversational AI with context preservation
//!
//! The demo shows how these systems work together to create a comprehensive
//! AI-powered terminal experience that provides contextual assistance, learns
//! from user interactions, and maintains conversational state across sessions.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use tokio::sync::{mpsc, RwLock, Mutex};
use tracing::{info, warn, debug, error};

// Import all three major systems
use crate::ai_event_integration::{AiEventIntegrator, AiEventConfig, TerminalEventType};
use crate::command_assistance::{CommandAssistanceEngine, AssistanceConfig, AssistanceType};
use crate::conversation_management::{
    ConversationManager, ConversationConfig, MessageType, ConversationId,
};

// Import supporting modules
use crate::ai_runtime::{AiRuntime, AiProvider, AgentResponse};
use crate::ai_context_provider::{PtyAiContext, TerminalContext, ProjectInfo, ProjectType};
use crate::terminal_event_bridge::{TerminalEventBridge, TerminalEvent};
use crate::blocks_v2::ShellType;

/// Complete AI terminal integration system
pub struct CompleteAiIntegration {
    /// AI event integration for real-time assistance
    ai_event_integrator: AiEventIntegrator,
    
    /// Command assistance engine
    command_assistance: Arc<RwLock<CommandAssistanceEngine>>,
    
    /// Conversation management system
    conversation_manager: ConversationManager,
    
    /// Shared AI runtime
    ai_runtime: Arc<RwLock<AiRuntime>>,
    
    /// Terminal event bridge
    event_bridge: TerminalEventBridge,
    
    /// Current active conversation
    current_conversation: Option<ConversationId>,
    
    /// Integration statistics
    stats: Arc<RwLock<IntegrationStats>>,
    
    /// Event coordination
    event_coordinator: EventCoordinator,
}

/// Statistics for the complete integration
#[derive(Debug, Clone, Default)]
pub struct IntegrationStats {
    pub events_processed: u64,
    pub commands_assisted: u64,
    pub conversations_created: u64,
    pub suggestions_generated: u64,
    pub errors_analyzed: u64,
    pub context_switches: u64,
    pub session_duration: Duration,
    pub most_used_commands: HashMap<String, u32>,
    pub error_patterns: HashMap<String, u32>,
    pub assistance_effectiveness: f64,
}

/// Coordinates events between different AI systems
pub struct EventCoordinator {
    event_sender: mpsc::UnboundedSender<IntegrationEvent>,
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<IntegrationEvent>>>,
}

/// Events that coordinate between different AI systems
#[derive(Debug, Clone)]
pub enum IntegrationEvent {
    /// Terminal event occurred
    TerminalEvent {
        event: TerminalEvent,
        context: PtyAiContext,
    },
    
    /// Command assistance requested
    AssistanceRequested {
        assistance_type: AssistanceType,
        command: String,
        context: PtyAiContext,
    },
    
    /// Error occurred and needs analysis
    ErrorOccurred {
        error_message: String,
        command: String,
        context: PtyAiContext,
    },
    
    /// Conversation message added
    ConversationMessage {
        conversation_id: ConversationId,
        message_type: MessageType,
        content: String,
        context: PtyAiContext,
    },
    
    /// Context change detected
    ContextChanged {
        old_context: PtyAiContext,
        new_context: PtyAiContext,
    },
    
    /// Suggestion generated
    SuggestionGenerated {
        suggestion: String,
        confidence: f64,
        source: String,
    },
}

impl CompleteAiIntegration {
    /// Create a new complete AI integration system
    pub async fn new() -> Result<Self> {
        // Initialize shared AI runtime
        let ai_runtime = Arc::new(RwLock::new(
            AiRuntime::new(vec![AiProvider::Ollama]).await?
        ));
        
        // Initialize command assistance
        let assistance_config = AssistanceConfig::default();
        let command_assistance = Arc::new(RwLock::new(
            CommandAssistanceEngine::new(assistance_config).await?
        ));
        
        // Initialize conversation manager
        let conversation_config = ConversationConfig::default();
        let conversation_manager = ConversationManager::new(
            conversation_config,
            Arc::clone(&ai_runtime),
            Arc::clone(&command_assistance),
        ).await?;
        
        // Initialize AI event integrator
        let event_config = AiEventConfig::default();
        let ai_event_integrator = AiEventIntegrator::new(
            event_config,
            Arc::clone(&ai_runtime),
        ).await?;
        
        // Initialize terminal event bridge
        let event_bridge = TerminalEventBridge::new().await?;
        
        // Initialize event coordinator
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let event_coordinator = EventCoordinator {
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
        };
        
        Ok(Self {
            ai_event_integrator,
            command_assistance,
            conversation_manager,
            ai_runtime,
            event_bridge,
            current_conversation: None,
            stats: Arc::new(RwLock::new(IntegrationStats::default())),
            event_coordinator,
        })
    }
    
    /// Start the complete AI integration system
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting Complete AI Terminal Integration");
        
        // Start all subsystems
        self.ai_event_integrator.start().await?;
        self.command_assistance.write().await.start().await?;
        self.conversation_manager.start().await?;
        self.event_bridge.start().await?;
        
        // Start event coordination
        self.start_event_coordination().await;
        
        // Start statistics collection
        self.start_statistics_collection().await;
        
        info!("✅ Complete AI Terminal Integration started successfully");
        Ok(())
    }
    
    /// Process a terminal command with full AI integration
    pub async fn process_command(
        &mut self,
        command: String,
        context: &PtyAiContext,
    ) -> Result<IntegratedCommandResult> {
        let start_time = std::time::Instant::now();
        
        info!("🔄 Processing command with full AI integration: {}", command);
        
        let mut result = IntegratedCommandResult {
            original_command: command.clone(),
            suggestions: Vec::new(),
            conversation_response: None,
            error_analysis: None,
            context_updates: Vec::new(),
            assistance_provided: false,
            processing_time: Duration::ZERO,
        };
        
        // 1. Get command assistance (auto-completion, validation)
        let assistance_result = self.command_assistance
            .read()
            .await
            .analyze_command(&command, context)
            .await?;
        
        if !assistance_result.suggestions.is_empty() {
            result.suggestions.extend(assistance_result.suggestions);
            result.assistance_provided = true;
        }
        
        // 2. Add to conversation if there's an active one
        if let Some(conversation_id) = self.current_conversation {
            let message_id = self.conversation_manager
                .add_message(
                    conversation_id,
                    MessageType::User,
                    format!("$ {}", command),
                    Some(context.clone()),
                )
                .await?;
            
            // Generate AI response for the command
            let ai_response = self.conversation_manager
                .process_user_input(
                    format!("Help me with this command: {}", command),
                    context,
                )
                .await?;
            
            result.conversation_response = Some(ai_response);
        }
        
        // 3. Trigger AI event integration for command execution
        let terminal_event = TerminalEvent::CommandExecuted {
            command: command.clone(),
            exit_code: 0, // We'll update this after actual execution
            output: String::new(),
            working_directory: context.terminal_context.working_directory.clone(),
            duration: Duration::from_millis(100), // Placeholder
        };
        
        let _ = self.event_coordinator.event_sender.send(IntegrationEvent::TerminalEvent {
            event: terminal_event,
            context: context.clone(),
        });
        
        // 4. Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.commands_assisted += 1;
            if result.assistance_provided {
                stats.suggestions_generated += result.suggestions.len() as u64;
            }
        }
        
        result.processing_time = start_time.elapsed();
        
        info!("✅ Command processing completed in {:?}", result.processing_time);
        Ok(result)
    }
    
    /// Handle command execution result with full AI integration
    pub async fn handle_command_result(
        &mut self,
        command: String,
        exit_code: i32,
        output: String,
        context: &PtyAiContext,
    ) -> Result<IntegratedResultAnalysis> {
        info!("📊 Analyzing command result: {} (exit code: {})", command, exit_code);
        
        let mut analysis = IntegratedResultAnalysis {
            success: exit_code == 0,
            error_analysis: None,
            suggestions: Vec::new(),
            conversation_insights: Vec::new(),
            learning_updates: Vec::new(),
        };
        
        // 1. Handle errors with command assistance
        if exit_code != 0 {
            let error_analysis = self.command_assistance
                .read()
                .await
                .analyze_error(&command, &output, context)
                .await?;
            
            analysis.error_analysis = Some(error_analysis);
            
            // Send error event to coordinator
            let _ = self.event_coordinator.event_sender.send(IntegrationEvent::ErrorOccurred {
                error_message: output.clone(),
                command: command.clone(),
                context: context.clone(),
            });
            
            // Update statistics
            let mut stats = self.stats.write().await;
            stats.errors_analyzed += 1;
            *stats.error_patterns.entry(command.split_whitespace().next().unwrap_or("unknown").to_string())
                .or_insert(0) += 1;
        }
        
        // 2. Add command result to conversation
        if let Some(conversation_id) = self.current_conversation {
            let message_type = if exit_code == 0 { MessageType::CommandResult } else { MessageType::Error };
            
            let _message_id = self.conversation_manager
                .add_message(
                    conversation_id,
                    message_type,
                    output.clone(),
                    Some(context.clone()),
                )
                .await?;
            
            // Generate insights based on the result
            if exit_code != 0 {
                let insight = self.conversation_manager
                    .process_user_input(
                        format!("This command failed: {}. Output: {}", command, output),
                        context,
                    )
                    .await?;
                
                analysis.conversation_insights.push(insight.content);
            }
        }
        
        // 3. Trigger AI event integration
        let terminal_event = TerminalEvent::CommandCompleted {
            command: command.clone(),
            exit_code,
            output: output.clone(),
            working_directory: context.terminal_context.working_directory.clone(),
            duration: Duration::from_millis(500), // Placeholder
        };
        
        let _ = self.event_coordinator.event_sender.send(IntegrationEvent::TerminalEvent {
            event: terminal_event,
            context: context.clone(),
        });
        
        // 4. Generate suggestions based on context
        let context_suggestions = self.generate_context_suggestions(context).await?;
        analysis.suggestions.extend(context_suggestions);
        
        Ok(analysis)
    }
    
    /// Start a new AI conversation
    pub async fn start_conversation(
        &mut self,
        title: Option<String>,
        context: &PtyAiContext,
    ) -> Result<ConversationId> {
        let conversation_id = self.conversation_manager
            .create_conversation(title, context)
            .await?;
        
        self.current_conversation = Some(conversation_id);
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.conversations_created += 1;
        }
        
        info!("💬 Started new conversation: {}", conversation_id);
        Ok(conversation_id)
    }
    
    /// Get AI assistance for any query
    pub async fn get_ai_assistance(
        &mut self,
        query: String,
        context: &PtyAiContext,
    ) -> Result<AgentResponse> {
        // Ensure there's an active conversation
        if self.current_conversation.is_none() {
            self.start_conversation(Some("AI Assistance Session".to_string()), context).await?;
        }
        
        // Process the query through the conversation manager
        let response = self.conversation_manager
            .process_user_input(query, context)
            .await?;
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.suggestions_generated += 1;
        }
        
        Ok(response)
    }
    
    /// Handle directory changes with context awareness
    pub async fn handle_directory_change(
        &mut self,
        old_path: PathBuf,
        new_path: PathBuf,
        context: &PtyAiContext,
    ) -> Result<Vec<String>> {
        info!("📁 Directory changed: {} -> {}", old_path.display(), new_path.display());
        
        let mut suggestions = Vec::new();
        
        // 1. Trigger AI event integration
        let terminal_event = TerminalEvent::DirectoryChanged {
            old_directory: old_path.clone(),
            new_directory: new_path.clone(),
        };
        
        let _ = self.event_coordinator.event_sender.send(IntegrationEvent::TerminalEvent {
            event: terminal_event,
            context: context.clone(),
        });
        
        // 2. Get contextual suggestions from AI event integrator
        let event_suggestions = self.ai_event_integrator
            .handle_directory_change(&old_path, &new_path, context)
            .await?;
        
        suggestions.extend(event_suggestions);
        
        // 3. Add context change to conversation
        if let Some(conversation_id) = self.current_conversation {
            let _message_id = self.conversation_manager
                .add_message(
                    conversation_id,
                    MessageType::System,
                    format!("Changed directory to {}", new_path.display()),
                    Some(context.clone()),
                )
                .await?;
        }
        
        // 4. Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.context_switches += 1;
        }
        
        Ok(suggestions)
    }
    
    /// Get comprehensive system statistics
    pub async fn get_integration_stats(&self) -> IntegrationStats {
        let stats = self.stats.read().await;
        stats.clone()
    }
    
    /// Shutdown the integration system gracefully
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("🛑 Shutting down Complete AI Terminal Integration");
        
        // Stop all subsystems
        self.ai_event_integrator.stop().await?;
        self.command_assistance.write().await.stop().await?;
        self.conversation_manager.stop().await;
        self.event_bridge.stop().await?;
        
        info!("✅ Complete AI Terminal Integration shutdown completed");
        Ok(())
    }
    
    // Private implementation methods
    
    /// Start event coordination between systems
    async fn start_event_coordination(&mut self) {
        let event_receiver = Arc::clone(&self.event_coordinator.event_receiver);
        let stats = Arc::clone(&self.stats);
        
        tokio::spawn(async move {
            let mut receiver = event_receiver.lock().await;
            
            while let Some(event) = receiver.recv().await {
                Self::handle_coordination_event(event, Arc::clone(&stats)).await;
            }
        });
    }
    
    /// Handle coordination events
    async fn handle_coordination_event(event: IntegrationEvent, stats: Arc<RwLock<IntegrationStats>>) {
        match event {
            IntegrationEvent::TerminalEvent { event, context: _ } => {
                let mut stats_lock = stats.write().await;
                stats_lock.events_processed += 1;
                
                match event {
                    TerminalEvent::CommandExecuted { command, .. } |
                    TerminalEvent::CommandCompleted { command, .. } => {
                        *stats_lock.most_used_commands
                            .entry(command.split_whitespace().next().unwrap_or("unknown").to_string())
                            .or_insert(0) += 1;
                    },
                    _ => {}
                }
            },
            
            IntegrationEvent::ErrorOccurred { error_message, .. } => {
                debug!("🚨 Error coordination: {}", error_message);
            },
            
            IntegrationEvent::SuggestionGenerated { suggestion, confidence, source } => {
                debug!("💡 Suggestion from {}: {} (confidence: {:.2})", source, suggestion, confidence);
                let mut stats_lock = stats.write().await;
                stats_lock.suggestions_generated += 1;
            },
            
            _ => {
                debug!("📡 Event coordination: {:?}", event);
            }
        }
    }
    
    /// Start statistics collection
    async fn start_statistics_collection(&mut self) {
        let stats = Arc::clone(&self.stats);
        let start_time = std::time::Instant::now();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                let mut stats_lock = stats.write().await;
                stats_lock.session_duration = start_time.elapsed();
                
                // Calculate assistance effectiveness
                if stats_lock.commands_assisted > 0 {
                    stats_lock.assistance_effectiveness = 
                        stats_lock.suggestions_generated as f64 / stats_lock.commands_assisted as f64;
                }
                
                debug!("📊 Updated integration statistics");
            }
        });
    }
    
    /// Generate context-aware suggestions
    async fn generate_context_suggestions(&self, context: &PtyAiContext) -> Result<Vec<String>> {
        let mut suggestions = Vec::new();
        
        // Generate suggestions based on current context
        if let Some(ref project_info) = context.terminal_context.project_info {
            match project_info.project_type {
                ProjectType::Rust => {
                    suggestions.extend(vec![
                        "Run 'cargo check' to verify compilation".to_string(),
                        "Use 'cargo test' to run tests".to_string(),
                        "Try 'cargo clippy' for additional lints".to_string(),
                    ]);
                },
                ProjectType::NodeJs => {
                    suggestions.extend(vec![
                        "Run 'npm test' to execute tests".to_string(),
                        "Use 'npm audit' to check for vulnerabilities".to_string(),
                        "Try 'npm run build' if available".to_string(),
                    ]);
                },
                ProjectType::Python => {
                    suggestions.extend(vec![
                        "Run 'python -m pytest' for testing".to_string(),
                        "Use 'pip install -r requirements.txt' for dependencies".to_string(),
                        "Try 'python -m black .' for code formatting".to_string(),
                    ]);
                },
                _ => {
                    suggestions.push("Explore the project structure with 'find . -type f -name \"*\"'".to_string());
                }
            }
        }
        
        // Add Git-specific suggestions if in a Git repository
        if context.terminal_context.git_branch.is_some() {
            suggestions.extend(vec![
                "Check status with 'git status'".to_string(),
                "View recent commits with 'git log --oneline -10'".to_string(),
            ]);
        }
        
        Ok(suggestions)
    }
}

/// Result of integrated command processing
#[derive(Debug, Clone)]
pub struct IntegratedCommandResult {
    pub original_command: String,
    pub suggestions: Vec<String>,
    pub conversation_response: Option<AgentResponse>,
    pub error_analysis: Option<crate::command_assistance::ErrorAnalysis>,
    pub context_updates: Vec<String>,
    pub assistance_provided: bool,
    pub processing_time: Duration,
}

/// Analysis of command execution results
#[derive(Debug, Clone)]
pub struct IntegratedResultAnalysis {
    pub success: bool,
    pub error_analysis: Option<crate::command_assistance::ErrorAnalysis>,
    pub suggestions: Vec<String>,
    pub conversation_insights: Vec<String>,
    pub learning_updates: Vec<String>,
}

/// Demonstration of the complete AI integration
pub struct CompleteIntegrationDemo {
    integration: CompleteAiIntegration,
    demo_contexts: Vec<PtyAiContext>,
}

impl CompleteIntegrationDemo {
    /// Create a new complete integration demo
    pub async fn new() -> Result<Self> {
        let integration = CompleteAiIntegration::new().await?;
        let demo_contexts = Self::create_demo_contexts();
        
        Ok(Self {
            integration,
            demo_contexts,
        })
    }
    
    /// Run the complete integration demonstration
    pub async fn run_demo(&mut self) -> Result<()> {
        info!("🎯 Starting Complete AI Terminal Integration Demo");
        
        // Start the integration system
        self.integration.start().await?;
        
        // Demo various scenarios
        self.demo_basic_command_assistance().await?;
        self.demo_error_handling_integration().await?;
        self.demo_conversation_workflow().await?;
        self.demo_context_awareness().await?;
        self.demo_directory_navigation().await?;
        self.demo_project_specific_assistance().await?;
        self.demo_advanced_integration().await?;
        
        // Show final statistics
        self.show_integration_statistics().await?;
        
        // Shutdown gracefully
        self.integration.shutdown().await?;
        
        info!("✅ Complete AI Terminal Integration Demo completed successfully");
        Ok(())
    }
    
    /// Demo basic command assistance
    async fn demo_basic_command_assistance(&mut self) -> Result<()> {
        info!("📝 Demo: Basic Command Assistance");
        
        let context = &self.demo_contexts[0];
        
        // Start a conversation
        self.integration.start_conversation(Some("Basic Commands Demo".to_string()), context).await?;
        
        // Test various commands with assistance
        let commands = vec!["ls -la", "git status", "cargo check", "npm test"];
        
        for command in commands {
            let result = self.integration.process_command(command.to_string(), context).await?;
            
            info!("Command: {}", result.original_command);
            info!("  Suggestions: {}", result.suggestions.len());
            info!("  Assistance provided: {}", result.assistance_provided);
            info!("  Processing time: {:?}", result.processing_time);
            
            if let Some(response) = result.conversation_response {
                info!("  AI Response: {}", response.content.chars().take(100).collect::<String>());
            }
        }
        
        Ok(())
    }
    
    /// Demo error handling integration
    async fn demo_error_handling_integration(&mut self) -> Result<()> {
        info!("🚨 Demo: Error Handling Integration");
        
        let context = &self.demo_contexts[1]; // Context with errors
        
        // Simulate command errors
        let error_scenarios = vec![
            ("npm start", 1, "Error: Cannot find module 'express'"),
            ("cargo build", 1, "error[E0382]: borrow of moved value: `data`"),
            ("python main.py", 1, "ModuleNotFoundError: No module named 'requests'"),
            ("git push", 1, "fatal: remote origin already exists"),
        ];
        
        for (command, exit_code, error_output) in error_scenarios {
            let _cmd_result = self.integration.process_command(command.to_string(), context).await?;
            
            let analysis = self.integration
                .handle_command_result(
                    command.to_string(),
                    exit_code,
                    error_output.to_string(),
                    context,
                )
                .await?;
            
            info!("Error Analysis for '{}': Success: {}", command, analysis.success);
            info!("  Suggestions: {}", analysis.suggestions.len());
            info!("  Conversation insights: {}", analysis.conversation_insights.len());
            
            if let Some(error_analysis) = analysis.error_analysis {
                info!("  Error type: {:?}", error_analysis.error_type);
                info!("  Fixes: {}", error_analysis.suggested_fixes.len());
            }
        }
        
        Ok(())
    }
    
    /// Demo conversation workflow
    async fn demo_conversation_workflow(&mut self) -> Result<()> {
        info!("💬 Demo: Conversation Workflow");
        
        let context = &self.demo_contexts[0];
        
        // Interactive conversation flow
        let conversation_flow = vec![
            "How do I set up a new Rust project?",
            "What's the difference between cargo build and cargo check?",
            "How do I add dependencies to my Rust project?",
            "Can you explain Rust ownership?",
            "What are some best practices for error handling in Rust?",
        ];
        
        for query in conversation_flow {
            let response = self.integration.get_ai_assistance(query.to_string(), context).await?;
            
            info!("Query: {}", query);
            info!("Response: {}", response.content.chars().take(150).collect::<String>());
            info!("Confidence: {:.2}", response.confidence);
        }
        
        Ok(())
    }
    
    /// Demo context awareness
    async fn demo_context_awareness(&mut self) -> Result<()> {
        info!("🧠 Demo: Context Awareness");
        
        let rust_context = &self.demo_contexts[0];
        let node_context = &self.demo_contexts[2];
        let python_context = &self.demo_contexts[3];
        
        // Same query in different contexts should yield different responses
        let query = "How do I run tests in this project?";
        
        let rust_response = self.integration.get_ai_assistance(query.to_string(), rust_context).await?;
        let node_response = self.integration.get_ai_assistance(query.to_string(), node_context).await?;
        let python_response = self.integration.get_ai_assistance(query.to_string(), python_context).await?;
        
        info!("Query: {}", query);
        info!("Rust context response: {}", rust_response.content);
        info!("Node.js context response: {}", node_response.content);
        info!("Python context response: {}", python_response.content);
        
        Ok(())
    }
    
    /// Demo directory navigation
    async fn demo_directory_navigation(&mut self) -> Result<()> {
        info!("📁 Demo: Directory Navigation");
        
        let context = &self.demo_contexts[0];
        
        // Simulate directory changes
        let directory_changes = vec![
            (PathBuf::from("/tmp"), PathBuf::from("/tmp/rust-project")),
            (PathBuf::from("/tmp/rust-project"), PathBuf::from("/tmp/rust-project/src")),
            (PathBuf::from("/tmp/rust-project/src"), PathBuf::from("/tmp/node-project")),
        ];
        
        for (old_path, new_path) in directory_changes {
            let suggestions = self.integration
                .handle_directory_change(old_path, new_path.clone(), context)
                .await?;
            
            info!("Directory changed to: {}", new_path.display());
            info!("  Contextual suggestions: {}", suggestions.len());
            for suggestion in suggestions {
                info!("    - {}", suggestion);
            }
        }
        
        Ok(())
    }
    
    /// Demo project-specific assistance
    async fn demo_project_specific_assistance(&mut self) -> Result<()> {
        info!("🏗️ Demo: Project-Specific Assistance");
        
        // Test different project types
        let project_scenarios = vec![
            (&self.demo_contexts[0], "Rust", vec!["cargo check", "cargo test", "cargo clippy"]),
            (&self.demo_contexts[2], "Node.js", vec!["npm test", "npm audit", "npm run build"]),
            (&self.demo_contexts[3], "Python", vec!["python -m pytest", "pip install -r requirements.txt"]),
        ];
        
        for (context, project_type, commands) in project_scenarios {
            info!("Testing {} project assistance:", project_type);
            
            for command in commands {
                let result = self.integration.process_command(command.to_string(), context).await?;
                info!("  Command: {} - Suggestions: {}", command, result.suggestions.len());
            }
        }
        
        Ok(())
    }
    
    /// Demo advanced integration features
    async fn demo_advanced_integration(&mut self) -> Result<()> {
        info!("🚀 Demo: Advanced Integration Features");
        
        let context = &self.demo_contexts[0];
        
        // Test complex workflow
        let workflow_commands = vec![
            "git checkout -b feature/new-component",
            "cargo new sub-project --lib",
            "cd sub-project",
            "cargo add serde",
            "cargo test",
            "git add .",
            "git commit -m 'Add new sub-project'",
        ];
        
        for command in workflow_commands {
            let result = self.integration.process_command(command.to_string(), context).await?;
            
            // Simulate successful execution
            let _analysis = self.integration
                .handle_command_result(
                    command.to_string(),
                    0,
                    "Command executed successfully".to_string(),
                    context,
                )
                .await?;
            
            info!("Workflow step: {} - Completed", command);
        }
        
        // Get workflow summary from AI
        let summary = self.integration
            .get_ai_assistance(
                "Can you summarize the workflow we just completed?".to_string(),
                context,
            )
            .await?;
        
        info!("Workflow Summary: {}", summary.content);
        
        Ok(())
    }
    
    /// Show integration statistics
    async fn show_integration_statistics(&self) -> Result<()> {
        info!("📊 Integration Statistics");
        
        let stats = self.integration.get_integration_stats().await;
        
        info!("Session Statistics:");
        info!("  Events processed: {}", stats.events_processed);
        info!("  Commands assisted: {}", stats.commands_assisted);
        info!("  Conversations created: {}", stats.conversations_created);
        info!("  Suggestions generated: {}", stats.suggestions_generated);
        info!("  Errors analyzed: {}", stats.errors_analyzed);
        info!("  Context switches: {}", stats.context_switches);
        info!("  Session duration: {:?}", stats.session_duration);
        info!("  Assistance effectiveness: {:.2}%", stats.assistance_effectiveness * 100.0);
        
        info!("Most used commands:");
        for (command, count) in stats.most_used_commands.iter().take(5) {
            info!("  {}: {} times", command, count);
        }
        
        info!("Error patterns:");
        for (pattern, count) in stats.error_patterns.iter().take(3) {
            info!("  {}: {} occurrences", pattern, count);
        }
        
        Ok(())
    }
    
    /// Create demo contexts for different scenarios
    fn create_demo_contexts() -> Vec<PtyAiContext> {
        vec![
            // Rust project context
            PtyAiContext {
                terminal_context: TerminalContext {
                    working_directory: PathBuf::from("/tmp/rust-project"),
                    git_branch: Some("main".to_string()),
                    git_status: Some("On branch main\nnothing to commit, working tree clean".to_string()),
                    project_info: Some(ProjectInfo {
                        project_type: ProjectType::Rust,
                        config_files: vec!["Cargo.toml".to_string()],
                        dependencies: vec!["serde".to_string(), "tokio".to_string()],
                    }),
                    last_command: Some("cargo check".to_string()),
                    last_exit_code: Some(0),
                    shell_type: ShellType::Bash,
                },
                last_output: Some("Checking hello-world v0.1.0\nFinished dev [unoptimized + debuginfo] target(s) in 0.85s".to_string()),
                error_context: None,
                suggestions: Vec::new(),
            },
            
            // Error context
            PtyAiContext {
                terminal_context: TerminalContext {
                    working_directory: PathBuf::from("/tmp/error-project"),
                    git_branch: Some("develop".to_string()),
                    git_status: Some("On branch develop\nChanges not staged for commit:\n  modified:   src/main.rs".to_string()),
                    project_info: Some(ProjectInfo {
                        project_type: ProjectType::Rust,
                        config_files: vec!["Cargo.toml".to_string()],
                        dependencies: vec!["serde".to_string()],
                    }),
                    last_command: Some("cargo build".to_string()),
                    last_exit_code: Some(1),
                    shell_type: ShellType::Bash,
                },
                last_output: None,
                error_context: Some("error[E0382]: borrow of moved value: `data`\n  --> src/main.rs:10:15".to_string()),
                suggestions: vec!["Consider using references or cloning the data".to_string()],
            },
            
            // Node.js project context
            PtyAiContext {
                terminal_context: TerminalContext {
                    working_directory: PathBuf::from("/tmp/node-project"),
                    git_branch: Some("main".to_string()),
                    git_status: Some("On branch main\nYour branch is up to date with 'origin/main'.".to_string()),
                    project_info: Some(ProjectInfo {
                        project_type: ProjectType::NodeJs,
                        config_files: vec!["package.json".to_string()],
                        dependencies: vec!["express".to_string(), "lodash".to_string()],
                    }),
                    last_command: Some("npm test".to_string()),
                    last_exit_code: Some(0),
                    shell_type: ShellType::Bash,
                },
                last_output: Some("Test Suites: 3 passed, 3 total\nTests: 15 passed, 15 total".to_string()),
                error_context: None,
                suggestions: Vec::new(),
            },
            
            // Python project context
            PtyAiContext {
                terminal_context: TerminalContext {
                    working_directory: PathBuf::from("/tmp/python-project"),
                    git_branch: Some("develop".to_string()),
                    git_status: Some("On branch develop\nnothing to commit, working tree clean".to_string()),
                    project_info: Some(ProjectInfo {
                        project_type: ProjectType::Python,
                        config_files: vec!["requirements.txt".to_string(), "setup.py".to_string()],
                        dependencies: vec!["pandas".to_string(), "numpy".to_string()],
                    }),
                    last_command: Some("python -m pytest".to_string()),
                    last_exit_code: Some(0),
                    shell_type: ShellType::Bash,
                },
                last_output: Some("collected 8 items\n8 passed in 0.12s".to_string()),
                error_context: None,
                suggestions: Vec::new(),
            },
        ]
    }
}

/// Run the complete integration demo
pub async fn run_complete_integration_demo() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    let mut demo = CompleteIntegrationDemo::new().await?;
    demo.run_demo().await?;
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    run_complete_integration_demo().await
}