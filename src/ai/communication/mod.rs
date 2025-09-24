// Agent Communication Framework
// Message passing, event bus, and coordination system for multi-agent workflows

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock, Mutex};
use anyhow::{Result, anyhow};
use uuid::Uuid;
use tracing::{debug, info, warn, error};

use crate::ai::agents::{Agent, AgentRequest, AgentResponse, AgentContext};

pub mod event_bus;
pub mod message_router;
pub mod workflow_coordinator;

/// Core message types for agent communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMessage {
    /// Direct request to a specific agent
    DirectRequest {
        target_agent: String,
        request: AgentRequest,
        reply_to: Option<String>,
    },
    /// Broadcast request that any capable agent can handle
    BroadcastRequest {
        request: AgentRequest,
        exclude_agents: Vec<String>,
    },
    /// Response to a previous request
    Response {
        response: AgentResponse,
        original_sender: String,
    },
    /// Event notification
    Event {
        event: AgentEvent,
        source_agent: String,
    },
    /// Coordination message for workflow management
    Coordination {
        workflow_id: Uuid,
        coordination_type: CoordinationType,
        payload: serde_json::Value,
    },
}

/// Types of coordination messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoordinationType {
    WorkflowStart,
    WorkflowComplete,
    WorkflowFailed,
    StepComplete,
    RequestInput,
    ShareContext,
    SyncState,
}

/// Events that agents can emit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    /// Agent started working on a task
    TaskStarted {
        task_id: Uuid,
        task_description: String,
    },
    /// Agent completed a task
    TaskCompleted {
        task_id: Uuid,
        success: bool,
        artifacts: Vec<String>, // Artifact IDs
    },
    /// Agent needs assistance from another agent
    AssistanceRequested {
        task_id: Uuid,
        required_capability: String,
        context: serde_json::Value,
    },
    /// Agent updated its context
    ContextUpdated {
        context_type: String,
        updates: HashMap<String, serde_json::Value>,
    },
    /// Security risk detected
    SecurityRiskDetected {
        risk_level: String,
        description: String,
        affected_context: Option<String>,
    },
    /// Agent status changed
    StatusChanged {
        new_status: String,
        details: Option<String>,
    },
}

/// Message envelope with routing and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub id: Uuid,
    pub message: AgentMessage,
    pub sender: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub priority: MessagePriority,
    pub ttl: Option<chrono::DateTime<chrono::Utc>>, // Time to live
    pub trace_id: Option<Uuid>, // For distributed tracing
}

/// Message priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MessagePriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Event bus for agent communication
pub struct AgentEventBus {
    // Broadcast channel for all agents to receive events
    event_sender: broadcast::Sender<MessageEnvelope>,
    // Agent-specific channels for direct messaging
    agent_channels: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<MessageEnvelope>>>>,
    // Message history for debugging and tracing
    message_history: Arc<Mutex<Vec<MessageEnvelope>>>,
    // Configuration
    config: EventBusConfig,
}

/// Configuration for the event bus
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    pub max_broadcast_subscribers: usize,
    pub message_history_limit: usize,
    pub default_message_ttl: chrono::Duration,
    pub enable_tracing: bool,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            max_broadcast_subscribers: 100,
            message_history_limit: 1000,
            default_message_ttl: chrono::Duration::minutes(5),
            enable_tracing: true,
        }
    }
}

impl AgentEventBus {
    pub fn new(config: EventBusConfig) -> Self {
        let (event_sender, _) = broadcast::channel(config.max_broadcast_subscribers);
        
        Self {
            event_sender,
            agent_channels: Arc::new(RwLock::new(HashMap::new())),
            message_history: Arc::new(Mutex::new(Vec::new())),
            config,
        }
    }

    /// Register an agent for direct messaging
    pub async fn register_agent(&self, agent_id: String) -> mpsc::UnboundedReceiver<MessageEnvelope> {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        let mut channels = self.agent_channels.write().await;
        channels.insert(agent_id.clone(), sender);
        
        info!("Registered agent {} for direct messaging", agent_id);
        receiver
    }

    /// Unregister an agent
    pub async fn unregister_agent(&self, agent_id: &str) {
        let mut channels = self.agent_channels.write().await;
        channels.remove(agent_id);
        
        info!("Unregistered agent {}", agent_id);
    }

    /// Send a message through the event bus
    pub async fn send_message(&self, message: AgentMessage, sender: String, priority: MessagePriority) -> Result<()> {
        let envelope = MessageEnvelope {
            id: Uuid::new_v4(),
            message: message.clone(),
            sender: sender.clone(),
            timestamp: chrono::Utc::now(),
            priority,
            ttl: Some(chrono::Utc::now() + self.config.default_message_ttl),
            trace_id: None, // TODO: Extract from context
        };

        // Store in message history
        {
            let mut history = self.message_history.lock().await;
            history.push(envelope.clone());
            
            // Trim history if needed
            if history.len() > self.config.message_history_limit {
                history.remove(0);
            }
        }

        match &message {
            AgentMessage::DirectRequest { target_agent, .. } => {
                self.send_direct_message(target_agent, envelope).await?;
            }
            AgentMessage::BroadcastRequest { .. } | 
            AgentMessage::Event { .. } | 
            AgentMessage::Coordination { .. } => {
                self.broadcast_message(envelope).await?;
            }
            AgentMessage::Response { original_sender, .. } => {
                self.send_direct_message(original_sender, envelope).await?;
            }
        }

        Ok(())
    }

    /// Send a direct message to a specific agent
    async fn send_direct_message(&self, target_agent: &str, envelope: MessageEnvelope) -> Result<()> {
        let channels = self.agent_channels.read().await;
        
        if let Some(sender) = channels.get(target_agent) {
            sender.send(envelope.clone())
                .map_err(|_| anyhow!("Failed to send message to agent {}", target_agent))?;
            
            debug!("Sent direct message to agent {}", target_agent);
        } else {
            warn!("Agent {} not found for direct messaging", target_agent);
            return Err(anyhow!("Agent {} not registered", target_agent));
        }

        Ok(())
    }

    /// Broadcast a message to all subscribers
    async fn broadcast_message(&self, envelope: MessageEnvelope) -> Result<()> {
        match self.event_sender.send(envelope.clone()) {
            Ok(subscriber_count) => {
                debug!("Broadcasted message to {} subscribers", subscriber_count);
            }
            Err(_) => {
                // No subscribers, which is okay
                debug!("Broadcasted message but no subscribers");
            }
        }

        Ok(())
    }

    /// Subscribe to broadcast events
    pub fn subscribe_to_events(&self) -> broadcast::Receiver<MessageEnvelope> {
        self.event_sender.subscribe()
    }

    /// Get message history for debugging
    pub async fn get_message_history(&self, limit: Option<usize>) -> Vec<MessageEnvelope> {
        let history = self.message_history.lock().await;
        let limit = limit.unwrap_or(history.len());
        
        history.iter()
            .rev()
            .take(limit)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// Clean up expired messages
    pub async fn cleanup_expired_messages(&self) {
        let now = chrono::Utc::now();
        let mut history = self.message_history.lock().await;
        
        let original_len = history.len();
        history.retain(|envelope| {
            envelope.ttl.map_or(true, |ttl| ttl > now)
        });
        
        let removed = original_len - history.len();
        if removed > 0 {
            debug!("Cleaned up {} expired messages", removed);
        }
    }
}

/// Agent communication coordinator
pub struct AgentCommunicationCoordinator {
    event_bus: Arc<AgentEventBus>,
    agents: Arc<RwLock<HashMap<String, Box<dyn Agent>>>>,
    active_workflows: Arc<RwLock<HashMap<Uuid, WorkflowState>>>,
}

/// State tracking for active workflows
#[derive(Debug, Clone)]
pub struct WorkflowState {
    pub id: Uuid,
    pub participants: Vec<String>,
    pub status: WorkflowStatus,
    pub context: HashMap<String, serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowStatus {
    Starting,
    Running,
    WaitingForInput,
    Completed,
    Failed,
    Cancelled,
}

impl AgentCommunicationCoordinator {
    pub fn new(event_bus: Arc<AgentEventBus>) -> Self {
        Self {
            event_bus,
            agents: Arc::new(RwLock::new(HashMap::new())),
            active_workflows: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an agent with the coordinator
    pub async fn register_agent(&self, agent_id: String, agent: Box<dyn Agent>) -> Result<()> {
        // Register with event bus
        let message_receiver = self.event_bus.register_agent(agent_id.clone()).await;
        
        // Store agent reference
        {
            let mut agents = self.agents.write().await;
            agents.insert(agent_id.clone(), agent);
        }

        // Start message processing loop for this agent
        let coordinator = self.clone();
        tokio::spawn(async move {
            coordinator.process_agent_messages(agent_id, message_receiver).await;
        });

        Ok(())
    }

    /// Process messages for a specific agent
    async fn process_agent_messages(&self, agent_id: String, mut receiver: mpsc::UnboundedReceiver<MessageEnvelope>) {
        while let Some(envelope) = receiver.recv().await {
            if let Err(e) = self.handle_agent_message(&agent_id, envelope).await {
                error!("Error processing message for agent {}: {}", agent_id, e);
            }
        }
        
        info!("Message processing stopped for agent {}", agent_id);
    }

    /// Handle a message for a specific agent
    async fn handle_agent_message(&self, agent_id: &str, envelope: MessageEnvelope) -> Result<()> {
        // Check if message is expired
        if let Some(ttl) = envelope.ttl {
            if chrono::Utc::now() > ttl {
                debug!("Dropping expired message {}", envelope.id);
                return Ok(());
            }
        }

        match &envelope.message {
            AgentMessage::DirectRequest { request, reply_to, .. } => {
                self.handle_direct_request(agent_id, request.clone(), reply_to.as_deref()).await?;
            }
            AgentMessage::BroadcastRequest { request, exclude_agents } => {
                if !exclude_agents.contains(&agent_id.to_string()) {
                    // Check if this agent can handle the request
                    let agents = self.agents.read().await;
                    if let Some(agent) = agents.get(agent_id) {
                        if agent.can_handle(&request.request_type) {
                            self.handle_broadcast_request(agent_id, request.clone()).await?;
                        }
                    }
                }
            }
            AgentMessage::Event { event, source_agent } => {
                self.handle_event(agent_id, event.clone(), source_agent).await?;
            }
            AgentMessage::Coordination { workflow_id, coordination_type, payload } => {
                self.handle_coordination_message(agent_id, *workflow_id, coordination_type.clone(), payload.clone()).await?;
            }
            AgentMessage::Response { .. } => {
                // Responses are handled by the original requester
                debug!("Agent {} received response message", agent_id);
            }
        }

        Ok(())
    }

    /// Handle direct request to an agent
    async fn handle_direct_request(&self, agent_id: &str, request: AgentRequest, reply_to: Option<&str>) -> Result<()> {
        let agents = self.agents.read().await;
        
        if let Some(agent) = agents.get(agent_id) {
            match agent.handle_request(request.clone()).await {
                Ok(response) => {
                    if let Some(reply_agent) = reply_to {
                        // Send response back to requesting agent
                        self.event_bus.send_message(
                            AgentMessage::Response {
                                response,
                                original_sender: reply_agent.to_string(),
                            },
                            agent_id.to_string(),
                            MessagePriority::Normal,
                        ).await?;
                    }
                }
                Err(e) => {
                    error!("Agent {} failed to handle request: {}", agent_id, e);
                    if let Some(reply_agent) = reply_to {
                        // Send error response back to requesting agent
                        let error_response = AgentResponse {
                            request_id: request.id,
                            agent_id: agent_id.to_string(),
                            success: false,
                            data: None,
                            error: Some(format!("Request processing failed: {}", e)),
                            artifacts: Vec::new(),
                            suggested_actions: Vec::new(),
                            metadata: std::collections::HashMap::new(),
                        };
                        
                        if let Err(send_err) = self.event_bus.send_message(
                            AgentMessage::Response {
                                response: error_response,
                                original_sender: reply_agent.to_string(),
                            },
                            agent_id.to_string(),
                            MessagePriority::Normal,
                        ).await {
                            error!("Failed to send error response: {}", send_err);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle broadcast request
    async fn handle_broadcast_request(&self, agent_id: &str, request: AgentRequest) -> Result<()> {
        // This is a simplified version - in practice, you'd want more sophisticated
        // coordination to avoid multiple agents responding to the same broadcast
        self.handle_direct_request(agent_id, request, None).await
    }

    /// Handle events from other agents
    async fn handle_event(&self, _agent_id: &str, event: AgentEvent, source_agent: &str) -> Result<()> {
        match event {
            AgentEvent::AssistanceRequested { task_id, required_capability, context } => {
                info!("Agent {} requested assistance for capability {}", source_agent, required_capability);
                
                // Find agents with the required capability
                let agents = self.agents.read().await;
                let capable_agents: Vec<String> = agents
                    .iter()
                    .filter(|(id, agent)| {
                        *id != source_agent && agent.capabilities().contains(&required_capability)
                    })
                    .map(|(id, _)| id.clone())
                    .collect();
                
                if capable_agents.is_empty() {
                    warn!("No agents found with capability {:?} for task {}", required_capability, task_id);
                    return Ok(());
                }
                
                // Create coordination request for capable agents
                let coordination_request = AgentRequest {
                    id: Uuid::new_v4(),
                    request_type: AgentRequestType::AssistanceRequest,
                    content: format!("Assistance needed for task {} with capability {:?}", task_id, required_capability),
                    context: context.clone(),
                    priority: crate::ai::agents::RequestPriority::High,
                    metadata: {
                        let mut meta = std::collections::HashMap::new();
                        meta.insert("original_task_id".to_string(), task_id.to_string());
                        meta.insert("requesting_agent".to_string(), source_agent.to_string());
                        meta.insert("required_capability".to_string(), format!("{:?}", required_capability));
                        meta
                    },
                };
                
                // Send assistance request to the most suitable agent (first in list for now)
                // In a more sophisticated system, this could use load balancing or agent scoring
                if let Some(target_agent) = capable_agents.first() {
                    if let Err(e) = self.event_bus.send_message(
                        AgentMessage::DirectRequest {
                            request: coordination_request,
                            reply_to: Some(source_agent.to_string()),
                        },
                        target_agent.clone(),
                        MessagePriority::High,
                    ).await {
                        error!("Failed to send assistance request to {}: {}", target_agent, e);
                    } else {
                        info!("Coordinated assistance request from {} to {} for task {}", source_agent, target_agent, task_id);
                    }
                }
            }
            AgentEvent::SecurityRiskDetected { risk_level, description, .. } => {
                warn!("Security risk detected by {}: {} - {}", source_agent, risk_level, description);
                
                // Find security-aware agents (those with Security capability)
                let agents = self.agents.read().await;
                let security_agents: Vec<String> = agents
                    .iter()
                    .filter(|(id, agent)| {
                        *id != source_agent && agent.capabilities().contains(&AgentCapability::Security)
                    })
                    .map(|(id, _)| id.clone())
                    .collect();
                
                // Create security alert request
                let security_alert = AgentRequest {
                    id: Uuid::new_v4(),
                    request_type: AgentRequestType::SecurityAlert,
                    content: format!("Security risk detected: {} (Level: {:?})", description, risk_level),
                    context: std::collections::HashMap::new(),
                    priority: match risk_level {
                        crate::ai::agents::SecurityRiskLevel::Critical => crate::ai::agents::RequestPriority::Critical,
                        crate::ai::agents::SecurityRiskLevel::High => crate::ai::agents::RequestPriority::High,
                        crate::ai::agents::SecurityRiskLevel::Medium => crate::ai::agents::RequestPriority::Normal,
                        crate::ai::agents::SecurityRiskLevel::Low => crate::ai::agents::RequestPriority::Low,
                    },
                    metadata: {
                        let mut meta = std::collections::HashMap::new();
                        meta.insert("reporting_agent".to_string(), source_agent.to_string());
                        meta.insert("risk_level".to_string(), format!("{:?}", risk_level));
                        meta.insert("detection_time".to_string(), chrono::Utc::now().to_rfc3339());
                        meta
                    },
                };
                
                // Notify all security agents
                for security_agent in security_agents {
                    if let Err(e) = self.event_bus.send_message(
                        AgentMessage::DirectRequest {
                            request: security_alert.clone(),
                            reply_to: None, // Security alerts don't require responses
                        },
                        security_agent.clone(),
                        MessagePriority::High,
                    ).await {
                        error!("Failed to notify security agent {}: {}", security_agent, e);
                    }
                }
                
                // For critical risks, also broadcast to all agents
                if matches!(risk_level, crate::ai::agents::SecurityRiskLevel::Critical) {
                    if let Err(e) = self.event_bus.send_message(
                        AgentMessage::BroadcastRequest {
                            request: security_alert,
                            exclude_agents: vec![source_agent.to_string()],
                        },
                        "security-coordinator".to_string(),
                        MessagePriority::Critical,
                    ).await {
                        error!("Failed to broadcast critical security alert: {}", e);
                    }
                }
            }
            _ => {
                debug!("Received event from {}: {:?}", source_agent, event);
            }
        }

        Ok(())
    }

    /// Handle workflow coordination messages
    async fn handle_coordination_message(
        &self,
        agent_id: &str,
        workflow_id: Uuid,
        coordination_type: CoordinationType,
        payload: serde_json::Value,
    ) -> Result<()> {
        match coordination_type {
            CoordinationType::WorkflowStart => {
                info!("Starting workflow {} with agent {}", workflow_id, agent_id);
                let workflow = WorkflowState {
                    id: workflow_id,
                    participants: vec![agent_id.to_string()],
                    status: WorkflowStatus::Starting,
                    context: HashMap::new(),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };
                
                let mut workflows = self.active_workflows.write().await;
                workflows.insert(workflow_id, workflow);
            }
            CoordinationType::WorkflowComplete => {
                info!("Workflow {} completed by agent {}", workflow_id, agent_id);
                let mut workflows = self.active_workflows.write().await;
                if let Some(workflow) = workflows.get_mut(&workflow_id) {
                    workflow.status = WorkflowStatus::Completed;
                    workflow.updated_at = chrono::Utc::now();
                }
            }
            CoordinationType::StepComplete => {
                debug!("Workflow {} step completed by agent {}", workflow_id, agent_id);
                
                let mut workflows = self.active_workflows.write().await;
                if let Some(workflow) = workflows.get_mut(&workflow_id) {
                    workflow.updated_at = chrono::Utc::now();
                    
                    // Extract step information from payload
                    if let Ok(step_info) = serde_json::from_value::<serde_json::Map<String, serde_json::Value>>(payload) {
                        if let Some(step_id) = step_info.get("step_id").and_then(|v| v.as_str()) {
                            // Update workflow context with step completion
                            workflow.context.insert(
                                format!("step_{}_completed", step_id),
                                serde_json::Value::Bool(true),
                            );
                            workflow.context.insert(
                                format!("step_{}_completed_by", step_id),
                                serde_json::Value::String(agent_id.to_string()),
                            );
                            workflow.context.insert(
                                format!("step_{}_completed_at", step_id),
                                serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
                            );
                            
                            // If step has results, store them
                            if let Some(results) = step_info.get("results") {
                                workflow.context.insert(
                                    format!("step_{}_results", step_id),
                                    results.clone(),
                                );
                            }
                            
                            info!("Workflow {} step {} completed by agent {}", workflow_id, step_id, agent_id);
                            
                            // Check if this was the final step
                            if let Some(is_final) = step_info.get("is_final_step").and_then(|v| v.as_bool()) {
                                if is_final {
                                    workflow.status = WorkflowStatus::Completed;
                                    info!("Workflow {} marked as completed after final step", workflow_id);
                                }
                            }
                        }
                    }
                } else {
                    warn!("Received step completion for unknown workflow {}", workflow_id);
                }
            }
            _ => {
                debug!("Coordination message for workflow {}: {:?}", workflow_id, coordination_type);
            }
        }

        Ok(())
    }

    /// Get status of active workflows
    pub async fn get_workflow_status(&self, workflow_id: Uuid) -> Option<WorkflowState> {
        let workflows = self.active_workflows.read().await;
        workflows.get(&workflow_id).cloned()
    }

    /// List all active workflows
    pub async fn list_active_workflows(&self) -> Vec<WorkflowState> {
        let workflows = self.active_workflows.read().await;
        workflows.values().cloned().collect()
    }
}

impl Clone for AgentCommunicationCoordinator {
    fn clone(&self) -> Self {
        Self {
            event_bus: Arc::clone(&self.event_bus),
            agents: Arc::clone(&self.agents),
            active_workflows: Arc::clone(&self.active_workflows),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agents::{Agent, AgentCapability, AgentStatus, AgentConfig};
    use crate::ai::agents::{AgentRequestType, AgentArtifact, SuggestedAction};

    struct MockAgent {
        id: String,
        name: String,
    }

    #[async_trait]
    impl Agent for MockAgent {
        fn id(&self) -> &str { &self.id }
        fn name(&self) -> &str { &self.name }
        fn description(&self) -> &str { "Mock agent for testing" }
        fn capabilities(&self) -> Vec<AgentCapability> { vec![AgentCapability::CodeGeneration] }
        
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
    async fn test_event_bus_creation() {
        let config = EventBusConfig::default();
        let event_bus = AgentEventBus::new(config);
        
        // Test agent registration
        let receiver = event_bus.register_agent("test-agent".to_string()).await;
        
        // Should receive the channel
        assert!(receiver.try_recv().is_err()); // No messages yet
    }

    #[tokio::test]
    async fn test_message_sending() {
        let config = EventBusConfig::default();
        let event_bus = Arc::new(AgentEventBus::new(config));
        
        // Register an agent
        let mut receiver = event_bus.register_agent("test-agent".to_string()).await;
        
        // Send a direct message
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

        let message = AgentMessage::DirectRequest {
            target_agent: "test-agent".to_string(),
            request,
            reply_to: None,
        };

        event_bus.send_message(message, "sender".to_string(), MessagePriority::Normal).await.unwrap();
        
        // Should receive the message
        let envelope = receiver.recv().await.unwrap();
        match envelope.message {
            AgentMessage::DirectRequest { target_agent, .. } => {
                assert_eq!(target_agent, "test-agent");
            }
            _ => panic!("Expected DirectRequest message"),
        }
    }

    #[tokio::test]
    async fn test_coordinator() {
        let config = EventBusConfig::default();
        let event_bus = Arc::new(AgentEventBus::new(config));
        let coordinator = AgentCommunicationCoordinator::new(event_bus);
        
        let agent = MockAgent {
            id: "test-agent".to_string(),
            name: "Test Agent".to_string(),
        };
        
        coordinator.register_agent("test-agent".to_string(), Box::new(agent)).await.unwrap();
        
        // Test workflow creation
        let workflow_id = Uuid::new_v4();
        coordinator.handle_coordination_message(
            "test-agent",
            workflow_id,
            CoordinationType::WorkflowStart,
            serde_json::json!({}),
        ).await.unwrap();
        
        let workflow = coordinator.get_workflow_status(workflow_id).await.unwrap();
        assert_eq!(workflow.id, workflow_id);
        assert!(matches!(workflow.status, WorkflowStatus::Starting));
    }
}