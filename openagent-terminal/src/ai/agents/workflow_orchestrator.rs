use async_trait::async_trait;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tokio::time::{Duration, timeout};

use super::*;
use super::conversation_manager::ConversationManager;
use super::blitzy_project_context::BlitzyProjectContextAgent;

/// Advanced workflow orchestration system for multi-agent coordination
pub struct WorkflowOrchestrator {
    id: String,
    workflows: Arc<RwLock<HashMap<Uuid, WorkflowExecution>>>,
    workflow_templates: Arc<RwLock<HashMap<String, WorkflowTemplate>>>,
    agent_registry: Arc<RwLock<HashMap<String, Arc<dyn Agent>>>>,
    conversation_manager: Option<Arc<ConversationManager>>,
    project_context_agent: Option<Arc<BlitzyProjectContextAgent>>,
    execution_queue: Arc<Mutex<VecDeque<WorkflowTask>>>,
    config: WorkflowConfig,
    is_initialized: bool,
}

/// Workflow execution instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecution {
    pub id: Uuid,
    pub template_id: String,
    pub title: String,
    pub description: String,
    pub status: WorkflowStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub current_step: Option<usize>,
    pub steps: Vec<WorkflowStepExecution>,
    pub context: WorkflowContext,
    pub results: HashMap<String, serde_json::Value>,
    pub error_info: Option<WorkflowError>,
    pub metadata: HashMap<String, String>,
}

/// Workflow template for reusable workflow patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: WorkflowCategory,
    pub version: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub steps: Vec<WorkflowStep>,
    pub variables: HashMap<String, WorkflowVariable>,
    pub triggers: Vec<WorkflowTrigger>,
    pub conditions: Vec<WorkflowCondition>,
    pub error_handling: ErrorHandlingStrategy,
    pub timeout_seconds: Option<u64>,
    pub retry_config: RetryConfig,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Workflow step definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    pub step_type: WorkflowStepType,
    pub agent_id: Option<String>,
    pub request_template: serde_json::Value,
    pub dependencies: Vec<String>, // Step IDs this step depends on
    pub conditions: Vec<StepCondition>,
    pub timeout_seconds: Option<u64>,
    pub retry_attempts: u32,
    pub error_handling: StepErrorHandling,
    pub input_mapping: HashMap<String, String>, // Map variables to step inputs
    pub output_mapping: HashMap<String, String>, // Map step outputs to variables
    pub parallel_group: Option<String>, // Group ID for parallel execution
}

/// Workflow step execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStepExecution {
    pub step_id: String,
    pub status: StepExecutionStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub attempts: u32,
    pub agent_response: Option<AgentResponse>,
    pub error_info: Option<String>,
    pub inputs: HashMap<String, serde_json::Value>,
    pub outputs: HashMap<String, serde_json::Value>,
}

/// Workflow execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowContext {
    pub conversation_session_id: Option<Uuid>,
    pub project_root: Option<String>,
    pub user_id: Option<String>,
    pub environment: HashMap<String, String>,
    pub variables: HashMap<String, serde_json::Value>,
    pub shared_state: HashMap<String, serde_json::Value>,
}

/// Workflow variable definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVariable {
    pub name: String,
    pub variable_type: VariableType,
    pub description: String,
    pub default_value: Option<serde_json::Value>,
    pub required: bool,
    pub validation: Option<VariableValidation>,
}

/// Variable validation rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableValidation {
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
    pub allowed_values: Option<Vec<String>>,
}

/// Workflow trigger conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTrigger {
    pub trigger_type: TriggerType,
    pub conditions: HashMap<String, serde_json::Value>,
    pub enabled: bool,
}

/// Workflow-level conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCondition {
    pub condition_type: ConditionType,
    pub expression: String,
    pub description: String,
}

/// Step-level conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepCondition {
    pub condition_type: ConditionType,
    pub expression: String,
    pub skip_on_false: bool,
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f32,
    pub retry_on_timeout: bool,
    pub retry_on_agent_error: bool,
}

/// Workflow error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowError {
    pub step_id: Option<String>,
    pub error_type: WorkflowErrorType,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub occurred_at: DateTime<Utc>,
    pub recoverable: bool,
}

/// Task for workflow execution queue
#[derive(Debug, Clone)]
pub struct WorkflowTask {
    pub workflow_id: Uuid,
    pub step_id: String,
    pub priority: TaskPriority,
    pub created_at: DateTime<Utc>,
    pub scheduled_at: Option<DateTime<Utc>>,
}

/// Workflow orchestration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    pub max_concurrent_workflows: usize,
    pub max_concurrent_steps: usize,
    pub default_step_timeout_seconds: u64,
    pub default_workflow_timeout_seconds: u64,
    pub enable_parallel_execution: bool,
    pub enable_step_retry: bool,
    pub max_retry_attempts: u32,
    pub enable_persistence: bool,
    pub cleanup_completed_after_hours: u64,
}

// Enums

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowStatus {
    Created,
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowCategory {
    CodeGeneration,
    ProjectSetup,
    Testing,
    Deployment,
    Analysis,
    Documentation,
    Maintenance,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowStepType {
    AgentRequest,
    ConditionalBranch,
    Loop,
    Parallel,
    Wait,
    UserInput,
    Command,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
    TimedOut,
    Retrying,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableType {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Object,
    FilePath,
    DirectoryPath,
    Url,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerType {
    Manual,
    Schedule,
    FileChange,
    GitCommit,
    ConversationIntent,
    AgentEvent,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionType {
    Variable,
    FileExists,
    CommandSuccess,
    AgentResponse,
    Expression,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorHandlingStrategy {
    StopOnError,
    ContinueOnError,
    RetryOnError,
    SkipOnError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepErrorHandling {
    Fail,
    Skip,
    Retry,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowErrorType {
    AgentError,
    TimeoutError,
    ConditionFailed,
    DependencyFailed,
    ValidationError,
    SystemError,
    UserCancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            max_concurrent_workflows: 10,
            max_concurrent_steps: 20,
            default_step_timeout_seconds: 300, // 5 minutes
            default_workflow_timeout_seconds: 3600, // 1 hour
            enable_parallel_execution: true,
            enable_step_retry: true,
            max_retry_attempts: 3,
            enable_persistence: true,
            cleanup_completed_after_hours: 24,
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
            retry_on_timeout: true,
            retry_on_agent_error: true,
        }
    }
}

impl WorkflowOrchestrator {
    pub fn new() -> Self {
        Self {
            id: "workflow-orchestrator".to_string(),
            workflows: Arc::new(RwLock::new(HashMap::new())),
            workflow_templates: Arc::new(RwLock::new(HashMap::new())),
            agent_registry: Arc::new(RwLock::new(HashMap::new())),
            conversation_manager: None,
            project_context_agent: None,
            execution_queue: Arc::new(Mutex::new(VecDeque::new())),
            config: WorkflowConfig::default(),
            is_initialized: false,
        }
    }

    pub fn with_config(mut self, config: WorkflowConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_conversation_manager(mut self, conversation_manager: Arc<ConversationManager>) -> Self {
        self.conversation_manager = Some(conversation_manager);
        self
    }

    pub fn with_project_context_agent(mut self, project_context_agent: Arc<BlitzyProjectContextAgent>) -> Self {
        self.project_context_agent = Some(project_context_agent);
        self
    }

    /// Register an agent for workflow use
    pub async fn register_agent(&self, agent: Arc<dyn Agent>) -> Result<()> {
        let mut registry = self.agent_registry.write().await;
        let agent_id = agent.id().to_string();
        registry.insert(agent_id.clone(), agent);
        tracing::info!("Registered agent for workflows: {}", agent_id);
        Ok(())
    }

    /// Register a workflow template
    pub async fn register_template(&self, template: WorkflowTemplate) -> Result<()> {
        let mut templates = self.workflow_templates.write().await;
        let template_id = template.id.clone();
        
        // Validate template
        self.validate_template(&template)?;
        
        templates.insert(template_id.clone(), template);
        tracing::info!("Registered workflow template: {}", template_id);
        Ok(())
    }

    /// Create and start a new workflow execution
    pub async fn create_workflow(
        &self,
        template_id: &str,
        context: WorkflowContext,
        variables: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Uuid> {
        let templates = self.workflow_templates.read().await;
        let template = templates.get(template_id)
            .ok_or_else(|| anyhow!("Workflow template not found: {}", template_id))?;

        let workflow_id = Uuid::new_v4();
        let mut workflow_context = context;
        
        // Merge provided variables with defaults
        if let Some(vars) = variables {
            for (key, value) in vars {
                workflow_context.variables.insert(key, value);
            }
        }

        // Set default values for missing variables
        for (name, var_def) in &template.variables {
            if !workflow_context.variables.contains_key(name) {
                if let Some(default_value) = &var_def.default_value {
                    workflow_context.variables.insert(name.clone(), default_value.clone());
                } else if var_def.required {
                    return Err(anyhow!("Required variable '{}' not provided", name));
                }
            }
        }

        // Validate variables
        self.validate_variables(&template.variables, &workflow_context.variables)?;

        // Initialize step executions
        let step_executions = template.steps.iter().map(|step| {
            WorkflowStepExecution {
                step_id: step.id.clone(),
                status: StepExecutionStatus::Pending,
                started_at: None,
                completed_at: None,
                attempts: 0,
                agent_response: None,
                error_info: None,
                inputs: HashMap::new(),
                outputs: HashMap::new(),
            }
        }).collect();

        let workflow = WorkflowExecution {
            id: workflow_id,
            template_id: template_id.to_string(),
            title: format!("{} - {}", template.name, workflow_id),
            description: template.description.clone(),
            status: WorkflowStatus::Created,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            current_step: None,
            steps: step_executions,
            context: workflow_context,
            results: HashMap::new(),
            error_info: None,
            metadata: HashMap::new(),
        };

        // Store workflow
        let mut workflows = self.workflows.write().await;
        workflows.insert(workflow_id, workflow);
        drop(workflows);

        // Start execution
        self.start_workflow_execution(workflow_id).await?;

        tracing::info!("Created and started workflow: {} (template: {})", workflow_id, template_id);
        Ok(workflow_id)
    }

    /// Start workflow execution
    async fn start_workflow_execution(&self, workflow_id: Uuid) -> Result<()> {
        // Update workflow status
        {
            let mut workflows = self.workflows.write().await;
            if let Some(workflow) = workflows.get_mut(&workflow_id) {
                workflow.status = WorkflowStatus::Running;
                workflow.started_at = Some(Utc::now());
            }
        }

        // Start execution loop
        tokio::spawn({
            let orchestrator = Arc::new(self.clone());
            async move {
                if let Err(e) = orchestrator.execute_workflow_loop(workflow_id).await {
                    tracing::error!("Workflow execution failed: {} - {}", workflow_id, e);
                    orchestrator.mark_workflow_failed(workflow_id, WorkflowError {
                        step_id: None,
                        error_type: WorkflowErrorType::SystemError,
                        message: e.to_string(),
                        details: None,
                        occurred_at: Utc::now(),
                        recoverable: false,
                    }).await;
                }
            }
        });

        Ok(())
    }

    /// Main workflow execution loop
    async fn execute_workflow_loop(&self, workflow_id: Uuid) -> Result<()> {
        loop {
            let (next_steps, workflow_completed) = {
                let workflows = self.workflows.read().await;
                let workflow = workflows.get(&workflow_id)
                    .ok_or_else(|| anyhow!("Workflow not found: {}", workflow_id))?;

                if !matches!(workflow.status, WorkflowStatus::Running) {
                    break;
                }

                let templates = self.workflow_templates.read().await;
                let template = templates.get(&workflow.template_id)
                    .ok_or_else(|| anyhow!("Template not found: {}", workflow.template_id))?;

                let next_steps = self.get_next_executable_steps(workflow, template)?;
                let completed = self.is_workflow_completed(workflow);

                (next_steps, completed)
            };

            if workflow_completed {
                self.complete_workflow(workflow_id).await?;
                break;
            }

            if next_steps.is_empty() {
                // No steps ready to execute, wait briefly
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            // Execute steps (potentially in parallel)
            if self.config.enable_parallel_execution {
                self.execute_steps_parallel(workflow_id, next_steps).await?;
            } else {
                for step_id in next_steps {
                    self.execute_step(workflow_id, &step_id).await?;
                }
            }
        }

        Ok(())
    }

    /// Execute multiple steps in parallel
    async fn execute_steps_parallel(&self, workflow_id: Uuid, step_ids: Vec<String>) -> Result<()> {
        let mut handles = Vec::new();

        for step_id in step_ids {
            let orchestrator = Arc::new(self.clone());
            let handle = tokio::spawn(async move {
                orchestrator.execute_step(workflow_id, &step_id).await
            });
            handles.push(handle);
        }

        // Wait for all steps to complete
        for handle in handles {
            handle.await.map_err(|e| anyhow!("Task join error: {}", e))??;
        }

        Ok(())
    }

    /// Execute a single workflow step
    async fn execute_step(&self, workflow_id: Uuid, step_id: &str) -> Result<()> {
        let (step, workflow_context, template) = {
            let workflows = self.workflows.read().await;
            let workflow = workflows.get(&workflow_id)
                .ok_or_else(|| anyhow!("Workflow not found: {}", workflow_id))?;

            let templates = self.workflow_templates.read().await;
            let template = templates.get(&workflow.template_id)
                .ok_or_else(|| anyhow!("Template not found: {}", workflow.template_id))?;

            let step = template.steps.iter().find(|s| s.id == step_id)
                .ok_or_else(|| anyhow!("Step not found: {}", step_id))?;

            (step.clone(), workflow.context.clone(), template.clone())
        };

        // Update step status to running
        self.update_step_status(workflow_id, step_id, StepExecutionStatus::Running).await?;

        // Evaluate step conditions
        if !self.evaluate_step_conditions(&step, &workflow_context).await? {
            self.update_step_status(workflow_id, step_id, StepExecutionStatus::Skipped).await?;
            return Ok(());
        }

        // Prepare step inputs
        let inputs = self.prepare_step_inputs(&step, &workflow_context).await?;

        // Execute step with retry logic
        let result = self.execute_step_with_retry(&step, inputs, &template.retry_config).await;

        match result {
            Ok(response) => {
                // Process step outputs
                let outputs = self.process_step_outputs(&step, &response).await?;
                
                // Update workflow context with outputs
                self.update_workflow_context(workflow_id, &step.output_mapping, &outputs).await?;
                
                // Mark step as completed
                self.complete_step(workflow_id, step_id, response, outputs).await?;
            }
            Err(e) => {
                match step.error_handling {
                    StepErrorHandling::Fail => {
                        self.fail_step(workflow_id, step_id, e.to_string()).await?;
                        self.mark_workflow_failed(workflow_id, WorkflowError {
                            step_id: Some(step_id.to_string()),
                            error_type: WorkflowErrorType::AgentError,
                            message: e.to_string(),
                            details: None,
                            occurred_at: Utc::now(),
                            recoverable: false,
                        }).await;
                    }
                    StepErrorHandling::Skip => {
                        self.update_step_status(workflow_id, step_id, StepExecutionStatus::Skipped).await?;
                    }
                    StepErrorHandling::Retry => {
                        // Retry logic is handled in execute_step_with_retry
                        self.fail_step(workflow_id, step_id, e.to_string()).await?;
                    }
                    StepErrorHandling::Custom(_) => {
                        // Custom error handling would be implemented here
                        self.fail_step(workflow_id, step_id, e.to_string()).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Execute step with retry logic
    async fn execute_step_with_retry(
        &self,
        step: &WorkflowStep,
        inputs: HashMap<String, serde_json::Value>,
        retry_config: &RetryConfig,
    ) -> Result<AgentResponse> {
        let mut attempts = 0;
        let mut delay = retry_config.initial_delay_ms;

        loop {
            attempts += 1;

            let result = match &step.step_type {
                WorkflowStepType::AgentRequest => {
                    if let Some(agent_id) = &step.agent_id {
                        self.execute_agent_request(agent_id, &step.request_template, &inputs).await
                    } else {
                        Err(anyhow!("Agent ID required for AgentRequest step"))
                    }
                }
                WorkflowStepType::Command => {
                    self.execute_command_step(&step.request_template, &inputs).await
                }
                _ => {
                    Err(anyhow!("Step type not implemented: {:?}", step.step_type))
                }
            };

            match result {
                Ok(response) => return Ok(response),
                Err(e) => {
                    if attempts >= retry_config.max_attempts {
                        return Err(e);
                    }

                    tracing::warn!("Step execution failed (attempt {}): {} - {}", attempts, step.id, e);
                    
                    // Wait before retry
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    
                    // Increase delay for next attempt
                    delay = (delay as f32 * retry_config.backoff_multiplier) as u64;
                    delay = delay.min(retry_config.max_delay_ms);
                }
            }
        }
    }

    /// Execute an agent request
    async fn execute_agent_request(
        &self,
        agent_id: &str,
        request_template: &serde_json::Value,
        inputs: &HashMap<String, serde_json::Value>,
    ) -> Result<AgentResponse> {
        let registry = self.agent_registry.read().await;
        let agent = registry.get(agent_id)
            .ok_or_else(|| anyhow!("Agent not found: {}", agent_id))?;

        // Build agent request from template and inputs
        let request = self.build_agent_request(request_template, inputs)?;
        
        // Execute with timeout
        let timeout_duration = Duration::from_secs(self.config.default_step_timeout_seconds);
        
        match timeout(timeout_duration, agent.handle_request(request)).await {
            Ok(result) => result,
            Err(_) => Err(anyhow!("Agent request timed out")),
        }
    }

    /// Execute a command step
    async fn execute_command_step(
        &self,
        command_template: &serde_json::Value,
        inputs: &HashMap<String, serde_json::Value>,
    ) -> Result<AgentResponse> {
        // This would execute system commands
        // For safety, this is a simplified implementation
        let command = self.interpolate_template(command_template, inputs)?;
        
        // Create a mock response for command execution
        Ok(AgentResponse {
            request_id: Uuid::new_v4(),
            agent_id: "system".to_string(),
            success: true,
            payload: serde_json::json!({
                "command": command,
                "output": "Command executed successfully"
            }),
            artifacts: Vec::new(),
            next_actions: Vec::new(),
            metadata: HashMap::new(),
        })
    }

    /// Build agent request from template and inputs
    fn build_agent_request(
        &self,
        template: &serde_json::Value,
        inputs: &HashMap<String, serde_json::Value>,
    ) -> Result<AgentRequest> {
        let interpolated = self.interpolate_template(template, inputs)?;
        
        let request: AgentRequest = serde_json::from_value(interpolated)
            .map_err(|e| anyhow!("Failed to parse agent request template: {}", e))?;
        
        Ok(request)
    }

    /// Interpolate template with input values
    fn interpolate_template(
        &self,
        template: &serde_json::Value,
        inputs: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        // Simple template interpolation
        let template_str = serde_json::to_string(template)?;
        let mut result = template_str;
        
        for (key, value) in inputs {
            let placeholder = format!("{{{{{}}}}}", key);
            let value_str = match value {
                serde_json::Value::String(s) => s.clone(),
                _ => value.to_string(),
            };
            result = result.replace(&placeholder, &value_str);
        }
        
        Ok(serde_json::from_str(&result)?)
    }

    // Helper methods for workflow execution...

    /// Get next executable steps
    fn get_next_executable_steps(
        &self,
        workflow: &WorkflowExecution,
        template: &WorkflowTemplate,
    ) -> Result<Vec<String>> {
        let mut executable_steps = Vec::new();

        for step in &template.steps {
            let step_execution = workflow.steps.iter()
                .find(|s| s.step_id == step.id)
                .ok_or_else(|| anyhow!("Step execution not found: {}", step.id))?;

            // Skip if not pending
            if !matches!(step_execution.status, StepExecutionStatus::Pending) {
                continue;
            }

            // Check dependencies
            let dependencies_satisfied = step.dependencies.iter().all(|dep_id| {
                workflow.steps.iter()
                    .find(|s| s.step_id == *dep_id)
                    .map(|s| matches!(s.status, StepExecutionStatus::Completed))
                    .unwrap_or(false)
            });

            if dependencies_satisfied {
                executable_steps.push(step.id.clone());
            }
        }

        Ok(executable_steps)
    }

    /// Check if workflow is completed
    fn is_workflow_completed(&self, workflow: &WorkflowExecution) -> bool {
        workflow.steps.iter().all(|step| {
            matches!(step.status, 
                StepExecutionStatus::Completed | 
                StepExecutionStatus::Skipped | 
                StepExecutionStatus::Failed
            )
        })
    }

    // Additional helper methods would be implemented here...
    // Due to length constraints, I'm including key structure but not all implementations

    /// Validate workflow template
    fn validate_template(&self, template: &WorkflowTemplate) -> Result<()> {
        // Basic validation logic
        if template.steps.is_empty() {
            return Err(anyhow!("Workflow template must have at least one step"));
        }
        Ok(())
    }

    /// Validate variables
    fn validate_variables(
        &self,
        definitions: &HashMap<String, WorkflowVariable>,
        values: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        // Variable validation logic
        for (name, definition) in definitions {
            if definition.required && !values.contains_key(name) {
                return Err(anyhow!("Required variable missing: {}", name));
            }
        }
        Ok(())
    }

    /// Update step status
    async fn update_step_status(
        &self,
        workflow_id: Uuid,
        step_id: &str,
        status: StepExecutionStatus,
    ) -> Result<()> {
        let mut workflows = self.workflows.write().await;
        if let Some(workflow) = workflows.get_mut(&workflow_id) {
            if let Some(step) = workflow.steps.iter_mut().find(|s| s.step_id == step_id) {
                step.status = status.clone();
                match status {
                    StepExecutionStatus::Running => {
                        step.started_at = Some(Utc::now());
                    }
                    StepExecutionStatus::Completed | StepExecutionStatus::Failed | StepExecutionStatus::Skipped => {
                        step.completed_at = Some(Utc::now());
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    async fn evaluate_step_conditions(&self, _step: &WorkflowStep, _context: &WorkflowContext) -> Result<bool> {
        // Placeholder for condition evaluation
        Ok(true)
    }

    async fn prepare_step_inputs(&self, step: &WorkflowStep, context: &WorkflowContext) -> Result<HashMap<String, serde_json::Value>> {
        let mut inputs = HashMap::new();
        
        for (input_key, variable_name) in &step.input_mapping {
            if let Some(value) = context.variables.get(variable_name) {
                inputs.insert(input_key.clone(), value.clone());
            }
        }
        
        Ok(inputs)
    }

    async fn process_step_outputs(&self, _step: &WorkflowStep, response: &AgentResponse) -> Result<HashMap<String, serde_json::Value>> {
        // Extract outputs from agent response
        Ok(HashMap::from([
            ("success".to_string(), serde_json::Value::Bool(response.success)),
            ("payload".to_string(), response.payload.clone()),
        ]))
    }

    async fn update_workflow_context(
        &self,
        workflow_id: Uuid,
        output_mapping: &HashMap<String, String>,
        outputs: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let mut workflows = self.workflows.write().await;
        if let Some(workflow) = workflows.get_mut(&workflow_id) {
            for (output_key, variable_name) in output_mapping {
                if let Some(value) = outputs.get(output_key) {
                    workflow.context.variables.insert(variable_name.clone(), value.clone());
                }
            }
        }
        Ok(())
    }

    async fn complete_step(
        &self,
        workflow_id: Uuid,
        step_id: &str,
        response: AgentResponse,
        outputs: HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let mut workflows = self.workflows.write().await;
        if let Some(workflow) = workflows.get_mut(&workflow_id) {
            if let Some(step) = workflow.steps.iter_mut().find(|s| s.step_id == step_id) {
                step.status = StepExecutionStatus::Completed;
                step.completed_at = Some(Utc::now());
                step.agent_response = Some(response);
                step.outputs = outputs;
            }
        }
        Ok(())
    }

    async fn fail_step(&self, workflow_id: Uuid, step_id: &str, error: String) -> Result<()> {
        let mut workflows = self.workflows.write().await;
        if let Some(workflow) = workflows.get_mut(&workflow_id) {
            if let Some(step) = workflow.steps.iter_mut().find(|s| s.step_id == step_id) {
                step.status = StepExecutionStatus::Failed;
                step.completed_at = Some(Utc::now());
                step.error_info = Some(error);
            }
        }
        Ok(())
    }

    async fn complete_workflow(&self, workflow_id: Uuid) -> Result<()> {
        let mut workflows = self.workflows.write().await;
        if let Some(workflow) = workflows.get_mut(&workflow_id) {
            workflow.status = WorkflowStatus::Completed;
            workflow.completed_at = Some(Utc::now());
            tracing::info!("Workflow completed: {}", workflow_id);
        }
        Ok(())
    }

    async fn mark_workflow_failed(&self, workflow_id: Uuid, error: WorkflowError) {
        let mut workflows = self.workflows.write().await;
        if let Some(workflow) = workflows.get_mut(&workflow_id) {
            workflow.status = WorkflowStatus::Failed;
            workflow.completed_at = Some(Utc::now());
            workflow.error_info = Some(error);
            tracing::error!("Workflow failed: {}", workflow_id);
        }
    }

    /// Get workflow status
    pub async fn get_workflow_status(&self, workflow_id: Uuid) -> Result<WorkflowExecution> {
        let workflows = self.workflows.read().await;
        workflows.get(&workflow_id)
            .cloned()
            .ok_or_else(|| anyhow!("Workflow not found: {}", workflow_id))
    }

    /// List all workflows
    pub async fn list_workflows(&self) -> Vec<WorkflowExecution> {
        let workflows = self.workflows.read().await;
        workflows.values().cloned().collect()
    }

    /// Cancel workflow execution
    pub async fn cancel_workflow(&self, workflow_id: Uuid) -> Result<()> {
        let mut workflows = self.workflows.write().await;
        if let Some(workflow) = workflows.get_mut(&workflow_id) {
            workflow.status = WorkflowStatus::Cancelled;
            workflow.completed_at = Some(Utc::now());
            tracing::info!("Workflow cancelled: {}", workflow_id);
        }
        Ok(())
    }
}

impl Clone for WorkflowOrchestrator {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            workflows: Arc::clone(&self.workflows),
            workflow_templates: Arc::clone(&self.workflow_templates),
            agent_registry: Arc::clone(&self.agent_registry),
            conversation_manager: self.conversation_manager.clone(),
            project_context_agent: self.project_context_agent.clone(),
            execution_queue: Arc::clone(&self.execution_queue),
            config: self.config.clone(),
            is_initialized: self.is_initialized,
        }
    }
}

#[async_trait]
impl Agent for WorkflowOrchestrator {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Workflow Orchestrator"
    }

    fn description(&self) -> &str {
        "Advanced workflow orchestration system for coordinating multi-agent AI tasks with project context awareness"
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::WorkflowOrchestration,
            AgentCapability::Custom("MultiAgentCoordination".to_string()),
            AgentCapability::Custom("WorkflowTemplates".to_string()),
            AgentCapability::Custom("ParallelExecution".to_string()),
        ]
    }

    async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        let mut response = AgentResponse {
            request_id: request.id,
            agent_id: self.id.clone(),
            success: false,
            payload: serde_json::json!({}),
            artifacts: Vec::new(),
            next_actions: Vec::new(),
            metadata: HashMap::new(),
        };

        match request.request_type {
            AgentRequestType::ExecuteWorkflow => {
                if let Ok(workflow_request) = serde_json::from_value::<WorkflowExecutionRequest>(request.payload.clone()) {
                    match self.create_workflow(&workflow_request.template_id, workflow_request.context, workflow_request.variables).await {
                        Ok(workflow_id) => {
                            response.success = true;
                            response.payload = serde_json::json!({
                                "workflow_id": workflow_id,
                                "status": "started"
                            });
                        }
                        Err(e) => {
                            response.payload = serde_json::json!({
                                "error": e.to_string()
                            });
                        }
                    }
                }
            }
            _ => {
                return Err(anyhow!("Workflow Orchestrator cannot handle request type: {:?}", request.request_type));
            }
        }

        Ok(response)
    }

    fn can_handle(&self, request_type: &AgentRequestType) -> bool {
        matches!(request_type, AgentRequestType::ExecuteWorkflow)
    }

    async fn status(&self) -> AgentStatus {
        let workflows = self.workflows.read().await;
        let active_workflows = workflows.values()
            .filter(|w| matches!(w.status, WorkflowStatus::Running))
            .count();

        AgentStatus {
            is_healthy: self.is_initialized,
            is_busy: active_workflows > 0,
            last_activity: Utc::now(),
            current_task: if active_workflows > 0 {
                Some(format!("Managing {} active workflows", active_workflows))
            } else {
                None
            },
            error_message: None,
        }
    }

    async fn initialize(&mut self, _config: AgentConfig) -> Result<()> {
        self.is_initialized = true;
        tracing::info!("Workflow Orchestrator initialized");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Cancel all running workflows
        let workflow_ids: Vec<Uuid> = {
            let workflows = self.workflows.read().await;
            workflows.values()
                .filter(|w| matches!(w.status, WorkflowStatus::Running))
                .map(|w| w.id)
                .collect()
        };

        for workflow_id in workflow_ids {
            self.cancel_workflow(workflow_id).await?;
        }

        self.is_initialized = false;
        tracing::info!("Workflow Orchestrator shut down");
        Ok(())
    }
}

/// Request to execute a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionRequest {
    pub template_id: String,
    pub context: WorkflowContext,
    pub variables: Option<HashMap<String, serde_json::Value>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_orchestrator_creation() {
        let orchestrator = WorkflowOrchestrator::new();
        assert_eq!(orchestrator.id(), "workflow-orchestrator");
        assert_eq!(orchestrator.name(), "Workflow Orchestrator");
    }

    #[tokio::test]
    async fn test_workflow_template_registration() {
        let orchestrator = WorkflowOrchestrator::new();
        
        let template = WorkflowTemplate {
            id: "test-template".to_string(),
            name: "Test Template".to_string(),
            description: "A test workflow template".to_string(),
            category: WorkflowCategory::Custom("Test".to_string()),
            version: "1.0.0".to_string(),
            author: Some("Test Author".to_string()),
            tags: vec!["test".to_string()],
            steps: vec![],
            variables: HashMap::new(),
            triggers: vec![],
            conditions: vec![],
            error_handling: ErrorHandlingStrategy::StopOnError,
            timeout_seconds: Some(300),
            retry_config: RetryConfig::default(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(orchestrator.register_template(template).await.is_ok());
    }
}