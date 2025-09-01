// Workflow Engine - Execute YAML workflow definitions with full lifecycle management

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc, RwLock};
use std::sync::Arc;
use tera::{Context, Tera};
use regex::Regex;
use chrono::{DateTime, Utc};
use anyhow::{Result, anyhow};

pub mod parser;
pub mod validator;
pub mod executor;

use parser::WorkflowParser;
use validator::WorkflowValidator;

/// Main workflow definition structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub metadata: WorkflowMetadata,
    pub requirements: Vec<Requirement>,
    pub parameters: Vec<Parameter>,
    pub environment: HashMap<String, String>,
    pub steps: Vec<WorkflowStep>,
    pub hooks: WorkflowHooks,
    pub outputs: Vec<Output>,
    pub ai_context: Option<AiContext>,
}

/// Workflow metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetadata {
    pub tags: Vec<String>,
    pub icon: Option<String>,
    pub estimated_duration: Option<String>,
}

/// System requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_var: Option<String>,
    pub required: bool,
}

/// Workflow parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: ParameterType,
    pub description: String,
    #[serde(default)]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<ParameterOption>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    String,
    Number,
    Boolean,
    Choice,
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterOption {
    pub value: String,
    pub label: String,
}

/// Workflow step definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub commands: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(default)]
    pub continue_on_error: bool,
    #[serde(default)]
    pub allow_failure: bool,
    #[serde(default)]
    pub always_run: bool,
    #[serde(default)]
    pub parallel: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Vec<Secret>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<Vec<Artifact>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub attempts: u32,
    pub delay: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backoff: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Secret {
    pub name: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub path: String,
    pub name: String,
}

/// Workflow hooks
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowHooks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_workflow: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_workflow: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_success: Option<Vec<String>>,
}

/// Workflow output definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    pub name: String,
    pub value: String,
    pub description: String,
}

/// AI context hints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiContext {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub common_issues: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_suggestions: Option<HashMap<String, String>>,
}

/// Workflow execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub workflow_id: String,
    pub workflow_name: String,
    pub status: WorkflowStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub current_step: Option<String>,
    pub completed_steps: Vec<String>,
    pub failed_steps: Vec<String>,
    pub parameters: HashMap<String, serde_json::Value>,
    pub outputs: HashMap<String, String>,
    pub artifacts: Vec<PathBuf>,
    pub logs: Vec<LogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub step_id: Option<String>,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// Main workflow engine
pub struct WorkflowEngine {
    workflows: Arc<RwLock<HashMap<String, WorkflowDefinition>>>,
    states: Arc<RwLock<HashMap<String, WorkflowState>>>,
    event_sender: broadcast::Sender<WorkflowEvent>,
    template_engine: Tera,
}

impl WorkflowEngine {
    /// Create a new workflow engine
    pub fn new() -> Result<Self> {
        let (event_sender, _) = broadcast::channel(100);
        let mut template_engine = Tera::default();
        
        // Add custom template functions
        template_engine.register_function("env", |args: &HashMap<String, tera::Value>| {
            if let Some(var) = args.get("var").and_then(|v| v.as_str()) {
                Ok(tera::Value::String(std::env::var(var).unwrap_or_default()))
            } else {
                Ok(tera::Value::Null)
            }
        });
        
        Ok(Self {
            workflows: Arc::new(RwLock::new(HashMap::new())),
            states: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            template_engine,
        })
    }
    
    /// Load a workflow from YAML file
    pub async fn load_workflow(&self, path: &Path) -> Result<String> {
        let content = tokio::fs::read_to_string(path).await?;
        let workflow: WorkflowDefinition = serde_yaml::from_str(&content)?;
        
        // Validate workflow
        let validator = WorkflowValidator::new();
        validator.validate(&workflow)?;
        
        let workflow_id = format!("{}-{}", workflow.name, workflow.version);
        self.workflows.write().await.insert(workflow_id.clone(), workflow);
        
        Ok(workflow_id)
    }
    
    /// Execute a workflow
    pub async fn execute_workflow(
        &self,
        workflow_id: &str,
        parameters: HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        let workflows = self.workflows.read().await;
        let workflow = workflows.get(workflow_id)
            .ok_or_else(|| anyhow!("Workflow not found: {}", workflow_id))?
            .clone();
        drop(workflows);
        
        // Validate parameters
        self.validate_parameters(&workflow, &parameters)?;
        
        // Create execution state
        let execution_id = uuid::Uuid::new_v4().to_string();
        let state = WorkflowState {
            workflow_id: workflow_id.to_string(),
            workflow_name: workflow.name.clone(),
            status: WorkflowStatus::Pending,
            started_at: Utc::now(),
            finished_at: None,
            current_step: None,
            completed_steps: Vec::new(),
            failed_steps: Vec::new(),
            parameters: parameters.clone(),
            outputs: HashMap::new(),
            artifacts: Vec::new(),
            logs: Vec::new(),
        };
        
        self.states.write().await.insert(execution_id.clone(), state);
        
        // Spawn execution task
        let engine = self.clone();
        let exec_id = execution_id.clone();
        let wf = workflow.clone();
        let params = parameters.clone();
        
        tokio::spawn(async move {
            let _ = engine.run_workflow(exec_id, wf, params).await;
        });
        
        Ok(execution_id)
    }
    
    /// Run a workflow execution
    async fn run_workflow(
        &self,
        execution_id: String,
        workflow: WorkflowDefinition,
        parameters: HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        // Update status to running
        self.update_status(&execution_id, WorkflowStatus::Running).await?;
        self.log(&execution_id, None, LogLevel::Info, 
                 format!("Starting workflow: {}", workflow.name)).await;
        
        // Check requirements
        if let Err(e) = self.check_requirements(&workflow.requirements).await {
            self.log(&execution_id, None, LogLevel::Error, 
                     format!("Requirements check failed: {}", e)).await;
            self.update_status(&execution_id, WorkflowStatus::Failed).await?;
            return Err(e);
        }
        
        // Prepare execution context
        let mut context = Context::new();
        for (key, value) in &parameters {
            context.insert(key, value);
        }
        
        // Add workflow variables
        context.insert("WORKFLOW_ID", &execution_id);
        context.insert("WORKFLOW_NAME", &workflow.name);
        context.insert("WORKFLOW_VERSION", &workflow.version);
        
        // Execute pre-workflow hooks
        if let Some(pre_hooks) = &workflow.hooks.pre_workflow {
            for command in pre_hooks {
                let _ = self.execute_command(&execution_id, command, &context, &workflow.environment).await;
            }
        }
        
        // Execute steps
        let mut overall_success = true;
        for step in &workflow.steps {
            // Check condition
            if let Some(condition) = &step.condition {
                if !self.evaluate_condition(condition, &context).await? {
                    self.log(&execution_id, Some(&step.id), LogLevel::Info, 
                             "Skipping step due to condition".to_string()).await;
                    continue;
                }
            }
            
            // Update current step
            self.update_current_step(&execution_id, Some(&step.id)).await?;
            self.log(&execution_id, Some(&step.id), LogLevel::Info, 
                     format!("Executing step: {}", step.name)).await;
            
            // Execute step with retry logic
            let mut attempts = 0;
            let max_attempts = step.retry.as_ref().map(|r| r.attempts).unwrap_or(1);
            let mut step_success = false;
            
            while attempts < max_attempts && !step_success {
                attempts += 1;
                
                if attempts > 1 {
                    // Apply retry delay
                    if let Some(retry) = &step.retry {
                        let delay = parse_duration(&retry.delay)?;
                        tokio::time::sleep(delay).await;
                    }
                }
                
                // Execute commands
                let mut command_failed = false;
                for command in &step.commands {
                    let rendered_command = self.render_template(command, &context)?;
                    
                    let result = self.execute_command(
                        &execution_id,
                        &rendered_command,
                        &context,
                        &step.environment.as_ref().unwrap_or(&workflow.environment)
                    ).await;
                    
                    if let Err(e) = result {
                        self.log(&execution_id, Some(&step.id), LogLevel::Error, 
                                 format!("Command failed: {}", e)).await;
                        command_failed = true;
                        
                        if !step.continue_on_error {
                            break;
                        }
                    }
                }
                
                step_success = !command_failed;
            }
            
            // Handle step result
            if step_success {
                self.mark_step_completed(&execution_id, &step.id).await?;
            } else {
                self.mark_step_failed(&execution_id, &step.id).await?;
                
                if !step.allow_failure && !step.always_run {
                    overall_success = false;
                    break;
                }
            }
        }
        
        // Execute post-workflow hooks
        if overall_success {
            if let Some(success_hooks) = &workflow.hooks.on_success {
                for command in success_hooks {
                    let _ = self.execute_command(&execution_id, command, &context, &workflow.environment).await;
                }
            }
        } else {
            if let Some(failure_hooks) = &workflow.hooks.on_failure {
                for command in failure_hooks {
                    let _ = self.execute_command(&execution_id, command, &context, &workflow.environment).await;
                }
            }
        }
        
        if let Some(post_hooks) = &workflow.hooks.post_workflow {
            for command in post_hooks {
                let _ = self.execute_command(&execution_id, command, &context, &workflow.environment).await;
            }
        }
        
        // Process outputs
        for output in &workflow.outputs {
            let value = self.render_template(&output.value, &context)?;
            self.add_output(&execution_id, &output.name, &value).await?;
        }
        
        // Update final status
        let final_status = if overall_success {
            WorkflowStatus::Success
        } else {
            WorkflowStatus::Failed
        };
        
        self.update_status(&execution_id, final_status.clone()).await?;
        self.set_finished_time(&execution_id).await?;
        
        self.log(&execution_id, None, LogLevel::Info, 
                 format!("Workflow completed: {:?}", final_status)).await;
        
        Ok(())
    }
    
    /// Validate workflow parameters
    fn validate_parameters(
        &self,
        workflow: &WorkflowDefinition,
        provided: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        for param in &workflow.parameters {
            if param.required && !provided.contains_key(&param.name) && param.default.is_none() {
                return Err(anyhow!("Required parameter missing: {}", param.name));
            }
            
            if let Some(value) = provided.get(&param.name) {
                // Type validation
                match param.param_type {
                    ParameterType::String => {
                        if !value.is_string() {
                            return Err(anyhow!("Parameter {} must be a string", param.name));
                        }
                        
                        // Regex validation
                        if let Some(pattern) = &param.validation {
                            let regex = Regex::new(pattern)?;
                            if let Some(str_val) = value.as_str() {
                                if !regex.is_match(str_val) {
                                    return Err(anyhow!("Parameter {} does not match pattern: {}", 
                                                       param.name, pattern));
                                }
                            }
                        }
                    }
                    ParameterType::Number => {
                        if !value.is_number() {
                            return Err(anyhow!("Parameter {} must be a number", param.name));
                        }
                        
                        if let Some(num) = value.as_f64() {
                            if let Some(min) = param.min {
                                if num < min {
                                    return Err(anyhow!("Parameter {} must be >= {}", param.name, min));
                                }
                            }
                            if let Some(max) = param.max {
                                if num > max {
                                    return Err(anyhow!("Parameter {} must be <= {}", param.name, max));
                                }
                            }
                        }
                    }
                    ParameterType::Boolean => {
                        if !value.is_boolean() {
                            return Err(anyhow!("Parameter {} must be a boolean", param.name));
                        }
                    }
                    ParameterType::Choice => {
                        if let Some(options) = &param.options {
                            let valid_values: Vec<String> = options.iter()
                                .map(|o| o.value.clone())
                                .collect();
                            
                            if let Some(str_val) = value.as_str() {
                                if !valid_values.contains(&str_val.to_string()) {
                                    return Err(anyhow!("Parameter {} must be one of: {:?}", 
                                                       param.name, valid_values));
                                }
                            }
                        }
                    }
                    ParameterType::File | ParameterType::Directory => {
                        if let Some(path_str) = value.as_str() {
                            let path = Path::new(path_str);
                            if !path.exists() {
                                return Err(anyhow!("Path does not exist: {}", path_str));
                            }
                            
                            match param.param_type {
                                ParameterType::File if !path.is_file() => {
                                    return Err(anyhow!("Path is not a file: {}", path_str));
                                }
                                ParameterType::Directory if !path.is_dir() => {
                                    return Err(anyhow!("Path is not a directory: {}", path_str));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Check system requirements
    async fn check_requirements(&self, requirements: &[Requirement]) -> Result<()> {
        for req in requirements {
            if let Some(command) = &req.command {
                // Check if command exists
                let output = Command::new("which")
                    .arg(command)
                    .output()
                    .await?;
                
                if !output.status.success() && req.required {
                    return Err(anyhow!("Required command not found: {}", command));
                }
                
                // Check version if specified
                if let Some(min_version) = &req.min_version {
                    // This is simplified - in production you'd parse actual version output
                    let version_output = Command::new(command)
                        .arg("--version")
                        .output()
                        .await?;
                    
                    let version_str = String::from_utf8_lossy(&version_output.stdout);
                    // Version comparison logic would go here
                }
            }
            
            if let Some(env_var) = &req.env_var {
                if std::env::var(env_var).is_err() && req.required {
                    return Err(anyhow!("Required environment variable not set: {}", env_var));
                }
            }
        }
        
        Ok(())
    }
    
    /// Execute a shell command
    async fn execute_command(
        &self,
        execution_id: &str,
        command: &str,
        context: &Context,
        environment: &HashMap<String, String>,
    ) -> Result<()> {
        let rendered_command = self.render_template(command, context)?;
        
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&rendered_command);
        
        // Set environment variables
        for (key, value) in environment {
            let rendered_value = self.render_template(value, context)?;
            cmd.env(key, rendered_value);
        }
        
        // Set workflow environment variables
        cmd.env("WORKFLOW_ID", execution_id);
        cmd.env("WORKFLOW_STATUS", "running");
        
        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Command failed: {}", stderr));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.is_empty() {
            self.log(execution_id, None, LogLevel::Info, stdout.to_string()).await;
        }
        
        Ok(())
    }
    
    /// Evaluate a condition expression
    async fn evaluate_condition(&self, condition: &str, context: &Context) -> Result<bool> {
        let rendered = self.render_template(condition, context)?;
        
        // Simple evaluation - in production you'd use a proper expression evaluator
        Ok(rendered == "true" || rendered == "1")
    }
    
    /// Render a template string
    fn render_template(&self, template: &str, context: &Context) -> Result<String> {
        // Replace simple placeholders
        let mut result = template.to_string();
        
        // Use regex to find {{variable}} patterns
        let re = Regex::new(r"\{\{(\w+)\}\}")?;
        for cap in re.captures_iter(template) {
            if let Some(var_name) = cap.get(1) {
                if let Some(value) = context.get(var_name.as_str()) {
                    let str_value = match value {
                        tera::Value::String(s) => s.clone(),
                        tera::Value::Number(n) => n.to_string(),
                        tera::Value::Bool(b) => b.to_string(),
                        _ => String::new(),
                    };
                    result = result.replace(&cap[0], &str_value);
                }
            }
        }
        
        Ok(result)
    }
    
    /// Update workflow status
    async fn update_status(&self, execution_id: &str, status: WorkflowStatus) -> Result<()> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(execution_id) {
            state.status = status;
        }
        Ok(())
    }
    
    /// Update current step
    async fn update_current_step(&self, execution_id: &str, step_id: Option<&str>) -> Result<()> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(execution_id) {
            state.current_step = step_id.map(String::from);
        }
        Ok(())
    }
    
    /// Mark step as completed
    async fn mark_step_completed(&self, execution_id: &str, step_id: &str) -> Result<()> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(execution_id) {
            state.completed_steps.push(step_id.to_string());
        }
        Ok(())
    }
    
    /// Mark step as failed
    async fn mark_step_failed(&self, execution_id: &str, step_id: &str) -> Result<()> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(execution_id) {
            state.failed_steps.push(step_id.to_string());
        }
        Ok(())
    }
    
    /// Add output value
    async fn add_output(&self, execution_id: &str, name: &str, value: &str) -> Result<()> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(execution_id) {
            state.outputs.insert(name.to_string(), value.to_string());
        }
        Ok(())
    }
    
    /// Set finished time
    async fn set_finished_time(&self, execution_id: &str) -> Result<()> {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(execution_id) {
            state.finished_at = Some(Utc::now());
        }
        Ok(())
    }
    
    /// Add log entry
    async fn log(&self, execution_id: &str, step_id: Option<&str>, level: LogLevel, message: String) {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(execution_id) {
            state.logs.push(LogEntry {
                timestamp: Utc::now(),
                step_id: step_id.map(String::from),
                level,
                message: message.clone(),
            });
        }
        
        // Emit event
        let _ = self.event_sender.send(WorkflowEvent::Log {
            execution_id: execution_id.to_string(),
            step_id: step_id.map(String::from),
            message,
        });
    }
    
    /// Get workflow state
    pub async fn get_state(&self, execution_id: &str) -> Option<WorkflowState> {
        self.states.read().await.get(execution_id).cloned()
    }
    
    /// Cancel workflow execution
    pub async fn cancel_workflow(&self, execution_id: &str) -> Result<()> {
        self.update_status(execution_id, WorkflowStatus::Cancelled).await?;
        self.set_finished_time(execution_id).await?;
        Ok(())
    }
    
    /// Subscribe to workflow events
    pub fn subscribe(&self) -> broadcast::Receiver<WorkflowEvent> {
        self.event_sender.subscribe()
    }
}

impl Clone for WorkflowEngine {
    fn clone(&self) -> Self {
        Self {
            workflows: self.workflows.clone(),
            states: self.states.clone(),
            event_sender: self.event_sender.clone(),
            template_engine: Tera::default(),
        }
    }
}

/// Workflow events
#[derive(Debug, Clone)]
pub enum WorkflowEvent {
    Started { execution_id: String },
    StepStarted { execution_id: String, step_id: String },
    StepCompleted { execution_id: String, step_id: String },
    StepFailed { execution_id: String, step_id: String },
    Completed { execution_id: String, status: WorkflowStatus },
    Log { execution_id: String, step_id: Option<String>, message: String },
}

/// Parse duration string (e.g., "5s", "10m", "1h")
fn parse_duration(s: &str) -> Result<std::time::Duration> {
    let re = Regex::new(r"^(\d+)([smh])$")?;
    if let Some(caps) = re.captures(s) {
        let value: u64 = caps[1].parse()?;
        let unit = &caps[2];
        
        let duration = match unit {
            "s" => std::time::Duration::from_secs(value),
            "m" => std::time::Duration::from_secs(value * 60),
            "h" => std::time::Duration::from_secs(value * 3600),
            _ => return Err(anyhow!("Invalid duration unit: {}", unit)),
        };
        
        Ok(duration)
    } else {
        Err(anyhow!("Invalid duration format: {}", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_workflow_execution() {
        let engine = WorkflowEngine::new().unwrap();
        
        // Create a simple test workflow
        let workflow = WorkflowDefinition {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "Test workflow".to_string(),
            author: None,
            metadata: WorkflowMetadata {
                tags: vec![],
                icon: None,
                estimated_duration: None,
            },
            requirements: vec![],
            parameters: vec![],
            environment: HashMap::new(),
            steps: vec![
                WorkflowStep {
                    id: "step1".to_string(),
                    name: "Echo Test".to_string(),
                    description: None,
                    commands: vec!["echo 'Hello, World!'".to_string()],
                    condition: None,
                    continue_on_error: false,
                    allow_failure: false,
                    always_run: false,
                    parallel: false,
                    timeout: None,
                    retry: None,
                    environment: None,
                    secrets: None,
                    artifacts: None,
                }
            ],
            hooks: WorkflowHooks::default(),
            outputs: vec![],
            ai_context: None,
        };
        
        let workflow_id = "test-1.0.0".to_string();
        engine.workflows.write().await.insert(workflow_id.clone(), workflow);
        
        // Execute workflow
        let execution_id = engine.execute_workflow(&workflow_id, HashMap::new()).await.unwrap();
        
        // Wait for completion
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        
        // Check state
        let state = engine.get_state(&execution_id).await.unwrap();
        assert_eq!(state.status, WorkflowStatus::Success);
        assert!(state.completed_steps.contains(&"step1".to_string()));
    }
}
