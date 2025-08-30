use serde::Serialize;

use openagent_terminal_config_derive::ConfigDeserialize;

/// AI integration configuration (build- and run-time opt-in).
#[derive(ConfigDeserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AiConfig {
    /// Enable AI interface at runtime. Defaults to false.
    pub enabled: bool,

    /// Provider identifier, e.g. "null", "http"; application chooses the concrete impl.
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
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: Some("null".into()),
            endpoint_env: None,
            api_key_env: None,
            model_env: None,
            scratch_autosave: true,
            propose_max_commands: 10,
            never_auto_run: true,
        }
    }
}

