use async_trait::async_trait;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tokio::sync::RwLock;
use std::sync::Arc;

use crate::ai::agents::{
    Agent, AgentCapability, AgentStatus, AgentConfig, AgentRequest, 
    AgentResponse, AgentContext, AgentRequestType, RequestPriority
};

/// Workflow orchestration agent that coordinates multi-step tasks across multiple agents
#[derive(Debug)]
pub struct WorkflowOrchestrationAgent {
    id: String,
    config: AgentConfig,
    active_workflows: Arc<RwLock<HashMap<Uuid, WorkflowExecution>>>,
    workflow_templates: Arc<RwLock<HashMap<String, WorkflowTemplate>>>,
    status: AgentStatus,
}

/// Template for defining reusable workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub steps: Vec<WorkflowStep>,
    pub required_agents: Vec<String>,
    pub parameters: HashMap<String, WorkflowParameter>,
    pub timeout_seconds: Option<u64>,
}

/// Individual step in a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    pub agent_type: String,
    pub action: WorkflowAction,
    pub depends_on: Vec<String>,
    pub parameters: HashMap<String, serde_json::Value>,
    pub timeout_seconds: Option<u64>,
    pub retry_count: Option<u32>,
    pub on_failure: Option<FailureAction>,
}

/// Action to be performed in a workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowAction {
    ExecuteCommand { command: String },
    GenerateCode { language: String, requirements: String },
    AnalyzeFile { file_path: String },
    SecurityScan { target: String },
    Custom { action_type: String, payload: serde_json::Value },
}

/// Action to take on step failure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureAction {
    Retry,
    Skip,
    Abort,
    Fallback { alternative_step: String },
}

/// Parameter definition for workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowParameter {
    pub name: String,
    pub parameter_type: String,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
    pub description: String,
}

/// Runtime execution state of a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecution {
    pub id: Uuid,
    pub template_id: String,
    pub status: WorkflowExecutionStatus,
    pub current_step: Option<String>,
    pub completed_steps: Vec<String>,
    pub failed_steps: Vec<String>,
    pub step_results: HashMap<String, serde_json::Value>,
    pub parameters: HashMap<String, serde_json::Value>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

/// Status of workflow execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowExecutionStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl WorkflowOrchestrationAgent {
    pub fn new() -> Self {
        Self {
            id: "workflow-orchestration".to_string(),
            config: AgentConfig::default(),
            active_workflows: Arc::new(RwLock::new(HashMap::new())),
            workflow_templates: Arc::new(RwLock::new(HashMap::new())),
            status: AgentStatus::Idle,
        }
    }

    /// Register a new workflow template
    pub async fn register_template(&self, template: WorkflowTemplate) -> Result<()> {
        let mut templates = self.workflow_templates.write().await;
        templates.insert(template.id.clone(), template);
        Ok(())
    }

    /// Start executing a workflow from template
    pub async fn execute_workflow(
        &self,
        template_id: String,
        parameters: HashMap<String, serde_json::Value>,
    ) -> Result<Uuid> {
        let templates = self.workflow_templates.read().await;
        let template = templates.get(&template_id)
            .ok_or_else(|| anyhow!("Workflow template not found: {}", template_id))?;

        let workflow_id = Uuid::new_v4();
        let execution = WorkflowExecution {
            id: workflow_id,
            template_id: template_id.clone(),
            status: WorkflowExecutionStatus::Pending,
            current_step: None,
            completed_steps: Vec::new(),
            failed_steps: Vec::new(),
            step_results: HashMap::new(),
            parameters,
            started_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            error: None,
        };

        let mut workflows = self.active_workflows.write().await;
        workflows.insert(workflow_id, execution);

        // Start workflow execution asynchronously
        self.start_workflow_execution(workflow_id).await?;

        Ok(workflow_id)
    }

    /// Start executing the workflow steps
    async fn start_workflow_execution(&self, workflow_id: Uuid) -> Result<()> {
        let mut workflows = self.active_workflows.write().await;
        if let Some(execution) = workflows.get_mut(&workflow_id) {
            execution.status = WorkflowExecutionStatus::Running;
            execution.updated_at = Utc::now();
        }
        drop(workflows);

        // Find next executable step
        if let Some(next_step) = self.get_next_executable_step(workflow_id).await? {
            self.execute_step(workflow_id, next_step).await?;
        }

        Ok(())
    }

    /// Get the next step that can be executed based on dependencies
    async fn get_next_executable_step(&self, workflow_id: Uuid) -> Result<Option<WorkflowStep>> {
        let workflows = self.active_workflows.read().await;
        let execution = workflows.get(&workflow_id)
            .ok_or_else(|| anyhow!("Workflow not found: {}", workflow_id))?;

        let templates = self.workflow_templates.read().await;
        let template = templates.get(&execution.template_id)
            .ok_or_else(|| anyhow!("Template not found: {}", execution.template_id))?;

        for step in &template.steps {
            // Skip if already completed or failed
            if execution.completed_steps.contains(&step.id) || execution.failed_steps.contains(&step.id) {
                continue;
            }

            // Check if all dependencies are satisfied
            let dependencies_satisfied = step.depends_on.iter()
                .all(|dep| execution.completed_steps.contains(dep));

            if dependencies_satisfied {
                return Ok(Some(step.clone()));
            }
        }

        Ok(None)
    }

    /// Execute a specific workflow step
    async fn execute_step(&self, workflow_id: Uuid, step: WorkflowStep) -> Result<()> {
        // Update current step
        {
            let mut workflows = self.active_workflows.write().await;
            if let Some(execution) = workflows.get_mut(&workflow_id) {
                execution.current_step = Some(step.id.clone());
                execution.updated_at = Utc::now();
            }
        }

        // Execute the step action
        match self.execute_step_action(&step).await {
            Ok(result) => {
                self.complete_step(workflow_id, step.id, Some(result)).await?;
            }
            Err(e) => {
                self.handle_step_failure(workflow_id, step, e).await?;
            }
        }

        // Check if there are more steps to execute
        if let Some(next_step) = self.get_next_executable_step(workflow_id).await? {
            self.execute_step(workflow_id, next_step).await?;
        } else {
            // Check if workflow is complete
            self.check_workflow_completion(workflow_id).await?;
        }

        Ok(())
    }

    /// Execute the actual step action
    async fn execute_step_action(&self, step: &WorkflowStep) -> Result<serde_json::Value> {
        match &step.action {
            WorkflowAction::ExecuteCommand { command } => {
                // In a real implementation, this would integrate with the command execution system
                Ok(serde_json::json!({
                    "action": "command_executed",
                    "command": command,
                    "status": "success"
                }))
            }
            WorkflowAction::GenerateCode { language, requirements } => {
                Ok(serde_json::json!({
                    "action": "code_generated",
                    "language": language,
                    "requirements": requirements,
                    "status": "success"
                }))
            }
            WorkflowAction::AnalyzeFile { file_path } => {
                Ok(serde_json::json!({
                    "action": "file_analyzed",
                    "file_path": file_path,
                    "status": "success"
                }))
            }
            WorkflowAction::SecurityScan { target } => {
                Ok(serde_json::json!({
                    "action": "security_scan",
                    "target": target,
                    "status": "success"
                }))
            }
            WorkflowAction::Custom { action_type, payload } => {
                Ok(serde_json::json!({
                    "action": action_type,
                    "payload": payload,
                    "status": "success"
                }))
            }
        }
    }

    /// Mark a step as completed
    async fn complete_step(&self, workflow_id: Uuid, step_id: String, result: Option<serde_json::Value>) -> Result<()> {
        let mut workflows = self.active_workflows.write().await;
        if let Some(execution) = workflows.get_mut(&workflow_id) {
            execution.completed_steps.push(step_id.clone());
            execution.current_step = None;
            execution.updated_at = Utc::now();
            
            if let Some(result) = result {
                execution.step_results.insert(step_id, result);
            }
        }
        Ok(())
    }

    /// Handle step failure
    async fn handle_step_failure(&self, workflow_id: Uuid, step: WorkflowStep, error: anyhow::Error) -> Result<()> {
        let mut workflows = self.active_workflows.write().await;
        if let Some(execution) = workflows.get_mut(&workflow_id) {
            execution.failed_steps.push(step.id.clone());
            execution.current_step = None;
            execution.error = Some(error.to_string());
            execution.updated_at = Utc::now();
            
            // Handle failure action if specified
            match &step.on_failure {
                Some(FailureAction::Retry) => {
                    // Implement retry logic
                },
                Some(FailureAction::Skip) => {
                    // Continue to next step
                },
                Some(FailureAction::Abort) | None => {
                    execution.status = WorkflowExecutionStatus::Failed;
                    execution.completed_at = Some(Utc::now());
                },
                Some(FailureAction::Fallback { alternative_step: _ }) => {
                    // Implement fallback logic
                },
            }
        }
        Ok(())
    }

    /// Check if workflow is complete
    async fn check_workflow_completion(&self, workflow_id: Uuid) -> Result<()> {
        let mut workflows = self.active_workflows.write().await;
        if let Some(execution) = workflows.get_mut(&workflow_id) {
            let templates = self.workflow_templates.read().await;
            if let Some(template) = templates.get(&execution.template_id) {
                let total_steps = template.steps.len();
                let completed_steps = execution.completed_steps.len();
                let failed_steps = execution.failed_steps.len();
                
                if completed_steps + failed_steps >= total_steps {
                    if failed_steps > 0 {
                        execution.status = WorkflowExecutionStatus::Failed;
                    } else {
                        execution.status = WorkflowExecutionStatus::Completed;
                    }
                    execution.completed_at = Some(Utc::now());
                    execution.updated_at = Utc::now();
                }
            }
        }
        Ok(())
    }

    /// Get workflow execution status
    pub async fn get_workflow_status(&self, workflow_id: Uuid) -> Option<WorkflowExecution> {
        let workflows = self.active_workflows.read().await;
        workflows.get(&workflow_id).cloned()
    }

    /// List all active workflows
    pub async fn list_active_workflows(&self) -> Vec<WorkflowExecution> {
        let workflows = self.active_workflows.read().await;
        workflows.values().cloned().collect()
    }

    /// Cancel a running workflow
    pub async fn cancel_workflow(&self, workflow_id: Uuid) -> Result<()> {
        let mut workflows = self.active_workflows.write().await;
        if let Some(execution) = workflows.get_mut(&workflow_id) {
            execution.status = WorkflowExecutionStatus::Cancelled;
            execution.completed_at = Some(Utc::now());
            execution.updated_at = Utc::now();
        }
        Ok(())
    }
}

#[async_trait]
impl Agent for WorkflowOrchestrationAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Workflow Orchestration Agent"
    }

    fn description(&self) -> &str {
        "Coordinates multi-step tasks and manages workflow execution across multiple agents"
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::WorkflowOrchestration,
            AgentCapability::TaskCoordination,
        ]
    }

    fn status(&self) -> AgentStatus {
        self.status
    }

    fn config(&self) -> &AgentConfig {
        &self.config
    }

    async fn initialize(&mut self, _context: AgentContext) -> Result<()> {
        self.status = AgentStatus::Ready;
        
        // Register some default workflow templates
        self.register_default_templates().await?;
        
        Ok(())
    }

    async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        match request.request_type {
            AgentRequestType::WorkflowExecution => {
                let template_id = request.content;
                let workflow_id = self.execute_workflow(template_id, request.context).await?;
                
                Ok(AgentResponse {
                    request_id: request.id,
                    agent_id: self.id.clone(),
                    success: true,
                    data: Some(serde_json::json!({
                        "workflow_id": workflow_id,
                        "status": "started"
                    })),
                    error: None,
                    artifacts: Vec::new(),
                    suggested_actions: Vec::new(),
                    metadata: HashMap::new(),
                })
            }
            AgentRequestType::StatusQuery => {
                let workflows = self.list_active_workflows().await;
                
                Ok(AgentResponse {
                    request_id: request.id,
                    agent_id: self.id.clone(),
                    success: true,
                    data: Some(serde_json::to_value(workflows)?),
                    error: None,
                    artifacts: Vec::new(),
                    suggested_actions: Vec::new(),
                    metadata: HashMap::new(),
                })
            }
            _ => {
                Ok(AgentResponse {
                    request_id: request.id,
                    agent_id: self.id.clone(),
                    success: false,
                    data: None,
                    error: Some("Unsupported request type for workflow orchestration".to_string()),
                    artifacts: Vec::new(),
                    suggested_actions: Vec::new(),
                    metadata: HashMap::new(),
                })
            }
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Cancel all active workflows
        let workflow_ids: Vec<Uuid> = {
            let workflows = self.active_workflows.read().await;
            workflows.keys().cloned().collect()
        };
        
        for workflow_id in workflow_ids {
            let _ = self.cancel_workflow(workflow_id).await;
        }
        
        self.status = AgentStatus::Stopped;
        Ok(())
    }
}

impl WorkflowOrchestrationAgent {
    /// Register default workflow templates
    async fn register_default_templates(&self) -> Result<()> {
        // Example: Code review workflow
        let code_review_template = WorkflowTemplate {
            id: "code-review".to_string(),
            name: "Code Review Workflow".to_string(),
            description: "Automated code review process".to_string(),
            steps: vec![
                WorkflowStep {
                    id: "analyze-code".to_string(),
                    name: "Analyze Code".to_string(),
                    agent_type: "code-generation".to_string(),
                    action: WorkflowAction::AnalyzeFile { file_path: "{{file_path}}".to_string() },
                    depends_on: vec![],
                    parameters: HashMap::new(),
                    timeout_seconds: Some(60),
                    retry_count: Some(2),
                    on_failure: Some(FailureAction::Abort),
                },
                WorkflowStep {
                    id: "security-scan".to_string(),
                    name: "Security Scan".to_string(),
                    agent_type: "security".to_string(),
                    action: WorkflowAction::SecurityScan { target: "{{file_path}}".to_string() },
                    depends_on: vec!["analyze-code".to_string()],
                    parameters: HashMap::new(),
                    timeout_seconds: Some(120),
                    retry_count: Some(1),
                    on_failure: Some(FailureAction::Skip),
                },
            ],
            required_agents: vec!["code-generation".to_string(), "security".to_string()],
            parameters: HashMap::from([
                ("file_path".to_string(), WorkflowParameter {
                    name: "file_path".to_string(),
                    parameter_type: "string".to_string(),
                    required: true,
                    default_value: None,
                    description: "Path to the file to review".to_string(),
                }),
            ]),
            timeout_seconds: Some(300),
        };
        
        self.register_template(code_review_template).await?;
        Ok(())
    }
}

impl Default for WorkflowOrchestrationAgent {
    fn default() -> Self {
        Self::new()
    }
}
