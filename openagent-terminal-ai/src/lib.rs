//! AI interfaces for OpenAgent Terminal (optional, privacy-first).
//! Only traits and simple types; no network clients included.

#![forbid(unsafe_code)]

pub mod error;
pub mod privacy;
pub mod streaming;

/// A request to the AI provider, typically from a scratch buffer.
#[derive(Debug, Clone)]
pub struct AiRequest {
    pub scratch_text: String,
    pub working_directory: Option<String>,
    pub shell_kind: Option<String>,
    /// Arbitrary context from the terminal (env, platform, etc.).
    pub context: Vec<(String, String)>,
}

/// A single proposed change or command from the AI provider.
#[derive(Debug, Clone)]
pub struct AiProposal {
    pub title: String,
    pub description: Option<String>,
    /// Never auto-run: this is only a suggestion for the user to apply/copy.
    pub proposed_commands: Vec<String>,
}

/// A provider interface for generating proposals.
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &'static str;

    /// Generate proposals; implementations must never attempt to run commands.
    fn propose(&self, _req: AiRequest) -> Result<Vec<AiProposal>, String> {
        Ok(Vec::new())
    }

    /// Optional: Stream partial text chunks while generating a response.
    /// Returns Ok(true) if streaming was performed, Ok(false) if not supported.
    /// Implementations should call `on_chunk` with incremental text (not commands),
    /// and finish by returning Ok(true). Errors should abort streaming with Err.
    /// The `cancel` flag should be checked periodically to abort promptly when requested.
    fn propose_stream(
        &self,
        _req: AiRequest,
        _on_chunk: &mut dyn FnMut(&str),
        _cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        Ok(false)
    }
}

/// A no-op provider that returns no proposals.
#[derive(Debug, Default)]
pub struct NullProvider;

impl AiProvider for NullProvider {
    fn name(&self) -> &'static str {
        "null"
    }
}

#[cfg(any(feature = "ai-ollama", feature = "ai-openai", feature = "ai-anthropic"))]
pub mod providers;

/// Factory function for creating providers
pub fn create_provider(name: &str) -> Result<Box<dyn AiProvider>, error::AiError> {
    use error::AiError;

    match name {
        "null" => Ok(Box::new(NullProvider)),
        #[cfg(feature = "ai-ollama")]
        "ollama" => providers::OllamaProvider::from_env()
            .map(|p| Box::new(p) as Box<dyn AiProvider>)
            .map_err(|e| AiError::Configuration {
                setting: "Ollama".to_string(),
                message: e,
                suggestion: Some(
                    "Check OLLAMA_ENDPOINT and OLLAMA_MODEL environment variables".to_string(),
                ),
            }),
        #[cfg(feature = "ai-openai")]
        "openai" => providers::OpenAiProvider::from_env()
            .map(|p| Box::new(p) as Box<dyn AiProvider>)
            .map_err(|e| AiError::Configuration {
                setting: "OpenAI".to_string(),
                message: e,
                suggestion: Some(
                    "Check OPENAI_API_KEY and OPENAI_MODEL environment variables".to_string(),
                ),
            }),
        #[cfg(feature = "ai-anthropic")]
        "anthropic" => providers::AnthropicProvider::from_env()
            .map(|p| Box::new(p) as Box<dyn AiProvider>)
            .map_err(|e| AiError::Configuration {
                setting: "Anthropic".to_string(),
                message: e,
                suggestion: Some(
                    "Check ANTHROPIC_API_KEY and ANTHROPIC_MODEL environment variables".to_string(),
                ),
            }),
        _ => Err(AiError::Configuration {
            setting: "provider".to_string(),
            message: format!("Unknown provider: {}", name),
            suggestion: Some("Available providers: null, ollama, openai, anthropic".to_string()),
        }),
    }
}

#[cfg(test)]
mod tests {

    #[cfg(feature = "ai-openai")]
    #[test]
    fn openai_from_env_fails_without_model() {
        // Ensure model var is not set
        std::env::remove_var("OPENAI_MODEL");
        // Set a fake key so only the model check fails
        std::env::set_var("OPENAI_API_KEY", "x");
        let res = crate::providers::OpenAiProvider::from_env();
        assert!(res.is_err());
    }

    #[cfg(feature = "ai-ollama")]
    #[test]
    fn ollama_from_env_fails_without_model() {
        std::env::remove_var("OLLAMA_MODEL");
        let res = crate::providers::OllamaProvider::from_env();
        assert!(res.is_err());
    }
}
