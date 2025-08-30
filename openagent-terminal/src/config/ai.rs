use serde::{Deserialize, Serialize};

use openagent_terminal_config_derive::ConfigDeserialize;

/// AI integration configuration (build- and run-time opt-in).
#[derive(ConfigDeserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AiConfig {
    /// Enable AI interface at runtime. Defaults to false.
    pub enabled: bool,

    /// Provider identifier, e.g. "null", "ollama", "openai"; application chooses the concrete impl.
    pub provider: Option<String>,

    /// Environment variable name holding the remote endpoint (if any).
    pub endpoint_env: Option<String>,

    /// Environment variable name holding the API key/secret. Never printed.
    pub api_key_env: Option<String>,

    /// Environment variable name holding the model identifier (if used by provider).
    pub model_env: Option<String>,

    /// Auto-save scratch buffer to a file under XDG state dir.
    pub scratch_autosave: bool,

    /// Maximum number of commands per proposal the UI should display.
    pub propose_max_commands: u32,

    /// Hard safety: UI must never auto-run AI-proposed commands.
    pub never_auto_run: bool,
    
    /// Show inline AI suggestions as you type.
    pub inline_suggestions: bool,
    
    /// Keybinding to trigger AI assistant (e.g., "Ctrl+Shift+A").
    pub trigger_key: Option<String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: Some("null".into()),
            endpoint_env: Some("OPENAGENT_AI_ENDPOINT".into()),
            api_key_env: Some("OPENAGENT_AI_API_KEY".into()),
            model_env: Some("OPENAGENT_AI_MODEL".into()),
            scratch_autosave: true,
            propose_max_commands: 10,
            never_auto_run: true,
            inline_suggestions: false,
            trigger_key: Some("Ctrl+Shift+A".into()),
        }
    }
}

/// Ollama-specific configuration.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct OllamaConfig {
    /// Ollama API endpoint.
    pub endpoint: String,
    
    /// Model to use.
    pub model: String,
    
    /// Request timeout in seconds.
    pub timeout: u64,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434".to_string(),
            model: "codellama".to_string(),
            timeout: 30,
        }
    }
}

