use clap::ValueEnum;
use openagent_terminal_config_derive::{ConfigDeserialize, SerdeReplace};
use serde::{Deserialize, Serialize};

/// AI integration configuration (build- and run-time opt-in).
#[derive(ConfigDeserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AiConfig {
    /// Enable AI interface at runtime. Defaults to false.
    pub enabled: bool,

    /// Visual height of the AI panel as a fraction of the viewport (0.2..0.6 typical).
    #[serde(default)]
    pub panel_height_fraction: f32,

    /// Backdrop dim alpha drawn behind the AI panel (0.0..0.6 typical).
    #[serde(default)]
    pub backdrop_alpha: f32,

    /// Visuals: draw subtle drop shadow under the panel.
    #[serde(default)]
    pub shadow: bool,
    /// Shadow extent in pixels (approximate blur radius).
    #[serde(default)]
    pub shadow_size_px: u32,
    /// Shadow base alpha (will be distributed across steps).
    #[serde(default)]
    pub shadow_alpha: f32,

    /// Visuals: rounded corners (best-effort; exact rounding may depend on backend).
    #[serde(default)]
    pub rounded_corners: bool,

    /// Corner radius in pixels for rounded panel corners.
    #[serde(default)]
    pub corner_radius_px: f32,

    /// AI logging verbosity: "off" | "summary" | "verbose".
    /// off     -> only errors; summary -> start/finish; verbose -> per-chunk debug
    #[serde(default)]
    pub log_verbosity: AiLogVerbosity,

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

    /// Auto-focus the AI panel when it opens.
    #[serde(default)]
    pub auto_focus: bool,

    /// Show animated typing effect for AI responses.
    #[serde(default)]
    pub animated_typing: bool,

    /// Animation speed multiplier (1.0 = normal, 2.0 = double speed, 0.5 = half speed).
    #[serde(default)]
    pub animation_speed: f32,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            panel_height_fraction: 0.40,
            backdrop_alpha: 0.25,
            shadow: true,
            shadow_size_px: 8,
            shadow_alpha: 0.35,
            rounded_corners: true,
            corner_radius_px: 12.0,
            log_verbosity: AiLogVerbosity::Summary,
            provider: Some("null".into()),
            endpoint_env: Some("OPENAGENT_AI_ENDPOINT".into()),
            api_key_env: Some("OPENAGENT_AI_API_KEY".into()),
            model_env: Some("OPENAGENT_AI_MODEL".into()),
            scratch_autosave: true,
            propose_max_commands: 10,
            never_auto_run: true,
            inline_suggestions: false,
            trigger_key: Some("Ctrl+Shift+A".into()),
            auto_focus: true,
            animated_typing: true,
            animation_speed: 1.0,
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

#[derive(ValueEnum, SerdeReplace, Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AiLogVerbosity {
    Off,
    Summary,
    Verbose,
}

impl Default for AiLogVerbosity {
    fn default() -> Self {
        Self::Summary
    }
}

impl std::fmt::Display for AiLogVerbosity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiLogVerbosity::Off => write!(f, "off"),
            AiLogVerbosity::Summary => write!(f, "summary"),
            AiLogVerbosity::Verbose => write!(f, "verbose"),
        }
    }
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
