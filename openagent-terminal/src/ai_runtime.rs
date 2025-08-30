// AI runtime: UI state and provider wiring (optional feature)
#![cfg(feature = "ai")]

use openagent_terminal_ai::{AiProvider, AiProposal, AiRequest, NullProvider};

#[derive(Debug, Default, Clone)]
pub struct AiUiState {
    pub active: bool,
    pub scratch: String,
    pub proposals: Vec<AiProposal>,
}

pub struct AiRuntime {
    pub ui: AiUiState,
    pub provider: Box<dyn AiProvider>,
}

impl AiRuntime {
    pub fn new(provider: Box<dyn AiProvider>) -> Self {
        Self { ui: AiUiState::default(), provider }
    }

    pub fn from_config(
        provider_id: Option<&str>,
        _endpoint_env: Option<&str>,
        _api_key_env: Option<&str>,
        _model_env: Option<&str>,
    ) -> Self {
        // For now we only support a null provider placeholder.
        let provider: Box<dyn AiProvider> = match provider_id.unwrap_or("null").to_lowercase().as_str() {
            _ => Box::new(NullProvider::default()),
        };
        Self::new(provider)
    }

    pub fn propose(&mut self, working_directory: Option<String>, shell_kind: Option<String>) {
        let req = AiRequest {
            scratch_text: self.ui.scratch.clone(),
            working_directory,
            shell_kind,
            context: Vec::new(),
        };
        self.ui.proposals = self.provider.propose(req).unwrap_or_default();
    }
}

