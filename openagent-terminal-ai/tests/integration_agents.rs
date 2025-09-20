#![cfg(feature = "agents")]
use chrono::Utc;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use async_trait::async_trait;
use openagent_terminal_ai::agents::types::{
    AgentExecutionContext, AgentMessage, ExecutionCondition, ExecutionStrategy, MessagePriority,
    MessageType, NodeStatus, NodeType, WorkflowEdge, WorkflowExecutionGraph, WorkflowNode,
    WorkflowStatus,
};
use openagent_terminal_ai::agents::{
    AgentCapabilities, AgentError, AgentRequest, AgentResponse, AiAgent, PrivacyLevel,
};

#[derive(Debug)]
struct DummyAgent;

#[async_trait]
impl AiAgent for DummyAgent {
    fn name(&self) -> &'static str {
        "dummy"
    }
    fn version(&self) -> &'static str {
        "0.1.0"
    }

    async fn process(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        match request {
            AgentRequest::Command(_req) => {
                Ok(AgentResponse::Commands(vec![openagent_terminal_ai::AiProposal {
                    title: "noop".to_string(),
                    description: Some("no-op".to_string()),
                    proposed_commands: vec!["echo dummy".to_string()],
                }]))
            }
            _ => Err(AgentError::NotSupported("unsupported".to_string())),
        }
    }

    fn can_handle(&self, _request: &AgentRequest) -> bool {
        true
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            supported_languages: vec![],
            supported_frameworks: vec![],
            features: vec!["dummy_cap".to_string()],
            requires_internet: false,
            privacy_level: PrivacyLevel::Local,
        }
    }
}

#[tokio::test]
async fn workflow_sequential_with_dummy_agent_completes() {
    let orchestrator =
        openagent_terminal_ai::agents::workflow_orchestration::WorkflowOrchestrator::new(
            openagent_terminal_ai::agents::workflow_orchestration::OrchestratorConfig::default(),
        );

    // Register dummy agent
    orchestrator.register_agent(Arc::new(DummyAgent)).await.unwrap();

    // Build workflow: Start -> Task(using dummy_cap)
    let id = Uuid::new_v4();
    let mut nodes = std::collections::HashMap::new();
    nodes.insert(
        "start".to_string(),
        WorkflowNode {
            id: "start".to_string(),
            name: "Start".to_string(),
            node_type: NodeType::Start,
            agent_id: None,
            dependencies: vec![],
            status: NodeStatus::Pending,
            input_schema: None,
            output_schema: None,
            timeout_ms: None,
            retry_count: 0,
            max_retries: 0,
            parallel_group: None,
        },
    );
    nodes.insert(
        "task".to_string(),
        WorkflowNode {
            id: "task".to_string(),
            name: "Task".to_string(),
            node_type: NodeType::Task {
                agent_capability: "dummy_cap".to_string(),
                payload: serde_json::json!({"action":"run"}),
            },
            agent_id: Some("dummy".to_string()),
            dependencies: vec!["start".to_string()],
            status: NodeStatus::Pending,
            input_schema: None,
            output_schema: None,
            timeout_ms: Some(5_000),
            retry_count: 0,
            max_retries: 0,
            parallel_group: None,
        },
    );

    let edges = vec![WorkflowEdge {
        from: "start".to_string(),
        to: "task".to_string(),
        condition: Some(ExecutionCondition::Always),
        weight: 1.0,
    }];

    let graph = WorkflowExecutionGraph {
        id,
        name: "dummy_workflow".to_string(),
        nodes,
        edges,
        execution_strategy: ExecutionStrategy::Sequential,
        status: WorkflowStatus::Pending,
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
    };

    let ctx = AgentExecutionContext {
        workflow_id: Some(id),
        node_id: None,
        parent_context: None,
        variables: std::collections::HashMap::new(),
        metadata: std::collections::HashMap::new(),
        timeout: None,
        cancellation_token: None,
    };

    let result = orchestrator.execute_workflow(graph, ctx).await.unwrap();
    assert!(result.is_successful());
    assert!(result.get_node_result("task").is_some());
}

#[tokio::test]
async fn communication_hub_delivers_message() {
    let hub = openagent_terminal_ai::agents::communication_hub::CommunicationHub::new(
        openagent_terminal_ai::agents::communication_hub::HubConfig::default(),
    );

    // Register dummy agent
    hub.register_agent(Arc::new(DummyAgent)).await.unwrap();

    // Build AiRequest payload
    let req = openagent_terminal_ai::AiRequest {
        scratch_text: "do something".to_string(),
        working_directory: None,
        shell_kind: None,
        context: vec![],
    };
    let payload = serde_json::to_value(&req).unwrap();

    // Send message to dummy agent
    let message = AgentMessage {
        id: Uuid::new_v4(),
        from_agent: "tester".to_string(),
        to_agent: "dummy".to_string(),
        message_type: MessageType::Request,
        payload,
        correlation_id: None,
        timestamp: Utc::now(),
        priority: MessagePriority::Normal,
        ttl_seconds: Some(10),
    };

    hub.send_message(message).await.unwrap();

    // Allow background task to deliver
    sleep(Duration::from_millis(200)).await;

    let metrics = hub.get_metrics().await;
    let metrics_json = serde_json::to_value(&metrics).unwrap();
    let sent = metrics_json.get("messages_sent").and_then(|v| v.as_u64()).unwrap_or(0);
    assert!(sent >= 1);
}
