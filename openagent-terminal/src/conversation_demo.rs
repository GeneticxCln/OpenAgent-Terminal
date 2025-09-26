//! Conversation Management System Demo
//!
//! This demonstration showcases the comprehensive conversation management system,
//! including conversation history, context preservation, multi-turn interactions,
//! conversation branching, and terminal workflow integration.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{info, warn, debug};

use crate::conversation_management::{
    ConversationManager, ConversationConfig, ConversationId, MessageType, ConversationQuery,
};
use crate::ai_runtime::{AiRuntime, AiProvider, AgentResponse};
use crate::ai_context_provider::{PtyAiContext, TerminalContext, ProjectInfo, ProjectType};
use crate::command_assistance::{CommandAssistanceEngine, AssistanceConfig};
use crate::blocks_v2::ShellType;

/// Comprehensive demonstration of the conversation management system
pub struct ConversationDemo {
    conversation_manager: ConversationManager,
    demo_contexts: Vec<PtyAiContext>,
}

impl ConversationDemo {
    /// Create a new conversation demo
    pub async fn new() -> Result<Self> {
        // Initialize AI runtime
        let ai_runtime = Arc::new(RwLock::new(
            AiRuntime::new(vec![AiProvider::Ollama]).await?
        ));
        
        // Initialize command assistance
        let assistance_config = AssistanceConfig::default();
        let command_assistance = Arc::new(RwLock::new(
            CommandAssistanceEngine::new(assistance_config).await?
        ));
        
        // Create conversation config
        let conversation_config = ConversationConfig::default();
        
        // Initialize conversation manager
        let mut conversation_manager = ConversationManager::new(
            conversation_config,
            ai_runtime,
            command_assistance,
        ).await?;
        
        conversation_manager.start().await?;
        
        // Create demo contexts
        let demo_contexts = Self::create_demo_contexts();
        
        Ok(Self {
            conversation_manager,
            demo_contexts,
        })
    }
    
    /// Run the complete conversation management demo
    pub async fn run_demo(&mut self) -> Result<()> {
        info!("🎯 Starting Conversation Management System Demo");
        
        // Demo 1: Basic conversation creation and interaction
        self.demo_basic_conversation().await?;
        
        // Demo 2: Multi-turn conversations with context preservation
        self.demo_multi_turn_conversation().await?;
        
        // Demo 3: Conversation branching
        self.demo_conversation_branching().await?;
        
        // Demo 4: Context-aware responses
        self.demo_context_aware_responses().await?;
        
        // Demo 5: Conversation search and filtering
        self.demo_conversation_search().await?;
        
        // Demo 6: Terminal workflow integration
        self.demo_terminal_integration().await?;
        
        // Demo 7: Conversation statistics and analytics
        self.demo_statistics().await?;
        
        // Demo 8: Error handling and recovery
        self.demo_error_handling().await?;
        
        // Demo 9: Advanced features
        self.demo_advanced_features().await?;
        
        info!("✅ Conversation Management System Demo completed successfully");
        Ok(())
    }
    
    /// Demo 1: Basic conversation creation and interaction
    async fn demo_basic_conversation(&mut self) -> Result<()> {
        info!("📝 Demo 1: Basic Conversation Creation and Interaction");
        
        let context = &self.demo_contexts[0];
        
        // Create a new conversation
        let conversation_id = self.conversation_manager
            .create_conversation(
                Some("Getting Started with Rust".to_string()),
                context,
            )
            .await?;
        
        info!("Created conversation: {}", conversation_id);
        
        // Add some messages
        let _user_msg = self.conversation_manager
            .add_message(
                conversation_id,
                MessageType::User,
                "How do I create a new Rust project?".to_string(),
                Some(context.clone()),
            )
            .await?;
        
        let _assistant_msg = self.conversation_manager
            .add_message(
                conversation_id,
                MessageType::Assistant,
                "To create a new Rust project, use `cargo new project_name`. This creates a new directory with a basic Rust project structure including Cargo.toml and src/main.rs files.".to_string(),
                Some(context.clone()),
            )
            .await?;
        
        let _user_follow_up = self.conversation_manager
            .add_message(
                conversation_id,
                MessageType::User,
                "What about adding dependencies?".to_string(),
                Some(context.clone()),
            )
            .await?;
        
        // Get conversation history
        let history = self.conversation_manager
            .get_conversation_history(conversation_id, Some(5))
            .await?;
        
        info!("📚 Conversation history ({} messages):", history.len());
        for (i, message) in history.iter().enumerate() {
            info!("  {}: {:?} - {}", i + 1, message.message_type, 
                  message.content.chars().take(50).collect::<String>());
        }
        
        Ok(())
    }
    
    /// Demo 2: Multi-turn conversations with context preservation
    async fn demo_multi_turn_conversation(&mut self) -> Result<()> {
        info!("🔄 Demo 2: Multi-turn Conversations with Context Preservation");
        
        let git_context = &self.demo_contexts[1]; // Git project context
        
        // Create conversation
        let conversation_id = self.conversation_manager
            .create_conversation(
                Some("Git Workflow Help".to_string()),
                git_context,
            )
            .await?;
        
        // Simulate a multi-turn conversation about Git workflow
        let conversation_flow = vec![
            ("I'm having trouble with Git branches", MessageType::User),
            ("I can help with Git branches! What specific issue are you facing?", MessageType::Assistant),
            ("I want to create a feature branch", MessageType::User),
            ("To create a new feature branch, use `git checkout -b feature-name`. This creates and switches to the new branch.", MessageType::Assistant),
            ("How do I merge it back to main?", MessageType::User),
            ("First switch to main with `git checkout main`, then use `git merge feature-name`. Make sure to pull latest changes first with `git pull`.", MessageType::Assistant),
            ("What about conflicts?", MessageType::User),
            ("Git conflicts occur when the same lines are modified in both branches. Git will mark conflicted files, and you'll need to manually resolve them by editing the files and removing conflict markers.", MessageType::Assistant),
        ];
        
        // Add each message in the conversation flow
        for (content, msg_type) in conversation_flow {
            let _msg_id = self.conversation_manager
                .add_message(
                    conversation_id,
                    msg_type,
                    content.to_string(),
                    Some(git_context.clone()),
                )
                .await?;
            
            // Small delay to simulate realistic conversation timing
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        // Test process_user_input method for context-aware response
        let response = self.conversation_manager
            .process_user_input(
                "Can you show me the exact commands for the full workflow?".to_string(),
                git_context,
            )
            .await?;
        
        info!("🤖 Context-aware AI response: {}", response.content);
        
        // Get complete history
        let full_history = self.conversation_manager
            .get_conversation_history(conversation_id, None)
            .await?;
        
        info!("📊 Full conversation: {} messages", full_history.len());
        
        Ok(())
    }
    
    /// Demo 3: Conversation branching
    async fn demo_conversation_branching(&mut self) -> Result<()> {
        info!("🌿 Demo 3: Conversation Branching");
        
        let node_context = &self.demo_contexts[2]; // Node.js project context
        
        // Create main conversation
        let main_conversation = self.conversation_manager
            .create_conversation(
                Some("Node.js Development Help".to_string()),
                node_context,
            )
            .await?;
        
        // Add initial conversation
        let messages = vec![
            "I need help with Node.js performance optimization",
            "There are several approaches to optimize Node.js performance. What specific area concerns you most?",
            "My API responses are slow",
            "API performance can be improved through caching, database optimization, and async patterns.",
            "Tell me about caching strategies",
        ];
        
        for (i, message) in messages.iter().enumerate() {
            let msg_type = if i % 2 == 0 { MessageType::User } else { MessageType::Assistant };
            self.conversation_manager
                .add_message(
                    main_conversation,
                    msg_type,
                    message.to_string(),
                    Some(node_context.clone()),
                )
                .await?;
        }
        
        // Create branch for caching discussion
        let caching_branch = self.conversation_manager
            .create_branch(
                main_conversation,
                4, // Branch after the caching question
                "Caching Strategies Discussion".to_string(),
            )
            .await?;
        
        info!("Created caching branch: {}", caching_branch);
        
        // Continue caching branch
        self.conversation_manager
            .add_message(
                caching_branch,
                MessageType::Assistant,
                "For Node.js caching, consider: 1) Redis for distributed caching, 2) Memory caching with node-cache, 3) HTTP caching headers, 4) Database query result caching.".to_string(),
                Some(node_context.clone()),
            )
            .await?;
        
        // Create another branch for database optimization
        let db_branch = self.conversation_manager
            .create_branch(
                main_conversation,
                3, // Branch after API performance response
                "Database Optimization".to_string(),
            )
            .await?;
        
        info!("Created database branch: {}", db_branch);
        
        // Continue database branch
        self.conversation_manager
            .add_message(
                db_branch,
                MessageType::User,
                "What about database optimization specifically?".to_string(),
                Some(node_context.clone()),
            )
            .await?;
        
        self.conversation_manager
            .add_message(
                db_branch,
                MessageType::Assistant,
                "Database optimization includes: indexing frequently queried columns, connection pooling, query optimization, using database views, and implementing database-level caching.".to_string(),
                Some(node_context.clone()),
            )
            .await?;
        
        // Demonstrate switching between conversations
        self.conversation_manager
            .switch_conversation(caching_branch)
            .await?;
        
        info!("✅ Successfully demonstrated conversation branching with {} branches", 2);
        
        Ok(())
    }
    
    /// Demo 4: Context-aware responses based on terminal state
    async fn demo_context_aware_responses(&mut self) -> Result<()> {
        info!("🧠 Demo 4: Context-Aware Responses");
        
        // Create different contexts for different scenarios
        let python_context = &self.demo_contexts[3];
        let rust_context = &self.demo_contexts[4];
        
        // Python project conversation
        let python_conv = self.conversation_manager
            .create_conversation(
                Some("Python Development".to_string()),
                python_context,
            )
            .await?;
        
        // Add context-specific interaction
        self.conversation_manager
            .add_message(
                python_conv,
                MessageType::CommandResult,
                "ModuleNotFoundError: No module named 'requests'".to_string(),
                Some(python_context.clone()),
            )
            .await?;
        
        let python_response = self.conversation_manager
            .process_user_input(
                "I'm getting a module error".to_string(),
                python_context,
            )
            .await?;
        
        info!("🐍 Python context response: {}", python_response.content);
        
        // Rust project conversation
        let rust_conv = self.conversation_manager
            .create_conversation(
                Some("Rust Development".to_string()),
                rust_context,
            )
            .await?;
        
        // Add Rust-specific context
        self.conversation_manager
            .add_message(
                rust_conv,
                MessageType::CommandResult,
                "error[E0382]: borrow of moved value: `data`".to_string(),
                Some(rust_context.clone()),
            )
            .await?;
        
        let rust_response = self.conversation_manager
            .process_user_input(
                "I'm getting a borrow checker error".to_string(),
                rust_context,
            )
            .await?;
        
        info!("🦀 Rust context response: {}", rust_response.content);
        
        info!("✅ Context-aware responses demonstrated for different project types");
        
        Ok(())
    }
    
    /// Demo 5: Conversation search and filtering
    async fn demo_conversation_search(&mut self) -> Result<()> {
        info!("🔍 Demo 5: Conversation Search and Filtering");
        
        // Search for conversations about "Git"
        let git_query = ConversationQuery {
            text_search: Some("Git".to_string()),
            ..Default::default()
        };
        
        let git_results = self.conversation_manager
            .search_conversations(git_query)
            .await?;
        
        info!("Found {} conversations about Git", git_results.len());
        
        // Search for error conversations
        let error_query = ConversationQuery {
            has_errors: Some(true),
            ..Default::default()
        };
        
        let error_results = self.conversation_manager
            .search_conversations(error_query)
            .await?;
        
        info!("Found {} conversations with errors", error_results.len());
        
        // Search by working directory
        let dir_query = ConversationQuery {
            working_directory: Some(PathBuf::from("/tmp/rust-project")),
            ..Default::default()
        };
        
        let dir_results = self.conversation_manager
            .search_conversations(dir_query)
            .await?;
        
        info!("Found {} conversations in /tmp/rust-project", dir_results.len());
        
        // Search by message type
        let user_query = ConversationQuery {
            message_type: Some(MessageType::User),
            limit: Some(5),
            ..Default::default()
        };
        
        let user_results = self.conversation_manager
            .search_conversations(user_query)
            .await?;
        
        info!("Found {} conversations with user messages (limited to 5)", user_results.len());
        
        Ok(())
    }
    
    /// Demo 6: Terminal workflow integration
    async fn demo_terminal_integration(&mut self) -> Result<()> {
        info!("💻 Demo 6: Terminal Workflow Integration");
        
        let context = &self.demo_contexts[0];
        
        // Create conversation for terminal workflow
        let term_conv = self.conversation_manager
            .create_conversation(
                Some("Terminal Workflow".to_string()),
                context,
            )
            .await?;
        
        // Simulate terminal command execution and results
        let command_sequence = vec![
            ("ls -la", "total 24\ndrwxr-xr-x  6 user user  192 Jan 15 10:30 .\ndrwxr-xr-x 10 user user  320 Jan 15 10:25 ..\n-rw-r--r--  1 user user  150 Jan 15 10:30 Cargo.toml\ndrwxr-xr-x  2 user user   64 Jan 15 10:30 src", 0),
            ("cargo build", "Compiling hello-world v0.1.0 (/tmp/rust-project)\nFinished dev [unoptimized + debuginfo] target(s) in 2.34s", 0),
            ("cargo test", "test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out", 0),
            ("git status", "On branch main\nnothing to commit, working tree clean", 0),
        ];
        
        for (command, output, exit_code) in command_sequence {
            // Add command as user message
            self.conversation_manager
                .add_message(
                    term_conv,
                    MessageType::User,
                    format!("$ {}", command),
                    Some(context.clone()),
                )
                .await?;
            
            // Add command result
            let result_type = if exit_code == 0 { MessageType::CommandResult } else { MessageType::Error };
            self.conversation_manager
                .add_message(
                    term_conv,
                    result_type,
                    output.to_string(),
                    Some(context.clone()),
                )
                .await?;
            
            // Get AI assistance for the command result
            if exit_code != 0 {
                let error_response = self.conversation_manager
                    .process_user_input(
                        format!("The command '{}' failed with output: {}", command, output),
                        context,
                    )
                    .await?;
                
                info!("🚨 Error assistance: {}", error_response.content);
            }
        }
        
        // Ask for workflow summary
        let summary_response = self.conversation_manager
            .process_user_input(
                "Can you summarize what we've done in this terminal session?".to_string(),
                context,
            )
            .await?;
        
        info!("📋 Workflow summary: {}", summary_response.content);
        
        Ok(())
    }
    
    /// Demo 7: Conversation statistics and analytics
    async fn demo_statistics(&mut self) -> Result<()> {
        info!("📊 Demo 7: Conversation Statistics and Analytics");
        
        // Get current statistics
        let stats = self.conversation_manager.get_statistics().await;
        
        info!("📈 Current Statistics:");
        info!("  Total conversations: {}", stats.total_conversations);
        info!("  Active conversations: {}", stats.active_conversations);
        info!("  Total messages: {}", stats.total_messages);
        info!("  Average conversation length: {:.2}", stats.average_conversation_length);
        
        if let Some(ref active_dir) = stats.most_active_directory {
            info!("  Most active directory: {}", active_dir.display());
        }
        
        info!("  Command patterns: {} tracked", stats.common_command_patterns.len());
        info!("  Error frequencies: {} types tracked", stats.error_frequencies.len());
        info!("  Context compression ratio: {:.2}", stats.context_compression_ratio);
        info!("  Storage usage: {} bytes", stats.storage_usage_bytes);
        
        Ok(())
    }
    
    /// Demo 8: Error handling and recovery
    async fn demo_error_handling(&mut self) -> Result<()> {
        info!("⚠️  Demo 8: Error Handling and Recovery");
        
        let context = &self.demo_contexts[0];
        
        // Create conversation for error handling
        let error_conv = self.conversation_manager
            .create_conversation(
                Some("Error Recovery Demo".to_string()),
                context,
            )
            .await?;
        
        // Simulate various error scenarios
        let error_scenarios = vec![
            ("Command not found: npm", "bash: npm: command not found", MessageType::Error),
            ("Permission denied", "mkdir: cannot create directory '/etc/test': Permission denied", MessageType::Error),
            ("Compilation error", "error: cannot find function `undefined_function` in this scope", MessageType::Error),
            ("Network timeout", "curl: (28) Operation timed out after 30000 milliseconds", MessageType::Error),
        ];
        
        for (description, error_output, msg_type) in error_scenarios {
            // Add error message
            let error_msg_id = self.conversation_manager
                .add_message(
                    error_conv,
                    msg_type,
                    error_output.to_string(),
                    Some(context.clone()),
                )
                .await?;
            
            info!("Added error message: {}", error_msg_id);
            
            // Generate recovery suggestion
            let recovery_response = self.conversation_manager
                .process_user_input(
                    format!("How do I fix this error: {}?", description),
                    context,
                )
                .await?;
            
            info!("🔧 Recovery suggestion for '{}': {}", description, 
                  recovery_response.content.chars().take(100).collect::<String>());
        }
        
        Ok(())
    }
    
    /// Demo 9: Advanced features
    async fn demo_advanced_features(&mut self) -> Result<()> {
        info!("🚀 Demo 9: Advanced Features");
        
        // Archive old conversations
        let archived_count = self.conversation_manager
            .archive_old_conversations()
            .await?;
        
        info!("📦 Archived {} old conversations", archived_count);
        
        // Test conversation limits and cleanup
        info!("🧹 Testing automatic cleanup features");
        
        // Get final statistics
        let final_stats = self.conversation_manager.get_statistics().await;
        info!("📊 Final statistics: {} active conversations", final_stats.active_conversations);
        
        // Demonstrate graceful shutdown
        info!("🛑 Demonstrating graceful shutdown");
        // Note: In a real application, you would call conversation_manager.stop() here
        
        Ok(())
    }
    
    /// Create demo contexts for different scenarios
    fn create_demo_contexts() -> Vec<PtyAiContext> {
        vec![
            // Basic Rust project context
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
                last_output: Some("Checking hello-world v0.1.0 (/tmp/rust-project)\nFinished dev [unoptimized + debuginfo] target(s) in 0.85s".to_string()),
                error_context: None,
                suggestions: Vec::new(),
            },
            
            // Git project context
            PtyAiContext {
                terminal_context: TerminalContext {
                    working_directory: PathBuf::from("/projects/git-workflow"),
                    git_branch: Some("feature/new-component".to_string()),
                    git_status: Some("On branch feature/new-component\nChanges not staged for commit:\n  modified:   src/component.rs".to_string()),
                    project_info: Some(ProjectInfo {
                        project_type: ProjectType::Rust,
                        config_files: vec!["Cargo.toml".to_string(), ".gitignore".to_string()],
                        dependencies: vec!["serde".to_string(), "clap".to_string()],
                    }),
                    last_command: Some("git status".to_string()),
                    last_exit_code: Some(0),
                    shell_type: ShellType::Bash,
                },
                last_output: Some("On branch feature/new-component\nChanges not staged for commit:\n  modified:   src/component.rs".to_string()),
                error_context: None,
                suggestions: Vec::new(),
            },
            
            // Node.js project context
            PtyAiContext {
                terminal_context: TerminalContext {
                    working_directory: PathBuf::from("/projects/node-api"),
                    git_branch: Some("main".to_string()),
                    git_status: Some("On branch main\nYour branch is up to date with 'origin/main'.".to_string()),
                    project_info: Some(ProjectInfo {
                        project_type: ProjectType::NodeJs,
                        config_files: vec!["package.json".to_string(), "package-lock.json".to_string()],
                        dependencies: vec!["express".to_string(), "mongoose".to_string(), "dotenv".to_string()],
                    }),
                    last_command: Some("npm test".to_string()),
                    last_exit_code: Some(0),
                    shell_type: ShellType::Bash,
                },
                last_output: Some("> node-api@1.0.0 test\n> jest\n\nTest Suites: 5 passed, 5 total\nTests:       25 passed, 25 total".to_string()),
                error_context: None,
                suggestions: Vec::new(),
            },
            
            // Python project context with error
            PtyAiContext {
                terminal_context: TerminalContext {
                    working_directory: PathBuf::from("/projects/python-ml"),
                    git_branch: Some("develop".to_string()),
                    git_status: Some("On branch develop\nnothing to commit, working tree clean".to_string()),
                    project_info: Some(ProjectInfo {
                        project_type: ProjectType::Python,
                        config_files: vec!["requirements.txt".to_string(), "setup.py".to_string()],
                        dependencies: vec!["pandas".to_string(), "numpy".to_string(), "scikit-learn".to_string()],
                    }),
                    last_command: Some("python main.py".to_string()),
                    last_exit_code: Some(1),
                    shell_type: ShellType::Bash,
                },
                last_output: None,
                error_context: Some("ModuleNotFoundError: No module named 'requests'".to_string()),
                suggestions: vec!["Install the requests module with 'pip install requests'".to_string()],
            },
            
            // Rust project with compilation error
            PtyAiContext {
                terminal_context: TerminalContext {
                    working_directory: PathBuf::from("/projects/rust-advanced"),
                    git_branch: Some("experimental".to_string()),
                    git_status: Some("On branch experimental\nChanges to be committed:\n  modified:   src/lib.rs".to_string()),
                    project_info: Some(ProjectInfo {
                        project_type: ProjectType::Rust,
                        config_files: vec!["Cargo.toml".to_string()],
                        dependencies: vec!["tokio".to_string(), "serde_json".to_string()],
                    }),
                    last_command: Some("cargo build".to_string()),
                    last_exit_code: Some(1),
                    shell_type: ShellType::Bash,
                },
                last_output: None,
                error_context: Some("error[E0382]: borrow of moved value: `data`\n  --> src/lib.rs:45:22".to_string()),
                suggestions: vec!["Consider cloning the data or using references".to_string()],
            },
        ]
    }
}

/// Mock AI runtime for demonstration purposes
impl AiRuntime {
    /// Create a mock AI runtime for demo
    pub async fn new(providers: Vec<AiProvider>) -> Result<Self> {
        Ok(Self {
            providers,
            current_provider: AiProvider::Ollama,
        })
    }
    
    /// Mock prompt submission
    pub async fn submit_prompt(&self, prompt: String, _context: Option<String>) -> Result<AgentResponse> {
        // Generate contextual response based on prompt content
        let content = if prompt.contains("Git") || prompt.contains("git") {
            "I can help you with Git commands and workflows. Git is a distributed version control system that tracks changes in your code."
        } else if prompt.contains("Rust") || prompt.contains("cargo") {
            "Rust is a systems programming language focused on safety and performance. Use `cargo` commands to manage Rust projects."
        } else if prompt.contains("Node") || prompt.contains("npm") {
            "Node.js is a JavaScript runtime for server-side development. Use `npm` to manage packages and dependencies."
        } else if prompt.contains("Python") || prompt.contains("pip") {
            "Python is a versatile programming language. Use `pip` to install packages and manage dependencies."
        } else if prompt.contains("error") || prompt.contains("Error") {
            "I can help you debug and resolve errors. Please share the specific error message and context."
        } else {
            "I'm here to help with your development questions. What specific issue are you working on?"
        };
        
        Ok(AgentResponse {
            content: content.to_string(),
            confidence: 0.9,
            sources: vec!["demo-ai-assistant".to_string()],
            metadata: HashMap::new(),
        })
    }
}

/// Run the conversation management demo
pub async fn run_conversation_demo() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    let mut demo = ConversationDemo::new().await?;
    demo.run_demo().await?;
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    run_conversation_demo().await
}