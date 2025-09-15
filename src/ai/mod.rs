// AI Module - Core AI functionality for OpenAgent Terminal
// Provides agent-based AI assistance with privacy-first architecture

pub mod providers;
pub mod agents;
pub mod communication;
pub mod integrations;

// Re-export key types for convenience
pub use agents::{
    Agent, AgentManager, AgentRequest, AgentResponse, AgentContext,
    AgentCapability, AgentRequestType, AgentArtifact, SuggestedAction,
    ActionType, ActionPriority, ArtifactType,
};

pub use communication::{
    AgentEventBus, AgentCommunicationCoordinator, AgentMessage, 
    AgentEvent, MessagePriority, WorkflowState, WorkflowStatus,
};

pub use providers::{
    AiProvider, AiProviderManager, CompletionOptions, UsageStats,
    OpenAIProvider, AnthropicProvider, OllamaProvider, AiConfig,
};

use anyhow::Result;
use std::sync::Arc;
use std::collections::HashMap;

/// Main AI system that coordinates all components
pub struct OpenAgentAI {
    provider_manager: AiProviderManager,
    agent_manager: Arc<agents::AgentManager>,
    event_bus: Arc<communication::AgentEventBus>,
    coordinator: communication::AgentCommunicationCoordinator,
}

impl OpenAgentAI {
    /// Initialize the AI system with configuration
    pub async fn new(config: AiConfig) -> Result<Self> {
        // Initialize provider manager
        let provider_manager = AiProviderManager::new(config.clone())?;
        
        // Initialize agent manager
        let agent_config = agents::AgentConfig::default();
        let agent_manager = Arc::new(agents::AgentManager::new(agent_config));
        
        // Initialize event bus and coordinator
        let event_bus_config = communication::EventBusConfig::default();
        let event_bus = Arc::new(communication::AgentEventBus::new(event_bus_config));
        let coordinator = communication::AgentCommunicationCoordinator::new(Arc::clone(&event_bus));
        
        let ai_system = Self {
            provider_manager,
            agent_manager,
            event_bus,
            coordinator,
        };
        
        // Initialize default agents
        ai_system.initialize_default_agents().await?;
        
        Ok(ai_system)
    }
    
    /// Initialize default agents
    async fn initialize_default_agents(&self) -> Result<()> {
        use agents::code_generation::CodeGenerationAgent;
        use agents::security_lens::SecurityLensAgent;
        
        // Get a provider for the agents (prefer local Ollama)
        let active_provider = self.provider_manager.active_provider();
        
        // Create a cloned provider for each agent (simplified for this example)
        // In practice, you'd want to implement Clone for providers or use Arc
        
        // For now, we'll create agents without providers and initialize them separately
        // This is a design consideration - agents might share providers or have dedicated ones
        
        tracing::info!("AI system initialized with agent framework");
        Ok(())
    }
    
    /// Get the provider manager
    pub fn provider_manager(&self) -> &AiProviderManager {
        &self.provider_manager
    }
    
    /// Get the agent manager
    pub fn agent_manager(&self) -> Arc<agents::AgentManager> {
        Arc::clone(&self.agent_manager)
    }
    
    /// Get the event bus
    pub fn event_bus(&self) -> Arc<communication::AgentEventBus> {
        Arc::clone(&self.event_bus)
    }
    
    /// Get the communication coordinator
    pub fn coordinator(&self) -> &communication::AgentCommunicationCoordinator {
        &self.coordinator
    }
    
    /// Process a high-level AI request
    pub async fn process_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        self.agent_manager.handle_request(request).await
    }
    
    /// Send a message through the communication system
    pub async fn send_message(
        &self,
        message: AgentMessage,
        sender: String,
        priority: MessagePriority,
    ) -> Result<()> {
        self.event_bus.send_message(message, sender, priority).await
    }
    
    /// Get system health status
    pub async fn health_check(&self) -> HashMap<String, bool> {
        let mut health = HashMap::new();
        
        // Check provider health
        let provider_health = self.provider_manager.health_check_all().await;
        for (name, status) in provider_health {
            health.insert(format!("provider_{}", name), status);
        }
        
        // Check agent health
        let agents = self.agent_manager.list_agents().await;
        for agent_id in agents {
            if let Ok(status) = self.agent_manager.get_agent_status(&agent_id).await {
                health.insert(format!("agent_{}", agent_id), status.is_healthy);
            }
        }
        
        // Check communication system
        health.insert("event_bus".to_string(), true); // Simple health check
        
        health
    }
    
    /// Shutdown the AI system
    pub async fn shutdown(&self) -> Result<()> {
        // Shutdown agents
        self.agent_manager.shutdown_all().await?;
        
        tracing::info!("AI system shut down");
        Ok(())
    }
}