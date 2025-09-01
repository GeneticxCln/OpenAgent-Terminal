//! AI interfaces for OpenAgent Terminal (optional, privacy-first).
//! Only traits and simple types; no network clients included.

#![forbid(unsafe_code)]

pub mod privacy;

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
    fn propose(&self, _req: AiRequest) -> Result<Vec<AiProposal>, String> { Ok(Vec::new()) }

    /// Optional: Stream partial text chunks while generating a response.
    /// Returns Ok(true) if streaming was performed, Ok(false) if not supported.
    /// Implementations should call `on_chunk` with incremental text (not commands),
    /// and finish by returning Ok(true). Errors should abort streaming with Err.
    fn propose_stream(&self, _req: AiRequest, _on_chunk: &mut dyn FnMut(&str)) -> Result<bool, String> {
        Ok(false)
    }
}

/// A no-op provider that returns no proposals.
#[derive(Debug, Default)]
pub struct NullProvider;

impl AiProvider for NullProvider {
    fn name(&self) -> &'static str { "null" }
}

#[cfg(feature = "ollama")]
pub mod providers;

/// Factory function for creating providers
pub fn create_provider(name: &str) -> Result<Box<dyn AiProvider>, String> {
    match name {
        "null" => Ok(Box::new(NullProvider)),
        #[cfg(feature = "ollama")]
        "ollama" => Ok(Box::new(providers::OllamaProvider::from_env()?)),
        #[cfg(feature = "ollama")]
        "openai" => Ok(Box::new(providers::OpenAiProvider::from_env()?)),
        #[cfg(feature = "ollama")]
        "anthropic" => Ok(Box::new(providers::AnthropicProvider::from_env()?)),
        _ => Err(format!("Unknown provider: {}", name)),
    }
}

