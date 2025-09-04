//! Unified Plugin API definitions
//!
//! This module provides standardized types and interfaces for plugin communication

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plugin error types (unified from both systems)
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

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub settings: HashMap<String, serde_json::Value>,
    pub terminal_info: TerminalInfo,
    pub plugin_dir: String,
}

/// Terminal environment information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub shell: String,
    pub home_dir: String,
    pub current_dir: String,
}

/// Command execution output with standardized structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub execution_time_ms: u64,
}

/// Completion suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Completion {
    pub value: String,
    pub display: String,
    pub description: Option<String>,
    pub kind: CompletionKind,
    pub score: f32,
    pub icon: Option<String>,
}

/// Completion types
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

/// Context data for AI or other systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    pub name: String,
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub sensitivity: SensitivityLevel,
    pub size_bytes: usize,
}

/// Context collection request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRequest {
    pub purpose: String,
    pub max_size_bytes: usize,
    pub include_sensitive: bool,
    pub filters: Vec<String>,
}

/// Data sensitivity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensitivityLevel {
    Public,
    Internal,
    Confidential,
    Secret,
}

/// Hook event types
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

/// Hook event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEvent {
    pub hook_type: HookType,
    pub data: HookData,
    pub timestamp: u64,
}

/// Event data payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HookData {
    Command { cmd: String, args: Vec<String> },
    DirectoryChange { from: String, to: String },
    Session { action: String },
    Custom(HashMap<String, serde_json::Value>),
}

/// Hook response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResponse {
    pub modified_command: Option<String>,
    pub prevent_execution: bool,
    pub messages: Vec<String>,
}

/// Unified plugin trait (for native plugins)
pub trait Plugin: Send + Sync {
    /// Return plugin metadata
    fn metadata(&self) -> crate::PluginMetadata;

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

/// Context for completion requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionContext {
    pub input: String,
    pub cursor_position: usize,
    pub current_dir: String,
    pub environment: HashMap<String, String>,
    pub command_history: Vec<String>,
}

/// Helper macros for plugin metadata creation
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
        $crate::PluginMetadata {
            id: $name.to_string(),
            name: $name.to_string(),
            version: $version.to_string(),
            author: $author.to_string(),
            description: $desc.to_string(),
            license: "MIT".to_string(),
            homepage: None,
            capabilities: $crate::PluginCapabilities {
                $($cap_key: $cap_val,)*
                ..Default::default()
            },
            permissions: $crate::permissions::PluginPermissions {
                $($perm_key: $perm_val,)*
                ..Default::default()
            },
            abi_version: "1.0.0".to_string(),
            min_host_version: None,
        }
    };
}

/// Plugin registration macro for native plugins
#[macro_export]
macro_rules! register_plugin {
    ($plugin_type:ty) => {
        #[no_mangle]
        pub extern "C" fn create_plugin() -> Box<dyn $crate::api::Plugin> {
            Box::new(<$plugin_type>::new())
        }

        #[no_mangle]
        pub extern "C" fn plugin_abi_version() -> &'static str {
            "1.0.0"
        }
    };
}
