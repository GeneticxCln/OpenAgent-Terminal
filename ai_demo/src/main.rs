//! Standalone AI Terminal Integration Demo
//!
//! This is a complete, self-contained example showing how our AI terminal
//! integration system works. It demonstrates the event-driven architecture
//! where AI agents respond to terminal events in real-time.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, RwLock, Mutex};
use tracing::{info, warn};

use ai_integration_demo::{providers, security, analysis};
use ai_integration_demo::types::{AiProvider, ShellType};
// Types moved to library (ai_integration_demo::types)

#[derive(Debug, Clone)]
pub enum TerminalEventType {
    CommandExecuted {
        command: String,
        exit_code: i32,
        output: String,
        error_output: String,
        working_directory: PathBuf,
    },
    CommandFailed {
        command: String,
        error: String,
        exit_code: i32,
        working_directory: PathBuf,
    },
    DirectoryChanged {
        old_path: PathBuf,
        new_path: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub content: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct AiAgent {
    pub id: String,
    pub name: String,
    pub provider: AiProvider,
    pub enabled: bool,
    pub last_activation: Option<Instant>,
}

impl AiAgent {
    pub fn new(id: String, name: String, provider: AiProvider) -> Self {
        Self {
            id,
            name,
            provider,
            enabled: true,
            last_activation: None,
        }
    }

    fn provider_kind(&self) -> providers::ProviderKind {
        match self.provider {
            AiProvider::OpenAI => providers::ProviderKind::OpenAI,
            AiProvider::Anthropic => providers::ProviderKind::Anthropic,
            AiProvider::Ollama => providers::ProviderKind::Ollama,
            AiProvider::OpenRouter => providers::ProviderKind::OpenRouter,
        }
    }

    pub fn should_activate(&self, event: &TerminalEventType) -> bool {
        if !self.enabled {
            return false;
        }

        // Simple activation logic - respond to command failures
        match event {
            TerminalEventType::CommandFailed { .. } => true,
            TerminalEventType::DirectoryChanged { .. } => true,
            _ => false,
        }
    }

    pub async fn process_event(&mut self, event: &TerminalEventType) -> Result<AgentResponse, Box<dyn std::error::Error + Send + Sync>> {
        self.last_activation = Some(Instant::now());

        let providers = providers::AiProviders::new(providers::ProviderConfig::default())?;

        let response_content = match event {
            TerminalEventType::CommandFailed { command, error, exit_code, working_directory } => {
                // Perform security risk analysis
                let risk = security::analyze_command(command);
                // Build analysis prompt and call provider
                let system_prompt = analysis::system_prompt();
                let user_prompt = analysis::error_prompt(command, error, *exit_code, Some(working_directory));
let model = analysis::select_model(self.provider.clone());
                let content = providers.chat(self.provider_kind(), &model, &system_prompt, &user_prompt).await.unwrap_or_else(|e| format!("Analysis unavailable: {}", e));
                // Prepend risk summary
                let mut full = String::new();
                if risk.level != security::RiskLevel::Low {
                    full.push_str(&format!("SECURITY WARNING [{}]: {}\n", risk.level.as_str(), risk.summary));
                    if !risk.findings.is_empty() {
                        full.push_str("Findings:\n");
                        for f in &risk.findings { full.push_str(&format!("- {}\n", f)); }
                    }
                    if let Some(sugg) = &risk.suggestion { full.push_str(&format!("Suggested safer alternative: {}\n\n", sugg)); }
                }
                full.push_str(&content);
                full
            }
            TerminalEventType::CommandExecuted { command, exit_code, output, error_output, working_directory } => {
                let risk = security::analyze_command(command);
                let system_prompt = analysis::system_prompt();
                let user_prompt = analysis::success_prompt(command, *exit_code, output, error_output, Some(working_directory));
let model = analysis::select_model(self.provider.clone());
                let content = providers.chat(self.provider_kind(), &model, &system_prompt, &user_prompt).await.unwrap_or_else(|e| format!("Analysis unavailable: {}", e));
                let mut full = String::new();
                if risk.level != security::RiskLevel::Low {
                    full.push_str(&format!("SECURITY WARNING [{}]: {}\n\n", risk.level.as_str(), risk.summary));
                }
                full.push_str(&content);
                full
            }
            TerminalEventType::DirectoryChanged { new_path, .. } => {
                // Real file checks
                if path_contains(new_path, ".git") {
                    "You're now in a Git repository. Try `git --no-pager status` to see the current state.".to_string()
                } else if path_contains(new_path, "package.json") {
                    "This looks like a Node.js project. Try `npm install` to install dependencies.".to_string()
                } else if path_contains(new_path, "Cargo.toml") {
                    "This is a Rust project. Try `cargo build` to compile or `cargo run` to run.".to_string()
                } else {
                    format!("Now in directory: {}. Use `ls -la` to see contents.", new_path.display())
                }
            }
        };

        Ok(AgentResponse {
            content: response_content,
            metadata: HashMap::from([
                ("agent_id".to_string(), self.id.clone()),
                ("timestamp".to_string(), chrono::Utc::now().to_rfc3339()),
            ]),
        })
    }
}

pub struct AiEventIntegrator {
    agents: Arc<RwLock<HashMap<String, AiAgent>>>,
    event_sender: mpsc::UnboundedSender<TerminalEventType>,
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<TerminalEventType>>>,
    response_sender: mpsc::UnboundedSender<AgentResponse>,
    response_receiver: Arc<Mutex<mpsc::UnboundedReceiver<AgentResponse>>>,
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl AiEventIntegrator {
    pub fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let (response_sender, response_receiver) = mpsc::unbounded_channel();

        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            response_sender,
            response_receiver: Arc::new(Mutex::new(response_receiver)),
            task_handle: None,
        }
    }

    pub async fn register_agent(&self, agent: AiAgent) {
        let mut agents = self.agents.write().await;
        let id = agent.id.clone();
        agents.insert(id.clone(), agent);
        info!("Registered AI agent: {}", id);
    }

    pub fn get_event_sender(&self) -> mpsc::UnboundedSender<TerminalEventType> {
        self.event_sender.clone()
    }

    pub fn get_response_receiver(&self) -> Arc<Mutex<mpsc::UnboundedReceiver<AgentResponse>>> {
        Arc::clone(&self.response_receiver)
    }

    pub async fn start_processing(&mut self) {
        let agents = Arc::clone(&self.agents);
        let event_receiver = Arc::clone(&self.event_receiver);
        let response_sender = self.response_sender.clone();

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

                info!("Processing event: {:?}", event);

                // Find agents that should respond to this event
                let mut activated_agents = Vec::new();
                {
                    let agents_lock = agents.read().await;
                    for (id, agent) in agents_lock.iter() {
                        if agent.should_activate(&event) {
                            activated_agents.push((id.clone(), agent.clone()));
                        }
                    }
                }

                // Process responses from activated agents
                for (agent_id, mut agent) in activated_agents {
                    match agent.process_event(&event).await {
                        Ok(response) => {
                            info!("Agent {} generated response: {}", agent_id, response.content);
                            if let Err(e) = response_sender.send(response) {
                                warn!("Failed to send agent response: {}", e);
                            }

                            // Update agent in collection
                            let mut agents_lock = agents.write().await;
                            agents_lock.insert(agent_id, agent);
                        }
                        Err(e) => {
                            warn!("Agent {} failed to process event: {}", agent_id, e);
                        }
                    }
                }
            }
        });

        self.task_handle = Some(handle);
    }

    pub async fn stop_processing(&mut self) {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
            info!("AI event processing stopped");
        }
    }
}

// Demo terminal integration
pub struct TerminalEventBridge {
    ai_integrator: Arc<Mutex<AiEventIntegrator>>,
    current_directory: PathBuf,
}

impl TerminalEventBridge {
    pub fn new(ai_integrator: Arc<Mutex<AiEventIntegrator>>, initial_directory: PathBuf) -> Self {
        Self {
            ai_integrator,
            current_directory: initial_directory,
        }
    }

    pub async fn handle_command_execution(
        &mut self,
        command: String,
        exit_code: i32,
        output: String,
        error_output: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let event = if exit_code == 0 {
            TerminalEventType::CommandExecuted {
                command,
                exit_code,
                output,
                error_output,
                working_directory: self.current_directory.clone(),
            }
        } else {
            TerminalEventType::CommandFailed {
                command,
                error: if !error_output.is_empty() { error_output } else { "Command failed".to_string() },
                exit_code,
                working_directory: self.current_directory.clone(),
            }
        };

        let integrator = self.ai_integrator.lock().await;
        integrator.get_event_sender().send(event)?;
        Ok(())
    }

    pub async fn handle_directory_change(&mut self, new_directory: PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let old_directory = self.current_directory.clone();
        self.current_directory = new_directory.clone();

        let event = TerminalEventType::DirectoryChanged {
            old_path: old_directory,
            new_path: new_directory,
        };

        let integrator = self.ai_integrator.lock().await;
        integrator.get_event_sender().send(event)?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting AI Terminal Integration Demo");

    // Create AI event integrator
    let mut ai_integrator = AiEventIntegrator::new();

    // Create and register AI agents
    // Default to OpenRouter if API key is present, otherwise Ollama
    let default_provider = if std::env::var("OPENROUTER_API_KEY").is_ok() {
        AiProvider::OpenRouter
    } else if std::env::var("OPENAI_API_KEY").is_ok() {
        AiProvider::OpenAI
    } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        AiProvider::Anthropic
    } else {
        AiProvider::Ollama
    };

    let error_analyzer = AiAgent::new(
        "error_analyzer".to_string(),
        "Error Analyzer".to_string(),
        default_provider.clone(),
    );

    let directory_advisor = AiAgent::new(
        "directory_advisor".to_string(),
        "Directory Advisor".to_string(),
        default_provider,
    );

    ai_integrator.register_agent(error_analyzer).await;
    ai_integrator.register_agent(directory_advisor).await;

    // Start AI processing
    ai_integrator.start_processing().await;

    // Create terminal event bridge
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp"));
    let integrator_arc = Arc::new(Mutex::new(ai_integrator));
    let mut terminal_bridge = TerminalEventBridge::new(Arc::clone(&integrator_arc), current_dir.clone());

    // Set up response listening
    let response_receiver = {
        let integrator = integrator_arc.lock().await;
        integrator.get_response_receiver()
    };

    // Start response processing task
    let response_task = tokio::spawn(async move {
        let mut receiver = response_receiver.lock().await;
        while let Some(response) = receiver.recv().await {
            println!("🤖 AI Assistant: {}", response.content);
        }
    });

    // Demo scenarios
    info!("📋 Demo 1: Command Failure Analysis");
    terminal_bridge.handle_command_execution(
        "git status".to_string(),
        128,
        "".to_string(),
        "fatal: not a git repository (or any of the parent directories): .git".to_string(),
    ).await?;

    tokio::time::sleep(Duration::from_secs(1)).await;

    info!("📋 Demo 2: NPM Command Error");
    terminal_bridge.handle_command_execution(
        "npm install react".to_string(),
        127,
        "".to_string(),
        "npm: command not found".to_string(),
    ).await?;

    tokio::time::sleep(Duration::from_secs(1)).await;

    info!("📋 Demo 3: Python Module Error");
    terminal_bridge.handle_command_execution(
        "python -c \"import numpy\"".to_string(),
        1,
        "".to_string(),
        "ModuleNotFoundError: No module named 'numpy'".to_string(),
    ).await?;

    tokio::time::sleep(Duration::from_secs(1)).await;

    info!("📋 Demo 4: Directory Change to Git Repository");
    let git_dir = current_dir.join("test_git_repo");
    terminal_bridge.handle_directory_change(git_dir).await?;

    tokio::time::sleep(Duration::from_secs(1)).await;

    info!("📋 Demo 5: Directory Change to Node.js Project");
    let node_dir = current_dir.join("test_node_project");
    terminal_bridge.handle_directory_change(node_dir).await?;

    tokio::time::sleep(Duration::from_secs(1)).await;

    info!("📋 Demo 6: Directory Change to Rust Project");
    let rust_dir = current_dir.join("test_rust_project");
    terminal_bridge.handle_directory_change(rust_dir).await?;

    // Wait for all responses to be processed
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Clean shutdown
    {
        let mut integrator = integrator_arc.lock().await;
        integrator.stop_processing().await;
    }

    response_task.abort();

    info!("✅ Demo completed successfully!");
    info!("💡 This demonstrates how AI agents can provide real-time assistance based on terminal events:");
    info!("   - Command failures are analyzed and helpful suggestions provided");
    info!("   - Directory changes trigger context-aware advice");
    info!("   - Multiple specialized agents can respond to different event types");
    info!("   - The system is fully asynchronous and enterprise-ready");

    Ok(())
}

fn path_contains(dir: &Path, child: &str) -> bool {
    let p = dir.join(child);
    fs::metadata(p).is_ok()
}
