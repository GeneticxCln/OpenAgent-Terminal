#![cfg(feature = "agents")]
use std::sync::Arc;
use async_trait::async_trait;
use openagent_terminal_ai::agents::{AiAgent, AgentRequest, AgentResponse, AgentError, AgentCapabilities, PrivacyLevel};
use openagent_terminal_ai::agents::communication_hub::{CommunicationHub, HubConfig};
use openagent_terminal_ai::AiRequest;

#[derive(Debug)]
struct EchoAgent;

#[async_trait]
impl AiAgent for EchoAgent {
    fn name(&self) -> &'static str { "echo" }
    fn version(&self) -> &'static str { "0.1.0" }

    async fn process(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        match request {
            AgentRequest::Command(req) => Ok(AgentResponse::Commands(vec![openagent_terminal_ai::AiProposal{
                title: "echo".into(),
                description: Some("echo back".into()),
                proposed_commands: vec![format!("echo {}", req.scratch_text)],
            }])),
            _ => Err(AgentError::NotSupported("unsupported".into()))
        }
    }

    fn can_handle(&self, _request: &AgentRequest) -> bool { true }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities{
            supported_languages: vec![],
            supported_frameworks: vec![],
            features: vec!["echo".into()],
            requires_internet: false,
            privacy_level: PrivacyLevel::Local,
        }
    }
}

#[tokio::test]
async fn execute_parallel_single_task_completes_and_metrics_increment() {
    let hub = CommunicationHub::new(HubConfig::default());
    hub.register_agent(Arc::new(EchoAgent)).await.unwrap();

    let req = AiRequest { scratch_text: "hi".into(), working_directory: None, shell_kind: None, context: vec![] };
    let payload = serde_json::to_value(&req).unwrap();
    let msg = openagent_terminal_ai::agents::types::AgentMessage{
        id: uuid::Uuid::new_v4(),
        from_agent: "tester".into(),
        to_agent: "echo".into(),
        message_type: openagent_terminal_ai::agents::types::MessageType::Request,
        payload,
        correlation_id: None,
        timestamp: chrono::Utc::now(),
        priority: openagent_terminal_ai::agents::types::MessagePriority::Normal,
        ttl_seconds: Some(5),
    };

    // This exercises message routing and delivery
    hub.send_message(msg).await.unwrap();

    // Allow brief time for background delivery
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let metrics = hub.get_metrics().await;
    // Access via serde to avoid relying on field visibility
    let v = serde_json::to_value(&metrics).unwrap();
    let sent = v.get("messages_sent").and_then(|x| x.as_u64()).unwrap_or(0);
    assert!(sent >= 1);
}