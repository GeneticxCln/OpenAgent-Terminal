//! AI Terminal Integration Manager
//!
//! This module provides the main coordination layer for integrating AI agents
//! with terminal operations. It manages the AI runtime, event system, and
//! provides a unified interface for terminal applications.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock, Mutex, broadcast};
use tokio::time::interval;
use tracing::{debug, info, warn, error};

use crate::ai_runtime::{AiRuntime, AiProvider, AgentRequest, AgentResponse};
use crate::ai_event_integration::{
    AiEventIntegrator, TerminalEventType, DefaultAgents, AiAgent, AgentTrigger, 
    ActivationCondition, AssistanceType
};
use crate::terminal_event_bridge::{
    TerminalEventBridge, EventBridgeConfig, BridgeStatistics, 
    DefaultTerminalIntegration, TerminalIntegration
};
use crate::blocks_v2::ShellType;
use crate::ai_runtime::AiProviderConfig;

/// Configuration for the AI terminal integration system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTerminalConfig {
    /// AI runtime configuration
    pub ai_runtime: AiRuntimeConfig,
    
    /// Event monitoring configuration
    pub event_bridge: EventBridgeConfig,
    
    /// Agent configuration
    pub agents: AgentConfig,
    
    /// Performance and resource limits
    pub performance: PerformanceConfig,
    
    /// UI integration settings
    pub ui_integration: UiIntegrationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRuntimeConfig {
    /// Default AI provider to use
    pub default_provider: AiProvider,
    
    /// Available AI providers and their configurations
    pub providers: HashMap<AiProvider, ProviderConfig>,
    
    /// Enable conversation history persistence
    pub persist_conversations: bool,
    
    /// Maximum conversation history length
    pub max_conversation_length: usize,
    
    /// Response timeout in milliseconds
    pub response_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// API endpoint (for remote providers)
    pub endpoint: Option<String>,
    
    /// API key or authentication token
    pub auth_token: Option<String>,
    
    /// Model name to use
    pub model: String,
    
    /// Maximum tokens per request
    pub max_tokens: u32,
    
    /// Temperature setting (0.0 - 1.0)
    pub temperature: f32,
    
    /// Enable this provider
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Enable default agents
    pub enable_default_agents: bool,
    
    /// Custom agent configurations
    pub custom_agents: Vec<CustomAgentConfig>,
    
    /// Global agent settings
    pub global_settings: GlobalAgentSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAgentConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    pub provider: AiProvider,
    pub model: String,
    pub system_prompt: String,
    pub trigger_events: Vec<String>, // Event type names
    pub activation_conditions: Vec<String>, // Condition descriptions
    pub priority: u8,
    pub debounce_seconds: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalAgentSettings {
    /// Maximum number of agents that can respond to a single event
    pub max_agents_per_event: usize,
    
    /// Global rate limit (responses per minute)
    pub global_rate_limit: u32,
    
    /// Enable agent response caching
    pub enable_response_caching: bool,
    
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Maximum concurrent AI requests
    pub max_concurrent_requests: usize,
    
    /// Event processing buffer size
    pub event_buffer_size: usize,
    
    /// Response buffer size
    pub response_buffer_size: usize,
    
    /// Enable performance monitoring
    pub enable_monitoring: bool,
    
    /// Statistics collection interval (seconds)
    pub stats_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiIntegrationConfig {
    /// Show AI responses inline in terminal
    pub show_inline_responses: bool,
    
    /// Response display format
    pub response_format: ResponseFormat,
    
    /// Enable notifications for AI responses
    pub enable_notifications: bool,
    
    /// Enable response timestamps
    pub show_timestamps: bool,
    
    /// Maximum response length to display (characters)
    pub max_display_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseFormat {
    Inline,
    Sidebar,
    Popup,
    Notification,
}

/// Implement conversion from ProviderConfig to AiProviderConfig
impl From<ProviderConfig> for AiProviderConfig {
    fn from(config: ProviderConfig) -> Self {
        Self {
            enabled: config.enabled,
            api_key: config.auth_token,
            base_url: config.endpoint,
            model: config.model,
            max_tokens: Some(config.max_tokens),
            temperature: Some(config.temperature),
            timeout_seconds: 30, // Default timeout
        }
    }
}

impl Default for AiTerminalConfig {
    fn default() -> Self {
        Self {
            ai_runtime: AiRuntimeConfig {
                default_provider: AiProvider::Ollama,
                providers: HashMap::from([
                    (AiProvider::Ollama, ProviderConfig {
                        endpoint: Some("http://localhost:11434".to_string()),
                        auth_token: None,
                        model: "llama3:8b".to_string(),
                        max_tokens: 4096,
                        temperature: 0.7,
                        enabled: true,
                    }),
                ]),
                persist_conversations: true,
                max_conversation_length: 100,
                response_timeout_ms: 30000,
            },
            event_bridge: EventBridgeConfig::default(),
            agents: AgentConfig {
                enable_default_agents: true,
                custom_agents: Vec::new(),
                global_settings: GlobalAgentSettings {
                    max_agents_per_event: 3,
                    global_rate_limit: 120, // 2 per second max
                    enable_response_caching: true,
                    cache_ttl_seconds: 300, // 5 minutes
                },
            },
            performance: PerformanceConfig {
                max_concurrent_requests: 5,
                event_buffer_size: 1000,
                response_buffer_size: 500,
                enable_monitoring: true,
                stats_interval_seconds: 60,
            },
            ui_integration: UiIntegrationConfig {
                show_inline_responses: true,
                response_format: ResponseFormat::Inline,
                enable_notifications: false,
                show_timestamps: false,
                max_display_length: 500,
            },
        }
    }
}

/// Main AI terminal integration manager
pub struct AiTerminalIntegrationManager {
    /// Configuration
    config: Arc<RwLock<AiTerminalConfig>>,
    
    /// AI runtime instance
    ai_runtime: Arc<RwLock<AiRuntime>>,
    
    /// AI event integrator
    ai_event_integrator: Arc<Mutex<AiEventIntegrator>>,
    
    /// Terminal event bridge
    terminal_bridge: Arc<Mutex<TerminalEventBridge>>,
    
    /// Terminal integration implementation
    terminal_integration: Arc<dyn TerminalIntegration + Send + Sync>,
    
    /// Response processing channels
    response_sender: mpsc::UnboundedSender<String>,
    response_receiver: Arc<Mutex<mpsc::UnboundedReceiver<String>>>,
    
    /// Status and statistics
    stats: Arc<RwLock<IntegrationStats>>,
    
    /// Background task handles
    task_handles: Vec<tokio::task::JoinHandle<()>>,
    
    /// System state
    is_running: Arc<RwLock<bool>>,
}

/// Integration system statistics
#[derive(Debug, Clone, Default)]
pub struct IntegrationStats {
    pub start_time: Option<Instant>,
    pub total_events_processed: u64,
    pub total_ai_responses: u64,
    pub total_errors: u64,
    pub active_agents: usize,
    pub current_directory: PathBuf,
    pub session_duration: Duration,
    pub last_activity: Option<Instant>,
    pub performance_metrics: PerformanceMetrics,
}

#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    pub average_response_time_ms: f64,
    pub events_per_minute: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
}

impl AiTerminalIntegrationManager {
    /// Create a new AI terminal integration manager
    pub async fn new(
        config: AiTerminalConfig,
        initial_directory: PathBuf,
        shell_type: ShellType,
    ) -> Result<Self> {
        info!("Initializing AI terminal integration manager");

        // Create AI runtime
        let ai_runtime = Arc::new(RwLock::new(
            AiRuntime::new()
        ));

        // Create AI event integrator
        let ai_event_integrator = Arc::new(Mutex::new(
            AiEventIntegrator::new(Arc::clone(&ai_runtime))
        ));

        // Create terminal event bridge
        let terminal_bridge = Arc::new(Mutex::new(
            TerminalEventBridge::new(
                config.event_bridge.clone(),
                Arc::clone(&ai_event_integrator),
                initial_directory.clone(),
                shell_type,
            ).context("Failed to create terminal event bridge")?
        ));

        // Create terminal integration
        let terminal_integration = Arc::new(
            DefaultTerminalIntegration::new(Arc::clone(&terminal_bridge))
        );

        // Create response channels
        let (response_sender, response_receiver) = mpsc::unbounded_channel();

        // Set up response forwarding
        {
            let mut bridge = terminal_bridge.lock().await;
            bridge.set_response_forwarder(response_sender.clone());
        }

        let mut manager = Self {
            config: Arc::new(RwLock::new(config)),
            ai_runtime,
            ai_event_integrator,
            terminal_bridge,
            terminal_integration,
            response_sender,
            response_receiver: Arc::new(Mutex::new(response_receiver)),
            stats: Arc::new(RwLock::new(IntegrationStats {
                current_directory: initial_directory,
                ..Default::default()
            })),
            task_handles: Vec::new(),
            is_running: Arc::new(RwLock::new(false)),
        };

        // Initialize system
        manager.initialize().await?;

        info!("AI terminal integration manager initialized successfully");
        Ok(manager)
    }

    /// Initialize the integration system
    async fn initialize(&mut self) -> Result<()> {
        // Configure AI providers
        self.configure_ai_providers().await?;
        
        // Register AI agents
        self.register_ai_agents().await?;
        
        // Start AI event processing
        {
            let mut integrator = self.ai_event_integrator.lock().await;
            integrator.start_processing().await?;
        }

        Ok(())
    }

    /// Start the AI terminal integration system
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting AI terminal integration system");

        {
            let mut is_running = self.is_running.write().await;
            if *is_running {
                return Err(anyhow::anyhow!("Integration system is already running"));
            }
            *is_running = true;
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.start_time = Some(Instant::now());
        }

        // Start terminal event monitoring
        {
            let mut bridge = self.terminal_bridge.lock().await;
            bridge.start_monitoring().await?;
        }

        // Start background tasks
        self.start_background_tasks().await?;

        info!("AI terminal integration system started successfully");
        Ok(())
    }

    /// Stop the integration system
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping AI terminal integration system");

        {
            let mut is_running = self.is_running.write().await;
            if !*is_running {
                return Ok(()); // Already stopped
            }
            *is_running = false;
        }

        // Stop terminal monitoring
        {
            let mut bridge = self.terminal_bridge.lock().await;
            bridge.stop_monitoring().await;
        }

        // Stop AI event processing
        {
            let mut integrator = self.ai_event_integrator.lock().await;
            integrator.stop_processing().await;
        }

        // Stop background tasks
        for handle in &self.task_handles {
            handle.abort();
        }
        self.task_handles.clear();

        info!("AI terminal integration system stopped");
        Ok(())
    }

    /// Configure AI providers based on configuration
    async fn configure_ai_providers(&self) -> Result<()> {
        let config = self.config.read().await;
        let mut runtime = self.ai_runtime.write().await;

        for (provider_type, provider_config) in &config.ai_runtime.providers {
            if provider_config.enabled {
                runtime.configure_provider(provider_type.clone(), provider_config.clone())?;
                info!("Configured AI provider: {:?}", provider_type);
            }
        }

        Ok(())
    }

    /// Register AI agents based on configuration
    async fn register_ai_agents(&self) -> Result<()> {
        let config = self.config.read().await;
        let integrator = self.ai_event_integrator.lock().await;

        // Register default agents if enabled
        if config.agents.enable_default_agents {
            for agent in DefaultAgents::all() {
                integrator.register_agent(agent).await?;
                info!("Registered default agent: {}", "agent.name");
            }
        }

        // Register custom agents
        for custom_config in &config.agents.custom_agents {
            if custom_config.enabled {
                let agent = self.create_agent_from_config(custom_config)?;
                integrator.register_agent(agent).await?;
                info!("Registered custom agent: {}", custom_config.name);
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            let default_count = if config.agents.enable_default_agents { 4 } else { 0 };
            let custom_count = config.agents.custom_agents.iter().filter(|a| a.enabled).count();
            stats.active_agents = default_count + custom_count;
        }

        Ok(())
    }

    /// Create an AI agent from custom configuration
    fn create_agent_from_config(&self, config: &CustomAgentConfig) -> Result<AiAgent> {
        let mut agent = AiAgent::new(
            config.id.clone(),
            config.name.clone(),
            config.provider.clone(),
        );

        agent.description = config.description.clone();
        agent.model = config.model.clone();
        agent.system_prompt = config.system_prompt.clone();
        agent.enabled = config.enabled;

        // Configure trigger
        agent.trigger.priority = config.priority;
        agent.trigger.debounce_duration = Duration::from_secs(config.debounce_seconds);

        // Parse trigger events (simplified - in production would be more robust)
        agent.trigger.event_types = config.trigger_events.iter().map(|event_name| {
            match event_name.as_str() {
                "CommandFailed" => TerminalEventType::CommandFailed {
                    command: String::new(),
                    error: String::new(),
                    exit_code: 0,
                    working_directory: PathBuf::new(),
                },
                "CommandExecuted" => TerminalEventType::CommandExecuted {
                    command: String::new(),
                    exit_code: 0,
                    output: String::new(),
                    error_output: String::new(),
                    duration_ms: 0,
                    working_directory: PathBuf::new(),
                    shell: ShellType::Bash,
                },
                "DirectoryChanged" => TerminalEventType::DirectoryChanged {
                    old_path: PathBuf::new(),
                    new_path: PathBuf::new(),
                },
                _ => {
                    warn!("Unknown event type in agent config: {}", event_name);
                    return TerminalEventType::AiAssistanceRequested {
                        context: String::new(),
                        assistance_type: AssistanceType::Suggest,
                    };
                }
            }
        }).collect();

        // Parse activation conditions (simplified)
        agent.trigger.activation_conditions = config.activation_conditions.iter().filter_map(|condition| {
            if condition.starts_with("ExitCodeEquals:") {
                if let Some(code_str) = condition.strip_prefix("ExitCodeEquals:") {
                    if let Ok(code) = code_str.parse::<i32>() {
                        return Some(ActivationCondition::ExitCodeEquals(code));
                    }
                }
            } else if condition.starts_with("CommandContains:") {
                if let Some(pattern) = condition.strip_prefix("CommandContains:") {
                    return Some(ActivationCondition::CommandContains(pattern.to_string()));
                }
            } else if condition.starts_with("ErrorContains:") {
                if let Some(pattern) = condition.strip_prefix("ErrorContains:") {
                    return Some(ActivationCondition::ErrorContains(pattern.to_string()));
                }
            }
            None
        }).collect();

        Ok(agent)
    }

    /// Start background tasks for monitoring and maintenance
    async fn start_background_tasks(&mut self) -> Result<()> {
        // Response processing task
        self.start_response_processing_task().await?;
        
        // Statistics collection task
        if self.config.read().await.performance.enable_monitoring {
            self.start_statistics_task().await?;
        }

        // Performance monitoring task
        self.start_performance_monitoring_task().await?;

        Ok(())
    }

    /// Start response processing background task
    async fn start_response_processing_task(&mut self) -> Result<()> {
        let response_receiver = Arc::clone(&self.response_receiver);
        let stats = Arc::clone(&self.stats);
        let config = Arc::clone(&self.config);
        let is_running = Arc::clone(&self.is_running);

        let handle = tokio::spawn(async move {
            let mut receiver = response_receiver.lock().await;
            
            while *is_running.read().await {
                match receiver.try_recv() {
                    Ok(response) => {
                        // Process AI response
                        Self::process_ai_response(&response, &stats, &config).await;
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {
                        // No messages, continue
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        warn!("Response receiver disconnected");
                        break;
                    }
                }
            }
        });

        self.task_handles.push(handle);
        Ok(())
    }

    /// Process an AI response
    async fn process_ai_response(
        response: &str,
        stats: &Arc<RwLock<IntegrationStats>>,
        config: &Arc<RwLock<AiTerminalConfig>>,
    ) {
        // Update statistics
        {
            let mut stats_lock = stats.write().await;
            stats_lock.total_ai_responses += 1;
            stats_lock.last_activity = Some(Instant::now());
        }

        // Handle UI integration
        let ui_config = {
            let config_lock = config.read().await;
            config_lock.ui_integration.clone()
        };

        if ui_config.show_inline_responses {
            // Format response for display
            let formatted = if ui_config.show_timestamps {
                format!("[{}] 🤖 {}", chrono::Local::now().format("%H:%M:%S"), response)
            } else {
                format!("🤖 {}", response)
            };

            // Truncate if too long
            let display_response = if formatted.len() > ui_config.max_display_length {
                format!("{}...", &formatted[..ui_config.max_display_length])
            } else {
                formatted
            };

            // In a real implementation, this would send to the terminal UI
            info!("AI Response: {}", display_response);
        }
    }

    /// Start statistics collection task
    async fn start_statistics_task(&mut self) -> Result<()> {
        let stats = Arc::clone(&self.stats);
        let bridge = Arc::clone(&self.terminal_bridge);
        let integrator = Arc::clone(&self.ai_event_integrator);
        let is_running = Arc::clone(&self.is_running);
        let interval_seconds = self.config.read().await.performance.stats_interval_seconds;

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_seconds));
            
            while *is_running.read().await {
                interval.tick().await;
                
                // Collect statistics
                let bridge_stats = {
                    let bridge_guard = bridge.lock().await;
                    bridge_guard.get_statistics().await
                };

                let ai_stats = {
                    let integrator_guard = integrator.lock().await;
                    integrator_guard.get_stats().await
                };

                // Update integrated statistics
                {
                    let mut stats_lock = stats.write().await;
                    if let Some(start_time) = stats_lock.start_time {
                        stats_lock.session_duration = start_time.elapsed();
                    }
                    stats_lock.total_events_processed = ai_stats.events_processed;
                    stats_lock.current_directory = bridge_stats.current_directory;

                    // Calculate performance metrics
                    let events_per_minute = if stats_lock.session_duration.as_secs() > 0 {
                        (ai_stats.events_processed as f64 * 60.0) / stats_lock.session_duration.as_secs() as f64
                    } else {
                        0.0
                    };

                    stats_lock.performance_metrics.events_per_minute = events_per_minute;
                    stats_lock.performance_metrics.average_response_time_ms = ai_stats.average_processing_time_ms;
                }

                debug!("Statistics updated: {} events processed, {} responses generated", 
                       ai_stats.events_processed, ai_stats.responses_generated);
            }
        });

        self.task_handles.push(handle);
        Ok(())
    }

    /// Start performance monitoring task
    async fn start_performance_monitoring_task(&mut self) -> Result<()> {
        let stats = Arc::clone(&self.stats);
        let is_running = Arc::clone(&self.is_running);

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            
            while *is_running.read().await {
                interval.tick().await;
                
                // Monitor system resources (simplified)
                let memory_usage = Self::get_memory_usage();
                let cpu_usage = Self::get_cpu_usage();
                
                {
                    let mut stats_lock = stats.write().await;
                    stats_lock.performance_metrics.memory_usage_mb = memory_usage;
                    stats_lock.performance_metrics.cpu_usage_percent = cpu_usage;
                }
                
                // Log performance warnings
                if memory_usage > 500.0 {
                    warn!("High memory usage detected: {:.2}MB", memory_usage);
                }
                if cpu_usage > 80.0 {
                    warn!("High CPU usage detected: {:.2}%", cpu_usage);
                }
            }
        });

        self.task_handles.push(handle);
        Ok(())
    }

    /// Get current memory usage (simplified)
    fn get_memory_usage() -> f64 {
        // In a real implementation, would use system monitoring crates
        // For now, return a placeholder
        64.0
    }

    /// Get current CPU usage (simplified)
    fn get_cpu_usage() -> f64 {
        // In a real implementation, would use system monitoring crates
        // For now, return a placeholder
        15.0
    }

    /// Get terminal integration interface
    pub fn get_terminal_integration(&self) -> Arc<dyn TerminalIntegration + Send + Sync> {
        Arc::clone(&self.terminal_integration)
    }

    /// Get current statistics
    pub async fn get_statistics(&self) -> IntegrationStats {
        self.stats.read().await.clone()
    }

    /// Get current configuration
    pub async fn get_config(&self) -> AiTerminalConfig {
        self.config.read().await.clone()
    }

    /// Update configuration
    pub async fn update_config(&mut self, new_config: AiTerminalConfig) -> Result<()> {
        info!("Updating AI terminal integration configuration");
        
        let needs_restart = {
            let current_config = self.config.read().await;
            // Check if critical settings changed that require restart
            current_config.ai_runtime.default_provider != new_config.ai_runtime.default_provider ||
            current_config.agents.enable_default_agents != new_config.agents.enable_default_agents
        };

        *self.config.write().await = new_config.clone();

        if needs_restart && *self.is_running.read().await {
            info!("Configuration changes require system restart");
            self.stop().await?;
            self.initialize().await?;
            self.start().await?;
        } else {
            // Update bridge configuration
            let mut bridge = self.terminal_bridge.lock().await;
            bridge.update_config(new_config.event_bridge).await?;
        }

        info!("Configuration updated successfully");
        Ok(())
    }

    /// Request AI assistance explicitly
    pub async fn request_ai_assistance(&self, context: String, assistance_type: AssistanceType) -> Result<()> {
        let integration = &self.terminal_integration;
        integration.on_ai_help_requested(&context, assistance_type)?;
        Ok(())
    }

    /// Handle command execution (for external integration)
    pub async fn handle_command_execution(
        &self,
        command: String,
        exit_code: i32,
        output: String,
        error_output: String,
        duration: Duration,
    ) -> Result<()> {
        let integration = &self.terminal_integration;
        integration.on_command_completed(&command, exit_code, &output, &error_output, duration)?;
        Ok(())
    }

    /// Handle directory change (for external integration)
    pub async fn handle_directory_change(&self, new_directory: PathBuf) -> Result<()> {
        let integration = &self.terminal_integration;
        integration.on_directory_change(&new_directory)?;
        
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.current_directory = new_directory;
        }
        
        Ok(())
    }

    /// Check if system is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Get system health status
    pub async fn get_health_status(&self) -> SystemHealthStatus {
        let is_running = self.is_running().await;
        let stats = self.get_statistics().await;
        let config = self.get_config().await;

        let health_score = if is_running {
            let mut score: f64 = 100.0;
            
            // Reduce score based on performance metrics
            if stats.performance_metrics.memory_usage_mb > 500.0 {
                score -= 20.0;
            }
            if stats.performance_metrics.cpu_usage_percent > 80.0 {
                score -= 20.0;
            }
            if stats.performance_metrics.average_response_time_ms > 5000.0 {
                score -= 15.0;
            }
            
            score.max(0.0)
        } else {
            0.0
        };

        SystemHealthStatus {
            is_running,
            health_score,
            uptime: stats.session_duration,
            active_agents: stats.active_agents,
            total_events_processed: stats.total_events_processed,
            total_responses: stats.total_ai_responses,
            error_count: stats.total_errors,
            performance_metrics: stats.performance_metrics,
            last_activity: stats.last_activity,
        }
    }
}

/// System health status
#[derive(Debug, Clone)]
pub struct SystemHealthStatus {
    pub is_running: bool,
    pub health_score: f64, // 0-100
    pub uptime: Duration,
    pub active_agents: usize,
    pub total_events_processed: u64,
    pub total_responses: u64,
    pub error_count: u64,
    pub performance_metrics: PerformanceMetrics,
    pub last_activity: Option<Instant>,
}

impl Drop for AiTerminalIntegrationManager {
    fn drop(&mut self) {
        // Ensure system is stopped and cleaned up
        for handle in &self.task_handles {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_integration_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = AiTerminalConfig::default();
        
        let result = AiTerminalIntegrationManager::new(
            config,
            temp_dir.path().to_path_buf(),
            ShellType::Bash,
        ).await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_start_stop_cycle() {
        let temp_dir = TempDir::new().unwrap();
        let config = AiTerminalConfig::default();
        
        let mut manager = AiTerminalIntegrationManager::new(
            config,
            temp_dir.path().to_path_buf(),
            ShellType::Bash,
        ).await.unwrap();
        
        assert!(!manager.is_running().await);
        
        manager.start().await.unwrap();
        assert!(manager.is_running().await);
        
        manager.stop().await.unwrap();
        assert!(!manager.is_running().await);
    }
}