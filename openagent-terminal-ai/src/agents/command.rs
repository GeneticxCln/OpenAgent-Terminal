use super::{AgentCapabilities, AgentError, AgentRequest, AgentResponse, AiAgent, PrivacyLevel};
use crate::AiProvider;
use async_trait::async_trait;
use std::sync::Arc;

/// Command agent: routes AgentRequest::Command to the configured AiProvider and returns command proposals.
pub struct CommandAgent {
    provider: Arc<dyn AiProvider>,
}

impl CommandAgent {
    pub fn new(provider: Arc<dyn AiProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl AiAgent for CommandAgent {
    fn name(&self) -> &'static str {
        "command"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    async fn process(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        match request {
            AgentRequest::Command(ai_req) => {
                let proposals = self
                    .provider
                    .propose(ai_req)
                    .map_err(AgentError::ProcessingError)?;
                Ok(AgentResponse::Commands(proposals))
            }
            _ => Err(AgentError::NotSupported(
                "CommandAgent only supports AgentRequest::Command".to_string(),
            )),
        }
    }

    fn can_handle(&self, request: &AgentRequest) -> bool {
        matches!(request, AgentRequest::Command(_))
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            supported_languages: vec![],
            supported_frameworks: vec![],
            features: vec!["command".to_string(), "command_generation".to_string()],
            requires_internet: false, // depends on underlying provider
            privacy_level: PrivacyLevel::Local, // depends on underlying provider configuration
        }
    }
}
