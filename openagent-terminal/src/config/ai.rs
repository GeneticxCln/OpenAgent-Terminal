use clap::ValueEnum;
use openagent_terminal_config_derive::{ConfigDeserialize, SerdeReplace};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Routing mode for AI requests
#[derive(ConfigDeserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AiRoutingMode {
    #[default]
    Auto,
    Agent,
    Provider,
}

#[derive(ValueEnum, SerdeReplace, Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AiApplyJoinStrategy {
    AndThen,
    Lines,
}

impl Default for AiApplyJoinStrategy {
    fn default() -> Self {
        Self::AndThen
    }
}

/// AI integration configuration (build- and run-time opt-in).
#[derive(ConfigDeserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AiConfig {
    /// Enable AI interface at runtime. Defaults to false.
    pub enabled: bool,

    /// Routing mode: auto (try agents then fallback), agent, or provider.
    #[serde(default)]
    pub routing: AiRoutingMode,

    /// Context collection settings for enriching AI requests.
    #[serde(default)]
    pub context: AiContextConfig,

    /// History retention and pruning settings for AI runtime and conversation logs.
    #[serde(default)]
    pub history_retention: AiHistoryRetention,

    /// Strategy to join multiple commands when applying.
    #[serde(default)]
    pub apply_joiner: AiApplyJoinStrategy,

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

    /// Provider identifier, e.g. "null", "ollama", "openai"; application chooses the concrete
    /// impl.
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

    /// Provider-specific configurations (secure, isolated)
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            routing: AiRoutingMode::Auto,
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
            providers: HashMap::new(),
            context: AiContextConfig::default(),
            apply_joiner: AiApplyJoinStrategy::AndThen,
            history_retention: AiHistoryRetention::default(),
        }
    }
}

/// Secure provider-specific configuration.
#[derive(ConfigDeserialize, Serialize, Clone, Debug, PartialEq, Default)]
pub struct ProviderConfig {
    /// Environment variable name holding the API key/secret. Never printed.
    pub api_key_env: Option<String>,

    /// Environment variable name holding the remote endpoint (if any).
    pub endpoint_env: Option<String>,

    /// Environment variable name holding the model identifier.
    pub model_env: Option<String>,

    /// Default model if environment variable is not set.
    pub default_model: Option<String>,

    /// Default endpoint if environment variable is not set.
    pub default_endpoint: Option<String>,

    /// Additional provider-specific configuration.
    #[serde(default)]
    pub extra: HashMap<String, String>,
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

impl fmt::Display for AiLogVerbosity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiLogVerbosity::Off => write!(f, "off"),
            AiLogVerbosity::Summary => write!(f, "summary"),
            AiLogVerbosity::Verbose => write!(f, "verbose"),
        }
    }
}

/// Context collection configuration for enriching AI requests.
#[derive(ConfigDeserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AiContextConfig {
    /// Enable context collection
    pub enabled: bool,
    /// Maximum number of bytes to include from providers (approximate)
    pub max_bytes: usize,
    /// Providers to include, in priority order. Supported: "env", "git", "file_tree"
    #[serde(default)]
    pub providers: Vec<String>,
    /// Timeouts for provider collection
    #[serde(default)]
    pub timeouts: AiContextTimeouts,
    /// File tree provider options
    #[serde(default)]
    pub file_tree: AiFileTreeConfig,
    /// Git provider options
    #[serde(default)]
    pub git: AiGitConfig,
}

impl Default for AiContextConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_bytes: 32 * 1024, // 32KB
            providers: vec!["env".into(), "git".into(), "file_tree".into()],
            timeouts: AiContextTimeouts::default(),
            file_tree: AiFileTreeConfig::default(),
            git: AiGitConfig::default(),
        }
    }
}

#[derive(ConfigDeserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AiContextTimeouts {
    /// Soft per-provider timeout in milliseconds (providers run in parallel)
    pub per_provider_ms: u64,
    /// Overall deadline for context collection in milliseconds
    pub overall_ms: u64,
    /// Optional per-provider overrides (takes precedence over per_provider_ms)
    #[serde(default)]
    pub env_ms: Option<u64>,
    #[serde(default)]
    pub git_ms: Option<u64>,
    #[serde(default)]
    pub file_tree_ms: Option<u64>,
}

impl Default for AiContextTimeouts {
    fn default() -> Self {
        Self {
            per_provider_ms: 150,
            overall_ms: 300,
            env_ms: None,
            git_ms: None,
            file_tree_ms: None,
        }
    }
}

#[derive(ConfigDeserialize, Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AiRootStrategy {
    #[default]
    Git,
    Cwd,
}

#[derive(ConfigDeserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AiFileTreeConfig {
    /// Maximum number of file entries to include
    pub max_entries: usize,
    /// Root selection strategy: repo root or current working directory
    #[serde(default)]
    pub root_strategy: AiRootStrategy,
}

impl Default for AiFileTreeConfig {
    fn default() -> Self {
        Self { max_entries: 500, root_strategy: AiRootStrategy::Git }
    }
}

#[derive(ConfigDeserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AiGitConfig {
    #[serde(default = "default_true")]
    pub include_branch: bool,
    #[serde(default = "default_true")]
    pub include_status: bool,
}

impl Default for AiGitConfig {
    fn default() -> Self {
        Self { include_branch: true, include_status: true }
    }
}

#[allow(dead_code)]
fn default_true() -> bool {
    true
}

#[derive(ConfigDeserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AiHistoryRetention {
    /// Maximum number of UI prompt history entries to keep in memory
    pub ui_max_entries: usize,
    /// Maximum total bytes for UI prompt history (sum of entry lengths)
    pub ui_max_bytes: usize,
    /// Maximum on-disk JSONL size before rotation (bytes)
    pub conversation_jsonl_max_bytes: u64,
    /// How many rotated JSONL files to keep
    pub conversation_rotated_keep: usize,
    /// Maximum SQLite rows to keep for conversations
    pub conversation_max_rows: u64,
    /// Maximum age in days for conversations (SQLite and rotated JSONL cleanup)
    pub conversation_max_age_days: u64,
}

impl Default for AiHistoryRetention {
    fn default() -> Self {
        Self {
            ui_max_entries: 200,
            ui_max_bytes: 128 * 1024,                      // 128KB
            conversation_jsonl_max_bytes: 8 * 1024 * 1024, // 8MB
            conversation_rotated_keep: 8,
            conversation_max_rows: 50_000,
            conversation_max_age_days: 90,
        }
    }
}
