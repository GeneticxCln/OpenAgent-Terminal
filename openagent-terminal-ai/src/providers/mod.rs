#[cfg(feature = "ai-anthropic")]
pub mod anthropic;
#[cfg(feature = "ai-ollama")]
pub mod ollama;
#[cfg(feature = "ai-openai")]
pub mod openai;

#[cfg(feature = "ai-anthropic")]
pub use anthropic::AnthropicProvider;
#[cfg(feature = "ai-ollama")]
pub use ollama::OllamaProvider;
#[cfg(feature = "ai-openai")]
pub use openai::OpenAiProvider;
