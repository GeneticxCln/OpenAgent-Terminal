// Plugin API - Core trait definitions and types for OpenAgent Terminal plugins
#![allow(clippy::pedantic, clippy::missing_errors_doc, clippy::doc_markdown)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plugin metadata describing capabilities and requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub license: String,
    pub homepage: Option<String>,
    pub capabilities: PluginCapabilities,
    pub permissions: PluginPermissions,
}

/// Capabilities that a plugin can provide
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginCapabilities {
    pub completions: bool,
    pub context_provider: bool,
    pub commands: Vec<String>,
    pub hooks: Vec<HookType>,
    pub file_associations: Vec<String>,
}

/// Security permissions required by the plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginPermissions {
    pub read_files: Vec<String>,  // Glob patterns for file access
    pub write_files: Vec<String>, // Glob patterns for write access
    pub network: bool,
    pub execute_commands: bool,
    pub environment_variables: Vec<String>,
    pub max_memory_mb: u32,
    pub timeout_ms: u64,
    /// Allow access to host-managed persistent storage APIs
    #[serde(default)]
    pub storage: bool,
}

impl Default for PluginPermissions {
    fn default() -> Self {
        Self {
            read_files: vec![],
            write_files: vec![],
            network: false,
            execute_commands: false,
            environment_variables: vec![],
            max_memory_mb: 50,
            timeout_ms: 5000,
            storage: false,
        }
    }
}

/// Hook types that plugins can register for
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HookType {
    PreCommand,
    PostCommand,
    PrePrompt,
    PostPrompt,
    DirectoryChange,
    SessionStart,
    SessionEnd,
}

/// Main plugin trait that all plugins must implement
pub trait Plugin: Send + Sync {
    /// Return plugin metadata
    fn metadata(&self) -> PluginMetadata;

    /// Initialize the plugin with configuration
    fn init(&mut self, config: PluginConfig) -> Result<(), PluginError>;

    /// Provide completions for the given input
    fn provide_completions(&self, context: CompletionContext) -> Vec<Completion>;

    /// Collect context for AI or other systems
    fn collect_context(&self, request: ContextRequest) -> Option<Context>;

    /// Execute a custom command provided by this plugin
    fn execute_command(&self, cmd: &str, args: &[String]) -> Result<CommandOutput, PluginError>;

    /// Handle a hook event
    fn handle_hook(&mut self, hook: HookEvent) -> Result<HookResponse, PluginError>;

    /// Cleanup when plugin is being unloaded
    fn cleanup(&mut self) -> Result<(), PluginError>;
}

/// Configuration passed to plugin during initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub settings: HashMap<String, serde_json::Value>,
    pub terminal_info: TerminalInfo,
    pub plugin_dir: String,
}

/// Information about the terminal environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub shell: String,
    pub home_dir: String,
    pub current_dir: String,
}

/// Context for completion requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionContext {
    pub input: String,
    pub cursor_position: usize,
    pub current_dir: String,
    pub environment: HashMap<String, String>,
    pub command_history: Vec<String>,
}

/// A single completion suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Completion {
    pub value: String,
    pub display: String,
    pub description: Option<String>,
    pub kind: CompletionKind,
    pub score: f32,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompletionKind {
    Command,
    File,
    Directory,
    Argument,
    Option,
    Variable,
    Custom(String),
}

/// Request for context collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRequest {
    pub purpose: String,
    pub max_size_bytes: usize,
    pub include_sensitive: bool,
    pub filters: Vec<String>,
}

/// Context data returned by plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    pub name: String,
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub sensitivity: SensitivityLevel,
    pub size_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensitivityLevel {
    Public,
    Internal,
    Confidential,
    Secret,
}

/// Output from command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub execution_time_ms: u64,
}

/// Hook event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEvent {
    pub hook_type: HookType,
    pub data: HookData,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HookData {
    Command { cmd: String, args: Vec<String> },
    DirectoryChange { from: String, to: String },
    Session { action: String },
    Custom(HashMap<String, serde_json::Value>),
}

/// Response from hook handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResponse {
    pub modified_command: Option<String>,
    pub prevent_execution: bool,
    pub messages: Vec<String>,
}

/// Plugin error types
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Backwards-compat variant used by some plugins; alias of CommandFailed
    #[error("Command error: {0}")]
    CommandError(String),

    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Initialization failed: {0}")]
    InitError(String),

    #[error("Timeout exceeded")]
    Timeout,

    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}
/// Helper macro for creating plugin metadata
#[macro_export]
macro_rules! plugin_metadata {
    (
        name: $name:expr,
        version: $version:expr,
        author: $author:expr,
        description: $desc:expr,
        capabilities: { $($cap_key:ident: $cap_val:expr),* },
        permissions: { $($perm_key:ident: $perm_val:expr),* }
    ) => {
        PluginMetadata {
            name: $name.to_string(),
            version: $version.to_string(),
            author: $author.to_string(),
            description: $desc.to_string(),
            license: "MIT".to_string(),
            homepage: None,
            capabilities: PluginCapabilities {
                $($cap_key: $cap_val,)*
                ..Default::default()
            },
            permissions: PluginPermissions {
                $($perm_key: $perm_val,)*
                ..Default::default()
            },
        }
    };
}

/// Entry point for WASI plugins
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn _start() {
    // WASI entry point - plugins should override this
}

/// Plugin registration for native plugins
#[macro_export]
macro_rules! register_plugin {
    ($plugin_type:ty) => {
        #[no_mangle]
        pub extern "C" fn create_plugin() -> Box<dyn Plugin> {
            Box::new(<$plugin_type>::new())
        }

        #[no_mangle]
        pub extern "C" fn plugin_api_version() -> &'static str {
            "1.0.0"
        }
    };
}
