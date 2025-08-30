use std::path::PathBuf;

use serde::Serialize;

use openagent_terminal_config_derive::ConfigDeserialize;

/// Settings/history sync configuration (build- and run-time opt-in).
#[derive(ConfigDeserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct SyncConfig {
    /// Enable sync at runtime. Defaults to false.
    pub enabled: bool,

    /// Provider identifier, e.g. "null", "fs", "http"; application chooses the concrete impl.
    pub provider: Option<String>,

    /// Environment variable name holding the remote endpoint (if any).
    pub endpoint_env: Option<String>,

    /// Local data directory used by file-based providers.
    pub data_dir: Option<PathBuf>,

    /// Environment variable name holding an encryption key (if used).
    pub encryption_key_env: Option<String>,

    /// Sync shell history files.
    pub sync_history: bool,

    /// Sync application settings/config files.
    pub sync_settings: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: Some("null".into()),
            endpoint_env: None,
            data_dir: None,
            encryption_key_env: None,
            sync_history: true,
            sync_settings: true,
        }
    }
}

