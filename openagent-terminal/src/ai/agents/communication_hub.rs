use async_trait::async_trait;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, broadcast};
use uuid::Uuid;
use tracing::{info, warn, error};

use super::{
    Agent, AgentCapability, AgentConfig, AgentContext, AgentRequest, AgentResponse, 
    AgentStatus, AgentRequestType, AgentArtifact, ArtifactType, SuggestedAction, 
    ActionType, ActionPriority
};

/// Communication hub for routing messages between agents and coordinating workflows
pub struct AgentCommunicationHub {
    id: String,
    config: AgentConfig,
    // Agent registry for routing
    agents: Arc<RwLock<HashMap<String, Box<dyn Agent>>>>,
    // Message routing
    message_router: Arc<RwLock<MessageRouter>>,
    // Event bus for agent coordination
    event_bus: EventBus,
    // Workflow coordinator
    workflow_coordinator: Arc<RwLock<WorkflowCoordinator>>,
    is_initialized: bool,
}

/// Routes messages between agents based on capabilities and availability
pub struct MessageRouter {
    routing_table: HashMap<AgentCapability, Vec<String>>,
    agent_loads: HashMap<String, f64>, // Load balancing
}

/// Event bus for agent coordination and communication
pub struct EventBus {
    // Channel for broadcasting events to all agents
    broadcast_tx: broadcast::Sender<AgentEvent>,
    // Channel for direct agent-to-agent communication
    direct_channels: HashMap<String, mpsc::UnboundedSender<AgentMessage>>,
}

/// Coordinates multi-agent workflows and task execution
pub struct WorkflowCoordinator {
    active_workflows: HashMap<Uuid, ActiveWorkflow>,
    workflow_templates: HashMap<String, WorkflowTemplate>,
}

/// Events that can be broadcast to agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    AgentRegistered { agent_id: String, capabilities: Vec<AgentCapability> },
    AgentUnregistered { agent_id: String },
    TaskCompleted { workflow_id: Uuid, task_id: String, result: TaskResult },
    TaskFailed { workflow_id: Uuid, task_id: String, error: String },
    WorkflowStarted { workflow_id: Uuid, template_name: String },
    WorkflowCompleted { workflow_id: Uuid, success: bool },
    ContextUpdated { context_type: String, data: serde_json::Value },
    Custom { event_type: String, data: serde_json::Value },
}

/// Direct messages between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub from: String,
    pub to: String,
    pub message_type: MessageType,
    pub payload: serde_json::Value,
    pub correlation_id: Option<Uuid>,
}

/// Types of messages agents can send to each other
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Request,
    Response,
    Notification,
    Delegation,
    Collaboration,
}

/// Represents an active multi-agent workflow
#[derive(Debug, Clone)]
pub struct ActiveWorkflow {
    pub id: Uuid,
    pub template_name: String,
    pub tasks: HashMap<String, WorkflowTask>,
    pub execution_order: Vec<String>,
    pub current_step: usize,
    pub status: WorkflowStatus,
    pub context: serde_json::Value,
    pub results: HashMap<String, TaskResult>,
}

/// Template for defining multi-agent workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub name: String,
    pub description: String,
    pub tasks: HashMap<String, WorkflowTaskTemplate>,
    pub execution_strategy: ExecutionStrategy,
    pub dependencies: HashMap<String, Vec<String>>, // task_id -> [dependency_ids]
}

/// Task within a workflow
#[derive(Debug, Clone)]
pub struct WorkflowTask {
    pub id: String,
    pub template: WorkflowTaskTemplate,
    pub status: TaskStatus,
    pub assigned_agent: Option<String>,
    pub result: Option<TaskResult>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Template for workflow tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTaskTemplate {
    pub name: String,
    pub description: String,
    pub required_capability: AgentCapability,
    pub request_template: serde_json::Value,
    pub timeout_seconds: u64,
    pub retry_count: u32,
    pub critical: bool, // If true, workflow fails if this task fails
}

/// Strategies for executing workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStrategy {
    Sequential,     // Execute tasks one after another
    Parallel,       // Execute all tasks simultaneously
    Dependency,     // Execute based on dependency graph
    Custom(String), // Custom execution logic
}

/// Status of workflows and tasks
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed,
    Retrying,
}

/// Results from task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub success: bool,
    pub data: serde_json::Value,
    pub artifacts: Vec<AgentArtifact>,
    pub execution_time_ms: u64,
    pub error: Option<String>,
}

impl AgentCommunicationHub {
    pub fn new() -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        
        Self {
            id: "communication-hub".to_string(),
            config: AgentConfig::default(),
            agents: Arc::new(RwLock::new(HashMap::new())),
            message_router: Arc::new(RwLock::new(MessageRouter::new())),
            event_bus: EventBus::new(broadcast_tx),
            workflow_coordinator: Arc::new(RwLock::new(WorkflowCoordinator::new())),
            is_initialized: false,
        }
    }

    /// Register an agent with the communication hub
    pub async fn register_agent(&self, agent: Box<dyn Agent>) -> Result<()> {
        let agent_id = agent.id().to_string();
        let capabilities = agent.capabilities();
        
        // Add to agent registry
        {
            let mut agents = self.agents.write().await;
            agents.insert(agent_id.clone(), agent);
        }
        
        // Update routing table
        {
            let mut router = self.message_router.write().await;
            router.update_routing_for_agent(&agent_id, &capabilities).await;
        }
        
        // Broadcast registration event
        let event = AgentEvent::AgentRegistered { agent_id: agent_id.clone(), capabilities };
        let _ = self.event_bus.broadcast(event).await;
        
        info!("Registered agent '{}' with communication hub", agent_id);
        Ok(())
    }

    /// Unregister an agent from the communication hub
    pub async fn unregister_agent(&self, agent_id: &str) -> Result<()> {
        {
            let mut agents = self.agents.write().await;
            agents.remove(agent_id);
        }
        
        // Remove from routing table
        {
            let mut router = self.message_router.write().await;
            router.remove_agent(agent_id).await;
        }
        
        // Broadcast unregistration event
        let event = AgentEvent::AgentUnregistered { agent_id: agent_id.to_string() };
        let _ = self.event_bus.broadcast(event).await;
        
        info!("Unregistered agent '{}' from communication hub", agent_id);
        Ok(())
    }

    /// Route a request to the most appropriate agent
    pub async fn route_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        // Find the best agent for this request
        let target_agent_id = {
            let router = self.message_router.read().await;
            router.find_best_agent(&request.request_type).await?
        };
        
        // Get the agent and execute the request
        let agents = self.agents.read().await;
        if let Some(agent) = agents.get(&target_agent_id) {
            info!("Routing request {} to agent '{}'", request.id, target_agent_id);
            agent.handle_request(request).await
        } else {
            Err(anyhow!("Agent '{}' not found in registry", target_agent_id))
        }
    }

    /// Send a direct message between agents
    pub async fn send_message(&self, message: AgentMessage) -> Result<()> {
        self.event_bus.send_direct_message(message).await
    }

    /// Start a multi-agent workflow
    pub async fn start_workflow(&self, template_name: &str, context: serde_json::Value) -> Result<Uuid> {
        let workflow_id = {
            let mut coordinator = self.workflow_coordinator.write().await;
            coordinator.start_workflow(template_name, context).await?
        };
        
        // Broadcast workflow started event
        let event = AgentEvent::WorkflowStarted { 
            workflow_id, 
            template_name: template_name.to_string() 
        };
        let _ = self.event_bus.broadcast(event).await;
        
        info!("Started workflow '{}' with ID {}", template_name, workflow_id);
        
        // Begin executing the workflow
        self.execute_workflow(workflow_id).await?;
        
        Ok(workflow_id)
    }

    /// Execute a workflow by coordinating tasks across agents
    async fn execute_workflow(&self, workflow_id: Uuid) -> Result<()> {
        let execution_strategy = {
            let coordinator = self.workflow_coordinator.read().await;
            let workflow = coordinator.get_workflow(workflow_id).await?;
            let template_name = &workflow.template_name;
            coordinator.get_workflow_template(template_name).await
                .map(|t| t.execution_strategy.clone())
                .unwrap_or(ExecutionStrategy::Sequential)
        };
        
        match execution_strategy {
            ExecutionStrategy::Sequential => {
                self.execute_sequential_workflow(workflow_id).await
            }
            ExecutionStrategy::Parallel => {
                self.execute_parallel_workflow(workflow_id).await
            }
            ExecutionStrategy::Dependency => {
                self.execute_dependency_workflow(workflow_id).await
            }
            ExecutionStrategy::Custom(_) => {
                Err(anyhow!("Custom execution strategies not yet implemented"))
            }
        }
    }

    /// Execute workflow tasks sequentially
    async fn execute_sequential_workflow(&self, workflow_id: Uuid) -> Result<()> {
        let (execution_order, tasks) = {
            let coordinator = self.workflow_coordinator.read().await;
            let workflow = coordinator.get_workflow(workflow_id).await?;
            (workflow.execution_order.clone(), workflow.tasks.clone())
        };
        
        for task_id in &execution_order {
            if let Some(task) = tasks.get(task_id) {
                let result = self.execute_task(workflow_id, task.clone()).await;
                
                match result {
                    Ok(task_result) => {
                        {
                            let mut coordinator = self.workflow_coordinator.write().await;
                            coordinator.update_task_result(workflow_id, task_id, task_result).await?;
                        }
                        
                        // Broadcast task completion
                        let task_result = {
                            let coordinator = self.workflow_coordinator.read().await;
                            coordinator.get_task_result(workflow_id, task_id).await?.unwrap()
                        };
                        let event = AgentEvent::TaskCompleted {
                            workflow_id,
                            task_id: task_id.clone(),
                            result: task_result,
                        };
                        let _ = self.event_bus.broadcast(event).await;
                    }
                    Err(e) => {
                        error!("Task '{}' in workflow {} failed: {}", task_id, workflow_id, e);
                        
                        // Broadcast task failure
                        let event = AgentEvent::TaskFailed {
                            workflow_id,
                            task_id: task_id.clone(),
                            error: e.to_string(),
                        };
                        let _ = self.event_bus.broadcast(event).await;
                        
                        if task.template.critical {
                            {
                                let mut coordinator = self.workflow_coordinator.write().await;
                                coordinator.mark_workflow_failed(workflow_id).await?;
                            }
                            return Err(e);
                        }
                    }
                }
            }
        }
        
        {
            let mut coordinator = self.workflow_coordinator.write().await;
            coordinator.mark_workflow_completed(workflow_id).await?;
        }
        
        // Broadcast workflow completion
        let event = AgentEvent::WorkflowCompleted { workflow_id, success: true };
        let _ = self.event_bus.broadcast(event).await;
        
        Ok(())
    }

    /// Execute workflow tasks in parallel
    async fn execute_parallel_workflow(&self, _workflow_id: Uuid) -> Result<()> {
        // TODO: Implement parallel execution using tokio::spawn
        Err(anyhow!("Parallel workflow execution not yet implemented"))
    }

    /// Execute workflow based on dependency graph
    async fn execute_dependency_workflow(&self, _workflow_id: Uuid) -> Result<()> {
        // TODO: Implement dependency-based execution using topological sort
        Err(anyhow!("Dependency-based workflow execution not yet implemented"))
    }

    /// Execute a single task by routing to an appropriate agent
    async fn execute_task(&self, workflow_id: Uuid, task: WorkflowTask) -> Result<TaskResult> {
        let start_time = std::time::Instant::now();
        
        // Find an agent capable of handling this task
        let agent_id = {
            let router = self.message_router.read().await;
            router.find_agent_by_capability(&task.template.required_capability).await?
        };
        
        // Create agent request from task template
        let request = AgentRequest {
            id: Uuid::new_v4(),
            request_type: self.capability_to_request_type(&task.template.required_capability),
            payload: task.template.request_template.clone(),
            context: AgentContext {
                project_root: None,
                current_directory: std::env::current_dir().unwrap_or_default().to_string_lossy().to_string(),
                current_branch: None,
                open_files: vec![],
                recent_commands: vec![],
                environment_vars: HashMap::new(),
                user_preferences: HashMap::new(),
            },
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("workflow_id".to_string(), workflow_id.to_string());
                meta.insert("task_id".to_string(), task.id.clone());
                meta
            },
        };
        
        // Execute the request
        let agents = self.agents.read().await;
        if let Some(agent) = agents.get(&agent_id) {
            let response = agent.handle_request(request).await?;
            let execution_time = start_time.elapsed().as_millis() as u64;
            
            Ok(TaskResult {
                success: response.success,
                data: response.payload,
                artifacts: response.artifacts,
                execution_time_ms: execution_time,
                error: if response.success { None } else { Some("Task failed".to_string()) },
            })
        } else {
            Err(anyhow!("No agent found with capability {:?}", task.template.required_capability))
        }
    }

    /// Convert agent capability to request type
    fn capability_to_request_type(&self, capability: &AgentCapability) -> AgentRequestType {
        match capability {
            AgentCapability::CodeGeneration => AgentRequestType::GenerateCode,
            AgentCapability::CodeAnalysis => AgentRequestType::AnalyzeCode,
            AgentCapability::SecurityAnalysis => AgentRequestType::CheckSecurity,
            AgentCapability::QualityAssurance => AgentRequestType::ValidateQuality,
            AgentCapability::ProjectManagement => AgentRequestType::ManageProject,
            AgentCapability::WorkflowOrchestration => AgentRequestType::ExecuteWorkflow,
            AgentCapability::ContextManagement => AgentRequestType::UpdateContext,
            AgentCapability::FileSystem => AgentRequestType::ProcessFile,
            AgentCapability::GitIntegration => AgentRequestType::GitOperation,
            AgentCapability::LSPIntegration => AgentRequestType::LSPQuery,
            AgentCapability::TerminalIntegration => AgentRequestType::TerminalCommand,
            AgentCapability::Custom(name) => AgentRequestType::Custom(name.clone()),
        }
    }

    /// Get the status of a workflow
    pub async fn get_workflow_status(&self, workflow_id: Uuid) -> Result<WorkflowStatus> {
        let coordinator = self.workflow_coordinator.read().await;
        coordinator.get_workflow_status(workflow_id).await
    }

    /// List all active workflows
    pub async fn list_active_workflows(&self) -> Vec<Uuid> {
        let coordinator = self.workflow_coordinator.read().await;
        coordinator.list_active_workflows().await
    }

    /// Subscribe to agent events
    pub fn subscribe_to_events(&self) -> broadcast::Receiver<AgentEvent> {
        self.event_bus.subscribe()
    }
    
    /// List all registered agents
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
}

#[async_trait]
impl Agent for AgentCommunicationHub {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Agent Communication Hub"
    }

    fn description(&self) -> &str {
        "Central hub for routing messages between agents and coordinating multi-agent workflows"
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::WorkflowOrchestration,
            AgentCapability::Custom("MessageRouting".to_string()),
            AgentCapability::Custom("AgentCoordination".to_string()),
        ]
    }

    async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        match request.request_type {
            AgentRequestType::ExecuteWorkflow => {
                if let Ok(workflow_request) = serde_json::from_value::<WorkflowRequest>(request.payload.clone()) {
                    let workflow_id = self.start_workflow(&workflow_request.template_name, workflow_request.context).await?;
                    
                    Ok(AgentResponse {
                        request_id: request.id,
                        agent_id: self.id.clone(),
                        success: true,
                        payload: serde_json::json!({"workflow_id": workflow_id}),
                        artifacts: Vec::new(),
                        next_actions: vec![
                            SuggestedAction {
                                action_type: ActionType::Custom("MonitorWorkflow".to_string()),
                                description: format!("Monitor workflow {} progress", workflow_id),
                                command: Some(format!("workflow status {}", workflow_id)),
                                priority: ActionPriority::Low,
                                safe_to_auto_execute: true,
                            }
                        ],
                        metadata: HashMap::new(),
                    })
                } else {
                    Ok(AgentResponse {
                        request_id: request.id,
                        agent_id: self.id.clone(),
                        success: false,
                        payload: serde_json::json!({"error": "Invalid workflow request format"}),
                        artifacts: Vec::new(),
                        next_actions: Vec::new(),
                        metadata: HashMap::new(),
                    })
                }
            }
            AgentRequestType::Custom(ref custom_type) if custom_type == "RouteRequest" => {
                if let Ok(route_request) = serde_json::from_value::<RouteRequest>(request.payload.clone()) {
                    let response = self.route_request(route_request.inner_request).await?;
                    
                    Ok(AgentResponse {
                        request_id: request.id,
                        agent_id: self.id.clone(),
                        success: true,
                        payload: serde_json::to_value(response)?,
                        artifacts: Vec::new(),
                        next_actions: Vec::new(),
                        metadata: HashMap::new(),
                    })
                } else {
                    Ok(AgentResponse {
                        request_id: request.id,
                        agent_id: self.id.clone(),
                        success: false,
                        payload: serde_json::json!({"error": "Invalid route request format"}),
                        artifacts: Vec::new(),
                        next_actions: Vec::new(),
                        metadata: HashMap::new(),
                    })
                }
            }
            _ => Err(anyhow!("Communication Hub cannot handle request type: {:?}", request.request_type))
        }
    }

    fn can_handle(&self, request_type: &AgentRequestType) -> bool {
        matches!(
            request_type,
            AgentRequestType::ExecuteWorkflow
        ) || matches!(
            request_type,
            AgentRequestType::Custom(custom_type) if custom_type == "RouteRequest"
        )
    }

    async fn status(&self) -> AgentStatus {
        let active_workflows = self.list_active_workflows().await.len();
        let current_task = if active_workflows > 0 {
            Some(format!("Managing {} active workflows", active_workflows))
        } else {
            None
        };

        AgentStatus {
            is_healthy: self.is_initialized,
            is_busy: active_workflows > 0,
            last_activity: chrono::Utc::now(),
            current_task,
            error_message: None,
        }
    }

    async fn initialize(&mut self, config: AgentConfig) -> Result<()> {
        self.config = config;
        
        // Initialize default workflow templates
        {
            let mut coordinator = self.workflow_coordinator.write().await;
            coordinator.register_default_templates().await?;
        }
        
        self.is_initialized = true;
        info!("Agent Communication Hub initialized");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Cancel all active workflows
        let active_workflows = self.list_active_workflows().await;
        for workflow_id in active_workflows {
            let mut coordinator = self.workflow_coordinator.write().await;
            let _ = coordinator.cancel_workflow(workflow_id).await;
        }
        
        self.is_initialized = false;
        info!("Agent Communication Hub shut down");
        Ok(())
    }
}

/// Request structures for communication hub operations
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowRequest {
    pub template_name: String,
    pub context: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RouteRequest {
    pub inner_request: AgentRequest,
}

// Implementations for helper structs
impl MessageRouter {
    pub fn new() -> Self {
        Self {
            routing_table: HashMap::new(),
            agent_loads: HashMap::new(),
        }
    }

    pub async fn update_routing_for_agent(&mut self, agent_id: &str, capabilities: &[AgentCapability]) {
        for capability in capabilities {
            self.routing_table
                .entry(capability.clone())
                .or_insert_with(Vec::new)
                .push(agent_id.to_string());
        }
        self.agent_loads.insert(agent_id.to_string(), 0.0);
    }

    pub async fn remove_agent(&mut self, agent_id: &str) {
        for agents in self.routing_table.values_mut() {
            agents.retain(|id| id != agent_id);
        }
        self.agent_loads.remove(agent_id);
    }

    pub async fn find_best_agent(&self, request_type: &AgentRequestType) -> Result<String> {
        let capability = self.request_type_to_capability(request_type);
        self.find_agent_by_capability(&capability).await
    }

    pub async fn find_agent_by_capability(&self, capability: &AgentCapability) -> Result<String> {
        if let Some(agents) = self.routing_table.get(capability) {
            if agents.is_empty() {
                return Err(anyhow!("No agents available for capability: {:?}", capability));
            }
            
            // Simple load balancing - choose agent with lowest load
            let best_agent = agents
                .iter()
                .min_by(|a, b| {
                    let load_a = self.agent_loads.get(*a).unwrap_or(&0.0);
                    let load_b = self.agent_loads.get(*b).unwrap_or(&0.0);
                    load_a.partial_cmp(load_b).unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap();
            
            Ok(best_agent.clone())
        } else {
            Err(anyhow!("No agents registered for capability: {:?}", capability))
        }
    }

    fn request_type_to_capability(&self, request_type: &AgentRequestType) -> AgentCapability {
        match request_type {
            AgentRequestType::GenerateCode => AgentCapability::CodeGeneration,
            AgentRequestType::AnalyzeCode => AgentCapability::CodeAnalysis,
            AgentRequestType::CheckSecurity => AgentCapability::SecurityAnalysis,
            AgentRequestType::ValidateQuality => AgentCapability::QualityAssurance,
            AgentRequestType::ManageProject => AgentCapability::ProjectManagement,
            AgentRequestType::ExecuteWorkflow => AgentCapability::WorkflowOrchestration,
            AgentRequestType::UpdateContext => AgentCapability::ContextManagement,
            AgentRequestType::ProcessFile => AgentCapability::FileSystem,
            AgentRequestType::GitOperation => AgentCapability::GitIntegration,
            AgentRequestType::LSPQuery => AgentCapability::LSPIntegration,
            AgentRequestType::TerminalCommand => AgentCapability::TerminalIntegration,
            AgentRequestType::Custom(name) => AgentCapability::Custom(name.clone()),
        }
    }
}

impl EventBus {
    pub fn new(broadcast_tx: broadcast::Sender<AgentEvent>) -> Self {
        Self {
            broadcast_tx,
            direct_channels: HashMap::new(),
        }
    }

    pub async fn broadcast(&self, event: AgentEvent) -> Result<()> {
        self.broadcast_tx.send(event)?;
        Ok(())
    }

    pub async fn send_direct_message(&self, _message: AgentMessage) -> Result<()> {
        // TODO: Implement direct message routing
        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.broadcast_tx.subscribe()
    }
}

impl WorkflowCoordinator {
    pub fn new() -> Self {
        Self {
            active_workflows: HashMap::new(),
            workflow_templates: HashMap::new(),
        }
    }

    pub async fn register_default_templates(&mut self) -> Result<()> {
        // Register a simple code generation workflow
        let code_gen_template = WorkflowTemplate {
            name: "code_generation_workflow".to_string(),
            description: "Generate and validate code with security analysis".to_string(),
            tasks: {
                let mut tasks = HashMap::new();
                tasks.insert("generate".to_string(), WorkflowTaskTemplate {
                    name: "Generate Code".to_string(),
                    description: "Generate code based on requirements".to_string(),
                    required_capability: AgentCapability::CodeGeneration,
                    request_template: serde_json::json!({"requirements": ""}),
                    timeout_seconds: 30,
                    retry_count: 2,
                    critical: true,
                });
                tasks.insert("analyze_security".to_string(), WorkflowTaskTemplate {
                    name: "Security Analysis".to_string(),
                    description: "Analyze generated code for security issues".to_string(),
                    required_capability: AgentCapability::SecurityAnalysis,
                    request_template: serde_json::json!({"code": ""}),
                    timeout_seconds: 15,
                    retry_count: 1,
                    critical: false,
                });
                tasks
            },
            execution_strategy: ExecutionStrategy::Sequential,
            dependencies: HashMap::new(),
        };
        
        self.workflow_templates.insert("code_generation_workflow".to_string(), code_gen_template);
        Ok(())
    }

    pub async fn start_workflow(&mut self, template_name: &str, context: serde_json::Value) -> Result<Uuid> {
        let template = self.workflow_templates.get(template_name)
            .ok_or_else(|| anyhow!("Workflow template not found: {}", template_name))?
            .clone();

        let workflow_id = Uuid::new_v4();
        let mut tasks = HashMap::new();
        let mut execution_order = Vec::new();

        for (task_id, task_template) in &template.tasks {
            tasks.insert(task_id.clone(), WorkflowTask {
                id: task_id.clone(),
                template: task_template.clone(),
                status: TaskStatus::Pending,
                assigned_agent: None,
                result: None,
                started_at: None,
                completed_at: None,
            });
            execution_order.push(task_id.clone());
        }

        let workflow = ActiveWorkflow {
            id: workflow_id,
            template_name: template_name.to_string(),
            tasks,
            execution_order,
            current_step: 0,
            status: WorkflowStatus::Pending,
            context,
            results: HashMap::new(),
        };

        self.active_workflows.insert(workflow_id, workflow);
        Ok(workflow_id)
    }

    pub async fn get_workflow(&self, workflow_id: Uuid) -> Result<&ActiveWorkflow> {
        self.active_workflows.get(&workflow_id)
            .ok_or_else(|| anyhow!("Workflow not found: {}", workflow_id))
    }
    
    pub async fn get_workflow_template(&self, template_name: &str) -> Option<&WorkflowTemplate> {
        self.workflow_templates.get(template_name)
    }

    pub async fn get_workflow_status(&self, workflow_id: Uuid) -> Result<WorkflowStatus> {
        let workflow = self.get_workflow(workflow_id).await?;
        Ok(workflow.status.clone())
    }

    pub async fn list_active_workflows(&self) -> Vec<Uuid> {
        self.active_workflows.keys().cloned().collect()
    }

    pub async fn update_task_result(&mut self, workflow_id: Uuid, task_id: &str, result: TaskResult) -> Result<()> {
        if let Some(workflow) = self.active_workflows.get_mut(&workflow_id) {
            workflow.results.insert(task_id.to_string(), result);
            
            if let Some(task) = workflow.tasks.get_mut(task_id) {
                task.status = TaskStatus::Completed;
                task.completed_at = Some(chrono::Utc::now());
            }
        }
        Ok(())
    }

    pub async fn get_task_result(&self, workflow_id: Uuid, task_id: &str) -> Result<Option<TaskResult>> {
        let workflow = self.get_workflow(workflow_id).await?;
        Ok(workflow.results.get(task_id).cloned())
    }

    pub async fn mark_workflow_completed(&mut self, workflow_id: Uuid) -> Result<()> {
        if let Some(workflow) = self.active_workflows.get_mut(&workflow_id) {
            workflow.status = WorkflowStatus::Completed;
        }
        Ok(())
    }

    pub async fn mark_workflow_failed(&mut self, workflow_id: Uuid) -> Result<()> {
        if let Some(workflow) = self.active_workflows.get_mut(&workflow_id) {
            workflow.status = WorkflowStatus::Failed;
        }
        Ok(())
    }

    pub async fn cancel_workflow(&mut self, workflow_id: Uuid) -> Result<()> {
        if let Some(workflow) = self.active_workflows.get_mut(&workflow_id) {
            workflow.status = WorkflowStatus::Cancelled;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_communication_hub_creation() {
        let hub = AgentCommunicationHub::new();
        assert_eq!(hub.id(), "communication-hub");
        assert_eq!(hub.name(), "Agent Communication Hub");
    }

    #[tokio::test]
    async fn test_message_router() {
        let mut router = MessageRouter::new();
        let capabilities = vec![AgentCapability::CodeGeneration];
        
        router.update_routing_for_agent("test-agent", &capabilities).await;
        
        let agent_id = router.find_agent_by_capability(&AgentCapability::CodeGeneration).await.unwrap();
        assert_eq!(agent_id, "test-agent");
    }

    #[tokio::test]
    async fn test_workflow_coordinator() {
        let mut coordinator = WorkflowCoordinator::new();
        coordinator.register_default_templates().await.unwrap();
        
        let context = serde_json::json!({"language": "rust"});
        let workflow_id = coordinator.start_workflow("code_generation_workflow", context).await.unwrap();
        
        let status = coordinator.get_workflow_status(workflow_id).await.unwrap();
        assert_eq!(status, WorkflowStatus::Pending);
    }
}