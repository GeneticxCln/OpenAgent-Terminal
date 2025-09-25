//! Integration tests for AI features
//! Tests the full AI agent system including communication, workflows, and error handling

use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;
use tokio::time::timeout;

#[cfg(feature = "ai")]
use openagent_terminal::ai::{
    agents::{
        Agent, AgentCapability, AgentConfig, AgentContext, AgentRequest, AgentResponse,
        AgentRequestType, RequestPriority,
        natural_language::{NaturalLanguageAgent, ConversationRole},
        workflow_orchestration::{WorkflowOrchestrationAgent, WorkflowTemplate, WorkflowStep, WorkflowAction},
    },
    communication::{
        AgentCommunicationCoordinator, EventBus, MessagePriority, AgentMessage,
    },
};

// Helper functions for test setup
#[cfg(feature = "ai")]
fn create_test_context() -> AgentContext {
    AgentContext {
        project_root: Some("/tmp/test-project".to_string()),
        current_directory: "/tmp/test-project/src".to_string(),
        current_branch: Some("main".to_string()),
        shell_kind: Some("zsh".to_string()),
        open_files: vec![
            "src/main.rs".to_string(),
            "Cargo.toml".to_string(),
        ],
        recent_commands: vec![
            "cargo build".to_string(),
            "git status".to_string(),
        ],
        environment_vars: {
            let mut env = HashMap::new();
            env.insert("RUST_LOG".to_string(), "debug".to_string());
            env
        },
        user_preferences: HashMap::new(),
    }
}

#[cfg(feature = "ai")]
fn create_test_request(request_type: AgentRequestType, content: String) -> AgentRequest {
    AgentRequest {
        id: Uuid::new_v4(),
        request_type,
        content,
        context: HashMap::new(),
        priority: RequestPriority::Normal,
        metadata: HashMap::new(),
    }
}

#[cfg(feature = "ai")]
mod natural_language_tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_initialization() {
        let mut agent = NaturalLanguageAgent::new();
        let context = create_test_context();
        
        let result = agent.initialize(context).await;
        assert!(result.is_ok(), "Agent initialization failed: {:?}", result);
        
        // Verify agent properties
        assert_eq!(agent.id(), "natural-language");
        assert_eq!(agent.name(), "Natural Language Agent");
        assert!(!agent.capabilities().is_empty());
    }

    #[tokio::test]
    async fn test_conversation_management() {
        let mut agent = NaturalLanguageAgent::new();
        
        // Add conversation turns
        let turn1_id = agent.add_conversation_turn(
            ConversationRole::User,
            "Hello, can you help me with Rust?".to_string(),
        );
        
        let turn2_id = agent.add_conversation_turn(
            ConversationRole::Assistant,
            "Of course! I'd be happy to help you with Rust programming.".to_string(),
        );
        
        // Verify turns were added
        assert_ne!(turn1_id, turn2_id);
        
        // Test conversation history limit
        for i in 0..60 {
            agent.add_conversation_turn(
                ConversationRole::User,
                format!("Message {}", i),
            );
        }
        
        // Should be limited to 50 turns
        // This is a behavioral test - we can't easily access the internal history size
        // but we can ensure the agent still functions after many additions
        let context = create_test_context();
        let result = agent.process_input("test input", &context);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_entity_extraction() {
        let agent = NaturalLanguageAgent::new();
        
        let test_cases = vec![
            ("create a rust function", "rust"),
            ("git commit changes", "git"),
            ("edit ~/project/main.rs", "~/project/main.rs"),
            ("run cargo build --release", "cargo"),
            ("analyze python code for security issues", "python"),
        ];
        
        for (input, expected_entity) in test_cases {
            let entities = agent.extract_entities(input);
            
            let found_entity = entities.iter().any(|entity| {
                entity.value.to_lowercase().contains(&expected_entity.to_lowercase())
            });
            
            assert!(found_entity, 
                "Expected to find entity '{}' in input '{}', but found: {:?}", 
                expected_entity, input, entities
            );
        }
    }

    #[tokio::test]
    async fn test_intent_classification() {
        let agent = NaturalLanguageAgent::new();
        let context = create_test_context();
        
        let test_cases = vec![
            ("generate a rust function", "code_generation"),
            ("check security vulnerabilities", "security_analysis"),
            ("git commit my changes", "git_operations"),
            ("analyze this file", "file_operations"),
        ];
        
        for (input, expected_intent) in test_cases {
            let result = agent.process_input(input, &context);
            assert!(result.is_ok(), "Failed to process input: {}", input);
            
            let processed = result.unwrap();
            if let Some(intent) = processed.intent {
                assert_eq!(intent.name, expected_intent,
                    "Expected intent '{}' for input '{}', got '{}'", 
                    expected_intent, input, intent.name
                );
                assert!(intent.confidence > 0.0, "Intent confidence should be positive");
            } else {
                panic!("No intent classified for input: {}", input);
            }
        }
    }

    #[tokio::test]
    async fn test_error_handling() {
        let agent = NaturalLanguageAgent::new();
        let context = create_test_context();
        
        // Test with empty input
        let result = agent.process_input("", &context);
        assert!(result.is_ok(), "Should handle empty input gracefully");
        
        // Test with very long input
        let long_input = "a".repeat(10000);
        let result = agent.process_input(&long_input, &context);
        assert!(result.is_ok(), "Should handle very long input");
        
        // Test with special characters
        let special_input = "!@#$%^&*()_+-=[]{}|;':,.<>?";
        let result = agent.process_input(special_input, &context);
        assert!(result.is_ok(), "Should handle special characters");
        
        // Test with Unicode characters
        let unicode_input = "Hello 世界! 🌍 Test émojis 🚀 and àccénts";
        let result = agent.process_input(unicode_input, &context);
        assert!(result.is_ok(), "Should handle Unicode characters");
    }
}

#[cfg(feature = "ai")]
mod workflow_orchestration_tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_agent_initialization() {
        let mut agent = WorkflowOrchestrationAgent::new();
        let context = create_test_context();
        
        let result = agent.initialize(context).await;
        assert!(result.is_ok(), "Workflow agent initialization failed: {:?}", result);
        
        assert_eq!(agent.id(), "workflow-orchestration");
        assert!(agent.capabilities().contains(&AgentCapability::WorkflowOrchestration));
    }

    #[tokio::test]
    async fn test_workflow_template_registration() {
        let agent = WorkflowOrchestrationAgent::new();
        
        let template = WorkflowTemplate {
            id: "test-workflow".to_string(),
            name: "Test Workflow".to_string(),
            description: "A test workflow".to_string(),
            steps: vec![
                WorkflowStep {
                    id: "step1".to_string(),
                    name: "Test Step".to_string(),
                    agent_type: "test-agent".to_string(),
                    action: WorkflowAction::Custom {
                        action_type: "test".to_string(),
                        payload: serde_json::json!({"test": true}),
                    },
                    depends_on: vec![],
                    parameters: HashMap::new(),
                    timeout_seconds: Some(60),
                    retry_count: Some(3),
                    on_failure: None,
                },
            ],
            required_agents: vec!["test-agent".to_string()],
            parameters: HashMap::new(),
            timeout_seconds: Some(300),
        };
        
        let result = agent.register_template(template).await;
        assert!(result.is_ok(), "Template registration failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_workflow_execution() {
        let agent = WorkflowOrchestrationAgent::new();
        
        // Register a simple template
        let template = WorkflowTemplate {
            id: "simple-workflow".to_string(),
            name: "Simple Workflow".to_string(),
            description: "A simple test workflow".to_string(),
            steps: vec![],
            required_agents: vec![],
            parameters: HashMap::new(),
            timeout_seconds: Some(60),
        };
        
        agent.register_template(template).await.unwrap();
        
        // Execute the workflow
        let parameters = HashMap::new();
        let result = agent.execute_workflow("simple-workflow".to_string(), parameters).await;
        
        assert!(result.is_ok(), "Workflow execution failed: {:?}", result);
        
        let workflow_id = result.unwrap();
        
        // Check workflow status
        let status = agent.get_workflow_status(workflow_id).await;
        assert!(status.is_some(), "Workflow status should be available");
        
        let execution = status.unwrap();
        assert_eq!(execution.template_id, "simple-workflow");
    }
}

#[cfg(feature = "ai")]
mod agent_communication_tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_creation() {
        let event_bus = EventBus::new();
        // Basic creation test - if it doesn't panic, it's good
        assert!(true, "EventBus created successfully");
    }

    #[tokio::test]
    async fn test_coordinator_creation() {
        let event_bus = EventBus::new();
        let coordinator = AgentCommunicationCoordinator::new(event_bus).await;
        
        // Test basic coordinator operations
        let workflows = coordinator.list_active_workflows().await;
        assert!(workflows.is_empty(), "Should start with no active workflows");
    }

    #[tokio::test]
    async fn test_message_sending() {
        let event_bus = EventBus::new();
        
        let message = AgentMessage::DirectRequest {
            request: create_test_request(AgentRequestType::StatusQuery, "test".to_string()),
            reply_to: Some("test-sender".to_string()),
        };
        
        let result = event_bus.send_message(
            message,
            "test-agent".to_string(),
            MessagePriority::Normal,
        ).await;
        
        // Message sending should succeed (even if no agent is listening)
        assert!(result.is_ok(), "Message sending failed: {:?}", result);
    }
}

#[cfg(feature = "ai")]
mod integration_scenarios {
    use super::*;

    #[tokio::test]
    async fn test_timeout_handling() {
        // Test that operations complete within reasonable time
        let event_bus = EventBus::new();
        let coordinator = AgentCommunicationCoordinator::new(event_bus).await;
        
        let operation = async {
            let mut agent = NaturalLanguageAgent::new();
            let context = create_test_context();
            agent.initialize(context).await
        };
        
        let result = timeout(Duration::from_secs(5), operation).await;
        assert!(result.is_ok(), "Agent initialization should complete within 5 seconds");
        assert!(result.unwrap().is_ok(), "Agent initialization should succeed");
    }

    #[tokio::test]
    async fn test_memory_management() {
        // Test that the system handles many operations without excessive memory use
        let mut agent = NaturalLanguageAgent::new();
        let context = create_test_context();
        
        // Process many inputs
        for i in 0..1000 {
            let input = format!("test input {}", i);
            let result = agent.process_input(&input, &context);
            assert!(result.is_ok(), "Input processing should not fail at iteration {}", i);
        }
        
        // Agent should still be functional
        let final_result = agent.process_input("final test", &context);
        assert!(final_result.is_ok(), "Agent should still be functional after many operations");
    }
}

#[cfg(not(feature = "ai"))]
mod disabled_ai_tests {
    #[test]
    fn ai_features_disabled() {
        // This test ensures the file compiles even when AI features are disabled
        assert!(true, "AI features are disabled - tests skipped");
    }
}