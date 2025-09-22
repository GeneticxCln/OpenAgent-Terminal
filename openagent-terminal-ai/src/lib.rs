//! AI interfaces for OpenAgent Terminal (optional, privacy-first).
//! Only traits and simple types; no network clients included.

#![forbid(unsafe_code)]
#![allow(
    clippy::pedantic,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::too_many_lines,
    clippy::items_after_statements,
    clippy::match_same_arms,
    clippy::redundant_else,
    clippy::redundant_closure_for_method_calls,
    clippy::map_unwrap_or,
    clippy::manual_let_else,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::unnecessary_literal_bound,
    clippy::uninlined_format_args,
    clippy::if_not_else,
    clippy::needless_continue,
    clippy::unnested_or_patterns,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::match_like_matches_macro,
    clippy::unnecessary_map_or,
    clippy::manual_range_contains,
    clippy::vec_init_then_push,
    clippy::manual_strip,
    clippy::map_flatten,
    clippy::manual_clamp,
    clippy::only_used_in_recursion,
    clippy::large_enum_variant
)]
#![allow(dead_code, unused_imports)]

pub mod context;
pub mod error;
pub mod privacy;
pub mod streaming;

// Enhanced multi-agent system for Blitzy Platform integration
#[cfg(feature = "agents")]
pub mod agents;

#[cfg(feature = "agents")]
use serde::{Deserialize, Serialize};

/// A request to the AI provider, typically from a scratch buffer.
#[cfg_attr(feature = "agents", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct AiRequest {
    pub scratch_text: String,
    pub working_directory: Option<String>,
    pub shell_kind: Option<String>,
    /// Arbitrary context from the terminal (env, platform, etc.).
    pub context: Vec<(String, String)>,
}

/// A single proposed change or command from the AI provider.
#[cfg_attr(feature = "agents", derive(Serialize, Deserialize))]
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

#[cfg(any(
    feature = "ai-ollama",
    feature = "ai-openai",
    feature = "ai-anthropic",
    feature = "ai-openrouter",
))]
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
                    "Check OPENAGENT_OLLAMA_ENDPOINT and OPENAGENT_OLLAMA_MODEL (preferred), or \
                     legacy OLLAMA_ENDPOINT/OLLAMA_MODEL environment variables"
                        .to_string(),
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
        #[cfg(feature = "ai-openrouter")]
        "openrouter" => providers::OpenRouterProvider::from_env()
            .map(|p| Box::new(p) as Box<dyn AiProvider>)
            .map_err(|e| AiError::Configuration {
                setting: "OpenRouter".to_string(),
                message: e,
                suggestion: Some(
                    "Check OPENROUTER_API_KEY and OPENROUTER_MODEL environment variables"
                        .to_string(),
                ),
            }),
        _ => Err(AiError::Configuration {
            setting: "provider".to_string(),
            message: format!("Unknown provider: {}", name),
            suggestion: Some(
                "Available providers: null, ollama, openai, anthropic, openrouter".to_string(),
            ),
        }),
    }
}

/// Helper to append collected context to an AI request, with privacy sanitization.
pub fn build_request_with_context(
    mut req: AiRequest,
    manager: &context::ContextManager,
    max_size_kb: usize,
) -> AiRequest {
    let ctx = manager.collect_all(max_size_kb);
    req.context.extend(ctx);
    // Apply default privacy options based on environment
    let opts = privacy::AiPrivacyOptions::from_env();
    privacy::sanitize_request(&req, opts)
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
