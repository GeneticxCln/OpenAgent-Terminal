//! IDE feature configuration (editor, LSP, DAP, indexer)

#![cfg_attr(not(feature = "ide"), allow(dead_code))]

use openagent_terminal_config_derive::ConfigDeserialize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(ConfigDeserialize, Serialize, Debug, Clone, PartialEq)]
pub struct IdeConfig {
    /// Master toggle for IDE features
    #[config(default = true)]
    pub enabled: bool,
    /// Enable embedded editor overlay
    #[config(default = true)]
    pub editor: bool,
    /// Enable Language Server Protocol client
    #[config(default = true)]
    pub lsp: bool,
    /// Enable Debug Adapter Protocol client
    #[config(default = true)]
    pub dap: bool,
    /// Enable project indexer + file tree
    #[config(default = true)]
    pub indexer: bool,

    /// Optional project root override
    #[serde(default)]
    pub project_root: Option<PathBuf>,

    /// Language server commands by language id (e.g., "rust" -> rust-analyzer)
    #[serde(default)]
    pub language_servers: HashMap<String, LanguageServerCommand>,

    /// Debug adapters by runtime (e.g., "node" -> node debug adapter, "codelldb")
    #[serde(default)]
    pub debug_adapters: HashMap<String, DebugAdapterCommand>,
}

impl Default for IdeConfig {
    fn default() -> Self {
        let mut language_servers = HashMap::new();
        language_servers.insert(
            "rust".into(),
            LanguageServerCommand {
                command: "rust-analyzer".into(),
                args: vec![],
                initialization_options: None,
            },
        );
        language_servers.insert(
            "typescript".into(),
            LanguageServerCommand {
                command: "typescript-language-server".into(),
                args: vec!["--stdio".into()],
                initialization_options: None,
            },
        );
        language_servers.insert(
            "python".into(),
            LanguageServerCommand {
                command: "pyright-langserver".into(),
                args: vec!["--stdio".into()],
                initialization_options: None,
            },
        );

        let mut debug_adapters = HashMap::new();
        debug_adapters.insert(
            "codelldb".into(),
            DebugAdapterCommand { command: "codelldb".into(), args: vec![] },
        );

        Self {
            enabled: true,
            editor: true,
            lsp: true,
            dap: true,
            indexer: true,
            project_root: None,
            language_servers,
            debug_adapters,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LanguageServerCommand {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub initialization_options: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DebugAdapterCommand {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}
