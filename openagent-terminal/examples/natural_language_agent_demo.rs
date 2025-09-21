#![allow(clippy::pedantic, clippy::uninlined_format_args, clippy::too_many_lines)]

use anyhow::Result;
use std::collections::HashMap;
use uuid::Uuid;

use openagent_terminal::ai::agents::{
    code_generation::CodeGenerationAgent, communication_hub::AgentCommunicationHub,
    natural_language::NaturalLanguageAgent, security_lens::SecurityLensAgent, Agent, AgentConfig,
    AgentContext, AgentRequest, AgentRequestType,
};

/// Demonstration of the Natural Language Agent system
///
/// This example shows how to:
/// 1. Create and initialize agents
/// 2. Register agents with the communication hub
/// 3. Process natural language inputs
/// 4. Route requests between agents
/// 5. Execute multi-agent workflows
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🤖 OpenAgent Terminal - Natural Language Agent Demo");
    println!("==================================================\n");

    // Create the communication hub
    let mut hub = AgentCommunicationHub::new();
    hub.initialize(AgentConfig::default()).await?;
    println!("✅ Communication Hub initialized");

    // Create and register agents
    let mut nl_agent = NaturalLanguageAgent::new();
    nl_agent.initialize(AgentConfig::default()).await?;
    println!("✅ Natural Language Agent initialized");

    let mut code_agent = CodeGenerationAgent::new();
    code_agent.initialize(AgentConfig::default()).await?;
    println!("✅ Code Generation Agent initialized");

    let mut security_agent = SecurityLensAgent::new();
    security_agent.initialize(AgentConfig::default()).await?;
    println!("✅ Security Lens Agent initialized");

    // Register agents with the hub
    hub.register_agent(Box::new(nl_agent)).await?;
    hub.register_agent(Box::new(code_agent)).await?;
    hub.register_agent(Box::new(security_agent)).await?;
    println!("✅ All agents registered with Communication Hub\n");

    // Demo 1: Natural Language Processing
    println!("🔍 Demo 1: Natural Language Processing");
    println!("--------------------------------------");

    let context = AgentContext {
        project_root: Some("/home/user/project".to_string()),
        current_directory: "/home/user/project/src".to_string(),
        current_branch: Some("feature/ai-agents".to_string()),
        open_files: vec!["main.rs".to_string(), "lib.rs".to_string()],
        recent_commands: vec!["cargo build".to_string(), "git status".to_string()],
        environment_vars: HashMap::new(),
        user_preferences: HashMap::new(),
    };

    let nl_request = AgentRequest {
        id: Uuid::new_v4(),
        request_type: AgentRequestType::Custom("ProcessNaturalLanguage".to_string()),
        payload: serde_json::json!("Generate a Rust function that validates email addresses"),
        context: context.clone(),
        metadata: HashMap::new(),
    };

    match hub.route_request(nl_request).await {
        Ok(response) => {
            println!("📝 Natural Language Processing Result:");
            if let Some(intent) = response.payload.get("intent") {
                println!("   Intent: {}", intent.get("name").unwrap_or(&serde_json::Value::Null));
                println!(
                    "   Confidence: {}",
                    intent.get("confidence").unwrap_or(&serde_json::Value::Null)
                );
            }
            if let Some(response_text) = response.payload.get("response") {
                println!("   Response: {}", response_text.as_str().unwrap_or("No response"));
            }

            if !response.next_actions.is_empty() {
                println!("   Suggested Actions:");
                for action in &response.next_actions {
                    println!("     - {:?}: {}", action.action_type, action.description);
                }
            }
        }
        Err(e) => println!("❌ NL Processing failed: {}", e),
    }
    println!();

    // Demo 2: Multi-Agent Workflow
    println!("🔄 Demo 2: Multi-Agent Workflow");
    println!("-------------------------------");

    let workflow_context = serde_json::json!({
        "requirements": "Create a secure password hashing function in Rust",
        "language": "rust",
        "security_level": "high"
    });

    match hub.start_workflow("code_generation_workflow", workflow_context).await {
        Ok(workflow_id) => {
            println!("🚀 Started workflow: {}", workflow_id);

            // Check workflow status
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            match hub.get_workflow_status(workflow_id).await {
                Ok(status) => println!("📊 Workflow status: {:?}", status),
                Err(e) => println!("❌ Failed to get workflow status: {}", e),
            }
        }
        Err(e) => println!("❌ Workflow failed to start: {}", e),
    }
    println!();

    // Demo 3: Agent Capabilities
    println!("🎯 Demo 3: Agent Capabilities");
    println!("-----------------------------");

    let agents = hub.list_agents().await;
    println!("📋 Registered agents: {}", agents.len());
    for agent_id in agents {
        match hub.get_agent_status(&agent_id).await {
            Ok(status) => {
                println!(
                    "   {} - Healthy: {}, Busy: {}",
                    agent_id, status.is_healthy, status.is_busy
                );
                if let Some(task) = status.current_task {
                    println!("     Current task: {}", task);
                }
            }
            Err(e) => println!("   {} - Error: {}", agent_id, e),
        }
    }
    println!();

    // Demo 4: Event Subscription
    println!("📡 Demo 4: Event Subscription");
    println!("-----------------------------");

    let mut event_receiver = hub.subscribe_to_events();

    // Start a simple workflow to generate events
    let simple_context = serde_json::json!({
        "task": "demo event generation"
    });

    if let Ok(workflow_id) = hub.start_workflow("code_generation_workflow", simple_context).await {
        println!("🎬 Started demo workflow: {}", workflow_id);

        // Listen for events (with timeout)
        let timeout = tokio::time::Duration::from_millis(500);
        match tokio::time::timeout(timeout, event_receiver.recv()).await {
            Ok(Ok(event)) => {
                println!("📨 Received event: {:?}", event);
            }
            Ok(Err(e)) => {
                println!("❌ Event receive error: {}", e);
            }
            Err(_) => {
                println!("⏱️  Event timeout (this is expected in demo)");
            }
        }
    }
    println!();

    // Demo 5: Natural Language Conversation
    println!("💬 Demo 5: Natural Language Conversation");
    println!("----------------------------------------");

    let conversation_examples = [
        "Check if the current directory has any security vulnerabilities",
        "Create a new Git branch called feature/security-improvements",
        "Generate documentation for the main.rs file",
        "Run tests and check code quality",
    ];

    for (i, input) in conversation_examples.iter().enumerate() {
        println!("User {}: {}", i + 1, input);

        let request = AgentRequest {
            id: Uuid::new_v4(),
            request_type: AgentRequestType::Custom("ProcessNaturalLanguage".to_string()),
            payload: serde_json::json!(input),
            context: context.clone(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("conversation_turn".to_string(), i.to_string());
                meta
            },
        };

        match hub.route_request(request).await {
            Ok(response) => {
                if let Some(response_text) = response.payload.get("response") {
                    println!(
                        "Bot {}: {}\n",
                        i + 1,
                        response_text.as_str().unwrap_or("No response")
                    );
                }
            }
            Err(e) => println!("Bot {}: Error - {}\n", i + 1, e),
        }

        // Small delay between requests
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    println!("🎉 Demo completed successfully!");
    println!("\nThe Natural Language Agent system provides:");
    println!("  • Intent recognition and entity extraction");
    println!("  • Multi-agent workflow coordination");
    println!("  • Event-driven agent communication");
    println!("  • Conversational AI interfaces");
    println!("  • Privacy-first, local processing");

    Ok(())
}

// Helper function to display agent information
#[allow(dead_code)]
async fn display_agent_info(agent: &dyn Agent) {
    println!("Agent: {}", agent.name());
    println!("  ID: {}", agent.id());
    println!("  Description: {}", agent.description());
    println!("  Capabilities: {:?}", agent.capabilities());

    let status = agent.status().await;
    if status.is_healthy && !status.is_busy {
        println!("  Status: Healthy and Ready");
    } else if status.is_busy {
        println!("  Status: Busy");
        if let Some(task) = status.current_task {
            println!("    Current task: {}", task);
        }
    } else {
        println!("  Status: Not healthy");
        if let Some(error) = status.error_message {
            println!("    Error: {}", error);
        }
    }
}
