// Core AI Agent System
// Provides foundational agent architecture for specialized AI tasks

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::{Result, anyhow};
use uuid::Uuid;

pub mod code_generation;
pub mod communication_hub;
pub mod natural_language;
pub mod project_context;
pub mod quality_validation;
pub mod security_lens;
pub mod workflow_orchestration;

/// Core trait for all AI agents in the system
#[async_trait]
pub trait Agent: Send + Sync {
    /// Unique identifier for this agent type
    fn id(&self) -> &str;
    
    /// Human-readable name for this agent
    fn name(&self) -> &str;
    
    /// Description of what this agent does
    fn description(&self) -> &str;
    
    /// Agent capabilities and specializations
    fn capabilities(&self) -> Vec<AgentCapability>;
    
    /// Process a request and return a response
    async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse>;
    
    /// Check if agent can handle this type of request
    fn can_handle(&self, request_type: &AgentRequestType) -> bool;
    
    /// Get agent's current status
    async fn status(&self) -> AgentStatus;
    
    /// Initialize the agent with configuration
    async fn initialize(&mut self, config: AgentConfig) -> Result<()>;
    
    /// Cleanup resources when agent is shut down
    async fn shutdown(&mut self) -> Result<()>;
}

/// Types of capabilities an agent can have
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentCapability {
    CodeGeneration,
    CodeAnalysis,
    QualityAssurance,
    SecurityAnalysis,
    ProjectManagement,
    WorkflowOrchestration,
    ContextManagement,
    FileSystem,
    GitIntegration,
    LSPIntegration,
    TerminalIntegration,
    Custom(String),
}

/// Request types that agents can handle
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentRequestType {
    GenerateCode,
    AnalyzeCode,
    ValidateQuality,
    CheckSecurity,
    ManageProject,
    ExecuteWorkflow,
    UpdateContext,
    ProcessFile,
    GitOperation,
    LSPQuery,
    TerminalCommand,
    Custom(String),
}

/// Agent request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub id: Uuid,
    pub request_type: AgentRequestType,
    pub payload: serde_json::Value,
    pub context: AgentContext,
    pub metadata: HashMap<String, String>,
}

/// Agent response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub request_id: Uuid,
    pub agent_id: String,
    pub success: bool,
    pub payload: serde_json::Value,
    pub artifacts: Vec<AgentArtifact>,
    pub next_actions: Vec<SuggestedAction>,
    pub metadata: HashMap<String, String>,
}

/// Context information available to agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    pub project_root: Option<String>,
    pub current_directory: String,
    pub current_branch: Option<String>,
    pub open_files: Vec<String>,
    pub recent_commands: Vec<String>,
    pub environment_vars: HashMap<String, String>,
    pub user_preferences: HashMap<String, String>,
}

/// Agent-generated artifacts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentArtifact {
    pub id: Uuid,
    pub artifact_type: ArtifactType,
    pub content: String,
    pub metadata: HashMap<String, String>,
}

/// Types of artifacts agents can produce
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactType {
    Code,
    Documentation,
    Configuration,
    Script,
    Report,
    Suggestion,
    Warning,
    Error,
    Custom(String),
}

/// Actions that agents can suggest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedAction {
    pub action_type: ActionType,
    pub description: String,
    pub command: Option<String>,
    pub priority: ActionPriority,
    pub safe_to_auto_execute: bool,
}

/// Types of actions that can be suggested
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    RunCommand,
    CreateFile,
    EditFile,
    DeleteFile,
    GitCommit,
    GitPush,
    InstallDependency,
    RunTest,
    Deploy,
    Custom(String),
}

/// Priority levels for suggested actions
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ActionPriority {
    Low,
    Medium,
    High,
    Critical,
}

/// Current status of an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub is_healthy: bool,
    pub is_busy: bool,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub current_task: Option<String>,
    pub error_message: Option<String>,
}

/// Configuration for agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub enabled: bool,
    pub max_concurrent_requests: usize,
    pub timeout_seconds: u64,
    pub provider_config: HashMap<String, serde_json::Value>,
    pub custom_settings: HashMap<String, serde_json::Value>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_concurrent_requests: 5,
            timeout_seconds: 30,
            provider_config: HashMap::new(),
            custom_settings: HashMap::new(),
        }
    }
}

/// Manager for all agents in the system
pub struct AgentManager {
    agents: Arc<RwLock<HashMap<String, Box<dyn Agent>>>>,
    config: AgentConfig,
}

impl AgentManager {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Register a new agent
    pub async fn register_agent(&self, mut agent: Box<dyn Agent>) -> Result<()> {
        let agent_id = agent.id().to_string();
        
        // Initialize the agent
        agent.initialize(self.config.clone()).await?;
        
        // Add to registry
        let mut agents = self.agents.write().await;
        agents.insert(agent_id.clone(), agent);
        
        tracing::info!("Registered agent: {}", agent_id);
        Ok(())
    }

    /// Unregister an agent
    pub async fn unregister_agent(&self, agent_id: &str) -> Result<()> {
        let mut agents = self.agents.write().await;
        
        if let Some(mut agent) = agents.remove(agent_id) {
            agent.shutdown().await?;
            tracing::info!("Unregistered agent: {}", agent_id);
            Ok(())
        } else {
            Err(anyhow!("Agent not found: {}", agent_id))
        }
    }

    /// Route a request to the appropriate agent
    pub async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        let agents = self.agents.read().await;
        
        // Find an agent that can handle this request type
        for agent in agents.values() {
            if agent.can_handle(&request.request_type) {
                return agent.handle_request(request).await;
            }
        }
        
        Err(anyhow!("No agent available to handle request type: {:?}", request.request_type))
    }

    /// Get all available agents
    pub async fn list_agents(&self) -> Vec<String> {
        let agents = self.agents.read().await;
        agents.keys().cloned().collect()
    }

    /// Get agent status by ID
    pub async fn get_agent_status(&self, agent_id: &str) -> Result<AgentStatus> {
        let agents = self.agents.read().await;
        
        if let Some(agent) = agents.get(agent_id) {
            Ok(agent.status().await)
        } else {
            Err(anyhow!("Agent not found: {}", agent_id))
        }
    }

    /// Get agents by capability
    pub async fn find_agents_by_capability(&self, capability: AgentCapability) -> Vec<String> {
        let agents = self.agents.read().await;
        let mut result = Vec::new();
        
        for (id, agent) in agents.iter() {
            if agent.capabilities().contains(&capability) {
                result.push(id.clone());
            }
        }
        
        result
    }

    /// Shutdown all agents
    pub async fn shutdown_all(&self) -> Result<()> {
        let mut agents = self.agents.write().await;
        
        for (id, agent) in agents.iter_mut() {
            if let Err(e) = agent.shutdown().await {
                tracing::error!("Failed to shutdown agent {}: {}", id, e);
            }
        }
        
        agents.clear();
        tracing::info!("All agents shut down");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAgent {
        id: String,
        name: String,
    }

    #[async_trait]
    impl Agent for MockAgent {
        fn id(&self) -> &str { &self.id }
        fn name(&self) -> &str { &self.name }
        fn description(&self) -> &str { "Mock agent for testing" }
        
        fn capabilities(&self) -> Vec<AgentCapability> {
            vec![AgentCapability::CodeGeneration]
        }
        
        async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse> {
            Ok(AgentResponse {
                request_id: request.id,
                agent_id: self.id.clone(),
                success: true,
                payload: serde_json::json!({"message": "mock response"}),
                artifacts: vec![],
                next_actions: vec![],
                metadata: HashMap::new(),
            })
        }
        
        fn can_handle(&self, request_type: &AgentRequestType) -> bool {
            matches!(request_type, AgentRequestType::GenerateCode)
        }
        
        async fn status(&self) -> AgentStatus {
            AgentStatus {
                is_healthy: true,
                is_busy: false,
                last_activity: chrono::Utc::now(),
                current_task: None,
                error_message: None,
            }
        }
        
        async fn initialize(&mut self, _config: AgentConfig) -> Result<()> { Ok(()) }
        async fn shutdown(&mut self) -> Result<()> { Ok(()) }
    }

    #[tokio::test]
    async fn test_agent_manager() {
        let manager = AgentManager::new(AgentConfig::default());
        
        let agent = MockAgent {
            id: "test-agent".to_string(),
            name: "Test Agent".to_string(),
        };
        
        manager.register_agent(Box::new(agent)).await.unwrap();
        
        let agents = manager.list_agents().await;
        assert!(agents.contains(&"test-agent".to_string()));
        
        let request = AgentRequest {
            id: Uuid::new_v4(),
            request_type: AgentRequestType::GenerateCode,
            payload: serde_json::json!({}),
            context: AgentContext {
                project_root: None,
                current_directory: "/tmp".to_string(),
                current_branch: None,
                open_files: vec![],
                recent_commands: vec![],
                environment_vars: HashMap::new(),
                user_preferences: HashMap::new(),
            },
            metadata: HashMap::new(),
        };
        
        let response = manager.handle_request(request).await.unwrap();
        assert!(response.success);
        assert_eq!(response.agent_id, "test-agent");
    }
}