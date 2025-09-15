// Example: Workflow Orchestrator Demo
// Demonstrates advanced workflow orchestration with multi-agent coordination,
// project context awareness, and conversation integration

use openagent_terminal::ai::agents::*;
use openagent_terminal::ai::agents::workflow_orchestrator::*;
use openagent_terminal::ai::agents::blitzy_project_context::*;
use openagent_terminal::ai::agents::conversation_manager::*;
use openagent_terminal::ai::agents::natural_language::*;
use openagent_terminal::ai::agents::code_generation::*;

use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    println!("🚀 Workflow Orchestrator Demo");
    println!("===============================");

    // Initialize core agents
    let mut conversation_manager = ConversationManager::new();
    conversation_manager.initialize(AgentConfig::default()).await?;
    let conversation_manager = Arc::new(conversation_manager);

    let project_context_agent = BlitzyProjectContextAgent::new()
        .with_conversation_manager(Arc::clone(&conversation_manager));
    let mut project_context_agent = project_context_agent;
    project_context_agent.initialize(AgentConfig::default()).await?;
    let project_context_agent = Arc::new(project_context_agent);

    let code_gen_agent = CodeGenerationAgent::new();
    let mut code_gen_agent = code_gen_agent;
    code_gen_agent.initialize(AgentConfig::default()).await?;
    let code_gen_agent = Arc::new(code_gen_agent);

    // Initialize workflow orchestrator
    let workflow_orchestrator = WorkflowOrchestrator::new()
        .with_conversation_manager(Arc::clone(&conversation_manager))
        .with_project_context_agent(Arc::clone(&project_context_agent));
    
    let mut workflow_orchestrator = workflow_orchestrator;
    workflow_orchestrator.initialize(AgentConfig::default()).await?;

    // Register agents with the orchestrator
    println!("\n📋 Registering Agents...");
    workflow_orchestrator.register_agent(Arc::clone(&project_context_agent) as Arc<dyn Agent>).await?;
    workflow_orchestrator.register_agent(Arc::clone(&code_gen_agent) as Arc<dyn Agent>).await?;
    println!("✅ Registered project context and code generation agents");

    // Create a conversation session
    let session_id = conversation_manager.create_session(Some("Workflow Demo Session".to_string())).await?;
    conversation_manager.add_turn(
        session_id,
        ConversationRole::User,
        "I want to analyze my project and then generate some code".to_string(),
        None,
        Vec::new(),
    ).await?;

    // Define a comprehensive workflow template
    println!("\n🛠️  Creating Workflow Template...");
    let workflow_template = create_project_analysis_workflow_template().await?;
    workflow_orchestrator.register_template(workflow_template).await?;
    println!("✅ Registered 'project-analysis-and-codegen' workflow template");

    // Create workflow context
    let workflow_context = WorkflowContext {
        conversation_session_id: Some(session_id),
        project_root: Some(".".to_string()),
        user_id: Some("demo-user".to_string()),
        environment: HashMap::from([
            ("RUST_LOG".to_string(), "info".to_string()),
            ("PROJECT_TYPE".to_string(), "rust".to_string()),
        ]),
        variables: HashMap::from([
            ("project_path".to_string(), serde_json::Value::String(".".to_string())),
            ("analysis_depth".to_string(), serde_json::Value::String("comprehensive".to_string())),
            ("generate_docs".to_string(), serde_json::Value::Bool(true)),
        ]),
        shared_state: HashMap::new(),
    };

    // Execute the workflow
    println!("\n🚀 Starting Workflow Execution...");
    let workflow_id = workflow_orchestrator.create_workflow(
        "project-analysis-and-codegen",
        workflow_context,
        None,
    ).await?;

    println!("✅ Workflow started with ID: {}", workflow_id);

    // Monitor workflow progress
    println!("\n📊 Monitoring Workflow Progress...");
    monitor_workflow_progress(&workflow_orchestrator, workflow_id).await?;

    // Display workflow results
    println!("\n📋 Workflow Execution Summary:");
    display_workflow_results(&workflow_orchestrator, workflow_id).await?;

    // Demonstrate workflow template variations
    println!("\n🔄 Creating Additional Workflow Templates...");
    demonstrate_workflow_templates(&workflow_orchestrator).await?;

    // Show orchestrator status
    println!("\n📈 Orchestrator Status:");
    let status = workflow_orchestrator.status().await;
    println!("  • Healthy: {}", status.is_healthy);
    println!("  • Busy: {}", status.is_busy);
    if let Some(task) = &status.current_task {
        println!("  • Current Task: {}", task);
    }

    // List all workflows
    let workflows = workflow_orchestrator.list_workflows().await;
    println!("  • Total Workflows: {}", workflows.len());
    for workflow in &workflows {
        println!("    - {}: {} ({})", workflow.id, workflow.title, format!("{:?}", workflow.status));
    }

    // Cleanup
    println!("\n🧹 Shutting Down...");
    workflow_orchestrator.shutdown().await?;
    println!("✅ Demo completed successfully!");

    Ok(())
}

async fn create_project_analysis_workflow_template() -> anyhow::Result<WorkflowTemplate> {
    let template = WorkflowTemplate {
        id: "project-analysis-and-codegen".to_string(),
        name: "Project Analysis and Code Generation".to_string(),
        description: "Comprehensive workflow that analyzes project structure and generates code".to_string(),
        category: WorkflowCategory::Analysis,
        version: "1.0.0".to_string(),
        author: Some("OpenAgent Terminal".to_string()),
        tags: vec![
            "analysis".to_string(),
            "codegen".to_string(),
            "project".to_string(),
        ],
        steps: vec![
            // Step 1: Analyze project structure
            WorkflowStep {
                id: "analyze-project".to_string(),
                name: "Analyze Project Structure".to_string(),
                step_type: WorkflowStepType::AgentRequest,
                agent_id: Some("blitzy-project-context".to_string()),
                request_template: serde_json::json!({
                    "id": "{{request_id}}",
                    "request_type": "ManageProject",
                    "payload": {
                        "path": "{{project_path}}",
                        "include_git": true,
                        "include_dependencies": true,
                        "include_file_summaries": false,
                        "max_files": 50
                    },
                    "context": {
                        "project_root": "{{project_path}}",
                        "current_directory": "{{project_path}}",
                        "current_branch": null,
                        "open_files": [],
                        "recent_commands": [],
                        "environment_vars": {},
                        "user_preferences": {}
                    },
                    "metadata": {}
                }),
                dependencies: vec![],
                conditions: vec![],
                timeout_seconds: Some(120),
                retry_attempts: 2,
                error_handling: StepErrorHandling::Retry,
                input_mapping: HashMap::from([
                    ("request_id".to_string(), "request_id".to_string()),
                    ("project_path".to_string(), "project_path".to_string()),
                ]),
                output_mapping: HashMap::from([
                    ("success".to_string(), "analysis_success".to_string()),
                    ("payload".to_string(), "project_analysis".to_string()),
                ]),
                parallel_group: None,
            },
            // Step 2: Generate code based on analysis
            WorkflowStep {
                id: "generate-code".to_string(),
                name: "Generate Code Documentation".to_string(),
                step_type: WorkflowStepType::AgentRequest,
                agent_id: Some("code-generation-agent".to_string()),
                request_template: serde_json::json!({
                    "id": "{{request_id}}",
                    "request_type": "GenerateCode",
                    "payload": {
                        "language": "rust",
                        "code_type": "documentation",
                        "context": "{{project_analysis}}",
                        "requirements": [
                            "Generate README.md improvements",
                            "Add inline documentation",
                            "Create usage examples"
                        ],
                        "style": "professional"
                    },
                    "context": {
                        "project_root": "{{project_path}}",
                        "current_directory": "{{project_path}}",
                        "current_branch": null,
                        "open_files": [],
                        "recent_commands": [],
                        "environment_vars": {},
                        "user_preferences": {}
                    },
                    "metadata": {}
                }),
                dependencies: vec!["analyze-project".to_string()],
                conditions: vec![
                    StepCondition {
                        condition_type: ConditionType::Variable,
                        expression: "analysis_success == true".to_string(),
                        skip_on_false: true,
                    }
                ],
                timeout_seconds: Some(180),
                retry_attempts: 2,
                error_handling: StepErrorHandling::Skip,
                input_mapping: HashMap::from([
                    ("request_id".to_string(), "request_id".to_string()),
                    ("project_path".to_string(), "project_path".to_string()),
                    ("project_analysis".to_string(), "project_analysis".to_string()),
                ]),
                output_mapping: HashMap::from([
                    ("success".to_string(), "codegen_success".to_string()),
                    ("payload".to_string(), "generated_code".to_string()),
                ]),
                parallel_group: None,
            },
            // Step 3: Update conversation with results
            WorkflowStep {
                id: "update-conversation".to_string(),
                name: "Update Conversation Context".to_string(),
                step_type: WorkflowStepType::Custom("ConversationUpdate".to_string()),
                agent_id: None,
                request_template: serde_json::json!({
                    "session_id": "{{conversation_session_id}}",
                    "analysis_results": "{{project_analysis}}",
                    "generated_code": "{{generated_code}}"
                }),
                dependencies: vec!["analyze-project".to_string(), "generate-code".to_string()],
                conditions: vec![],
                timeout_seconds: Some(30),
                retry_attempts: 1,
                error_handling: StepErrorHandling::Skip,
                input_mapping: HashMap::from([
                    ("conversation_session_id".to_string(), "conversation_session_id".to_string()),
                    ("project_analysis".to_string(), "project_analysis".to_string()),
                    ("generated_code".to_string(), "generated_code".to_string()),
                ]),
                output_mapping: HashMap::from([
                    ("success".to_string(), "conversation_updated".to_string()),
                ]),
                parallel_group: None,
            },
        ],
        variables: HashMap::from([
            ("project_path".to_string(), WorkflowVariable {
                name: "project_path".to_string(),
                variable_type: VariableType::DirectoryPath,
                description: "Path to the project to analyze".to_string(),
                default_value: Some(serde_json::Value::String(".".to_string())),
                required: true,
                validation: None,
            }),
            ("analysis_depth".to_string(), WorkflowVariable {
                name: "analysis_depth".to_string(),
                variable_type: VariableType::String,
                description: "Depth of project analysis".to_string(),
                default_value: Some(serde_json::Value::String("standard".to_string())),
                required: false,
                validation: Some(VariableValidation {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    allowed_values: Some(vec![
                        "basic".to_string(),
                        "standard".to_string(),
                        "comprehensive".to_string(),
                    ]),
                }),
            }),
        ]),
        triggers: vec![
            WorkflowTrigger {
                trigger_type: TriggerType::Manual,
                conditions: HashMap::new(),
                enabled: true,
            }
        ],
        conditions: vec![],
        error_handling: ErrorHandlingStrategy::StopOnError,
        timeout_seconds: Some(600), // 10 minutes
        retry_config: RetryConfig::default(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    Ok(template)
}

async fn monitor_workflow_progress(
    orchestrator: &WorkflowOrchestrator,
    workflow_id: Uuid,
) -> anyhow::Result<()> {
    let mut last_status = WorkflowStatus::Created;
    let mut completed_steps = 0;
    
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        let workflow = match orchestrator.get_workflow_status(workflow_id).await {
            Ok(w) => w,
            Err(_) => break,
        };
        
        // Check for status changes
        if workflow.status != last_status {
            println!("  📍 Workflow status: {:?} -> {:?}", last_status, workflow.status);
            last_status = workflow.status.clone();
            
            match workflow.status {
                WorkflowStatus::Completed => {
                    println!("  ✅ Workflow completed successfully!");
                    break;
                }
                WorkflowStatus::Failed => {
                    println!("  ❌ Workflow failed!");
                    if let Some(error) = &workflow.error_info {
                        println!("     Error: {}", error.message);
                    }
                    break;
                }
                WorkflowStatus::Cancelled => {
                    println!("  ⏹️  Workflow cancelled!");
                    break;
                }
                _ => {}
            }
        }
        
        // Check for step progress
        let current_completed = workflow.steps.iter()
            .filter(|s| matches!(s.status, StepExecutionStatus::Completed))
            .count();
        
        if current_completed > completed_steps {
            let newly_completed: Vec<_> = workflow.steps.iter()
                .filter(|s| matches!(s.status, StepExecutionStatus::Completed))
                .skip(completed_steps)
                .collect();
            
            for step in newly_completed {
                println!("  ✅ Step completed: {}", step.step_id);
            }
            
            completed_steps = current_completed;
        }
        
        // Check for running steps
        for step in &workflow.steps {
            if matches!(step.status, StepExecutionStatus::Running) {
                println!("  🔄 Running step: {}", step.step_id);
            }
        }
        
        // Break if workflow is no longer active
        if matches!(workflow.status, 
            WorkflowStatus::Completed | 
            WorkflowStatus::Failed | 
            WorkflowStatus::Cancelled
        ) {
            break;
        }
    }
    
    Ok(())
}

async fn display_workflow_results(
    orchestrator: &WorkflowOrchestrator,
    workflow_id: Uuid,
) -> anyhow::Result<()> {
    let workflow = orchestrator.get_workflow_status(workflow_id).await?;
    
    println!("═══════════════════════════════");
    println!("📊 Workflow: {}", workflow.title);
    println!("🆔 ID: {}", workflow.id);
    println!("📝 Description: {}", workflow.description);
    println!("⏱️  Created: {}", workflow.created_at.format("%Y-%m-%d %H:%M:%S"));
    if let Some(started) = workflow.started_at {
        println!("🚀 Started: {}", started.format("%Y-%m-%d %H:%M:%S"));
    }
    if let Some(completed) = workflow.completed_at {
        println!("✅ Completed: {}", completed.format("%Y-%m-%d %H:%M:%S"));
        let duration = completed - workflow.started_at.unwrap_or(workflow.created_at);
        println!("⏱️  Duration: {:.2}s", duration.num_milliseconds() as f64 / 1000.0);
    }
    println!("📊 Status: {:?}", workflow.status);
    
    println!("\n📋 Steps Summary:");
    for (i, step) in workflow.steps.iter().enumerate() {
        let status_emoji = match step.status {
            StepExecutionStatus::Completed => "✅",
            StepExecutionStatus::Failed => "❌",
            StepExecutionStatus::Skipped => "⏭️",
            StepExecutionStatus::Running => "🔄",
            StepExecutionStatus::Pending => "⏳",
            StepExecutionStatus::TimedOut => "⏰",
            StepExecutionStatus::Retrying => "🔁",
        };
        
        println!("  {}. {} {} ({:?})", i + 1, status_emoji, step.step_id, step.status);
        
        if let Some(started) = step.started_at {
            println!("     Started: {}", started.format("%H:%M:%S"));
        }
        if let Some(completed) = step.completed_at {
            println!("     Completed: {}", completed.format("%H:%M:%S"));
            if let Some(started) = step.started_at {
                let duration = completed - started;
                println!("     Duration: {:.2}s", duration.num_milliseconds() as f64 / 1000.0);
            }
        }
        if step.attempts > 1 {
            println!("     Attempts: {}", step.attempts);
        }
        if let Some(error) = &step.error_info {
            println!("     Error: {}", error);
        }
    }
    
    println!("\n🔧 Context Variables:");
    for (key, value) in &workflow.context.variables {
        let value_str = match value {
            serde_json::Value::String(s) => s.clone(),
            _ => value.to_string(),
        };
        let display_value = if value_str.len() > 50 {
            format!("{}...", &value_str[..50])
        } else {
            value_str
        };
        println!("  • {}: {}", key, display_value);
    }
    
    if !workflow.results.is_empty() {
        println!("\n📤 Results:");
        for (key, value) in &workflow.results {
            println!("  • {}: {:?}", key, value);
        }
    }
    
    if let Some(error) = &workflow.error_info {
        println!("\n❌ Error Information:");
        println!("  Type: {:?}", error.error_type);
        println!("  Message: {}", error.message);
        if let Some(step_id) = &error.step_id {
            println!("  Failed Step: {}", step_id);
        }
        println!("  Occurred: {}", error.occurred_at.format("%Y-%m-%d %H:%M:%S"));
        println!("  Recoverable: {}", error.recoverable);
    }
    
    Ok(())
}

async fn demonstrate_workflow_templates(orchestrator: &WorkflowOrchestrator) -> anyhow::Result<()> {
    // Create a simple code generation workflow
    let simple_codegen_template = WorkflowTemplate {
        id: "simple-codegen".to_string(),
        name: "Simple Code Generation".to_string(),
        description: "Generate code based on user requirements".to_string(),
        category: WorkflowCategory::CodeGeneration,
        version: "1.0.0".to_string(),
        author: Some("OpenAgent Terminal".to_string()),
        tags: vec!["codegen".to_string(), "simple".to_string()],
        steps: vec![
            WorkflowStep {
                id: "generate".to_string(),
                name: "Generate Code".to_string(),
                step_type: WorkflowStepType::AgentRequest,
                agent_id: Some("code-generation-agent".to_string()),
                request_template: serde_json::json!({
                    "id": "{{request_id}}",
                    "request_type": "GenerateCode",
                    "payload": {
                        "language": "{{language}}",
                        "code_type": "{{code_type}}",
                        "requirements": "{{requirements}}",
                        "style": "{{style}}"
                    },
                    "context": {
                        "project_root": null,
                        "current_directory": ".",
                        "current_branch": null,
                        "open_files": [],
                        "recent_commands": [],
                        "environment_vars": {},
                        "user_preferences": {}
                    },
                    "metadata": {}
                }),
                dependencies: vec![],
                conditions: vec![],
                timeout_seconds: Some(120),
                retry_attempts: 3,
                error_handling: StepErrorHandling::Retry,
                input_mapping: HashMap::from([
                    ("request_id".to_string(), "request_id".to_string()),
                    ("language".to_string(), "language".to_string()),
                    ("code_type".to_string(), "code_type".to_string()),
                    ("requirements".to_string(), "requirements".to_string()),
                    ("style".to_string(), "style".to_string()),
                ]),
                output_mapping: HashMap::from([
                    ("success".to_string(), "generation_success".to_string()),
                    ("payload".to_string(), "generated_code".to_string()),
                ]),
                parallel_group: None,
            },
        ],
        variables: HashMap::from([
            ("language".to_string(), WorkflowVariable {
                name: "language".to_string(),
                variable_type: VariableType::String,
                description: "Programming language for code generation".to_string(),
                default_value: Some(serde_json::Value::String("rust".to_string())),
                required: true,
                validation: Some(VariableValidation {
                    min_length: None,
                    max_length: None,
                    pattern: None,
                    allowed_values: Some(vec![
                        "rust".to_string(),
                        "javascript".to_string(),
                        "typescript".to_string(),
                        "python".to_string(),
                        "go".to_string(),
                    ]),
                }),
            }),
            ("code_type".to_string(), WorkflowVariable {
                name: "code_type".to_string(),
                variable_type: VariableType::String,
                description: "Type of code to generate".to_string(),
                default_value: Some(serde_json::Value::String("function".to_string())),
                required: false,
                validation: None,
            }),
        ]),
        triggers: vec![
            WorkflowTrigger {
                trigger_type: TriggerType::ConversationIntent,
                conditions: HashMap::from([
                    ("intent".to_string(), serde_json::Value::String("code_generation".to_string())),
                ]),
                enabled: true,
            }
        ],
        conditions: vec![],
        error_handling: ErrorHandlingStrategy::RetryOnError,
        timeout_seconds: Some(300),
        retry_config: RetryConfig::default(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    orchestrator.register_template(simple_codegen_template).await?;
    println!("✅ Registered 'simple-codegen' workflow template");

    // Create a testing workflow template
    let testing_template = WorkflowTemplate {
        id: "project-testing".to_string(),
        name: "Project Testing Workflow".to_string(),
        description: "Comprehensive testing workflow for projects".to_string(),
        category: WorkflowCategory::Testing,
        version: "1.0.0".to_string(),
        author: Some("OpenAgent Terminal".to_string()),
        tags: vec!["testing".to_string(), "quality".to_string()],
        steps: vec![
            WorkflowStep {
                id: "run-tests".to_string(),
                name: "Run Test Suite".to_string(),
                step_type: WorkflowStepType::Command,
                agent_id: None,
                request_template: serde_json::json!({
                    "command": "{{test_command}}",
                    "working_directory": "{{project_path}}"
                }),
                dependencies: vec![],
                conditions: vec![],
                timeout_seconds: Some(300),
                retry_attempts: 2,
                error_handling: StepErrorHandling::Fail,
                input_mapping: HashMap::from([
                    ("test_command".to_string(), "test_command".to_string()),
                    ("project_path".to_string(), "project_path".to_string()),
                ]),
                output_mapping: HashMap::from([
                    ("success".to_string(), "tests_passed".to_string()),
                    ("payload".to_string(), "test_results".to_string()),
                ]),
                parallel_group: None,
            },
        ],
        variables: HashMap::from([
            ("test_command".to_string(), WorkflowVariable {
                name: "test_command".to_string(),
                variable_type: VariableType::String,
                description: "Command to run tests".to_string(),
                default_value: Some(serde_json::Value::String("cargo test".to_string())),
                required: true,
                validation: None,
            }),
        ]),
        triggers: vec![
            WorkflowTrigger {
                trigger_type: TriggerType::GitCommit,
                conditions: HashMap::new(),
                enabled: false, // Disabled for demo
            }
        ],
        conditions: vec![],
        error_handling: ErrorHandlingStrategy::StopOnError,
        timeout_seconds: Some(600),
        retry_config: RetryConfig::default(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    orchestrator.register_template(testing_template).await?;
    println!("✅ Registered 'project-testing' workflow template");

    println!("📚 Available workflow templates:");
    println!("  1. project-analysis-and-codegen - Comprehensive project analysis and code generation");
    println!("  2. simple-codegen - Simple code generation workflow");
    println!("  3. project-testing - Project testing workflow");

    Ok(())
}