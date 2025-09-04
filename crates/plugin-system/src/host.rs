//! Host integration for plugin system
//!
//! This module provides the host interface for plugins to interact with the terminal

use std::sync::Arc;

use anyhow::Result as AnyResult;

use crate::api::{CommandOutput, PluginError};
use crate::permissions::PluginPermissions;

/// Host interface that plugins can call into
pub trait HostInterface: Send + Sync {
    /// Log a message from the plugin
    fn log(&self, level: LogLevel, message: &str);

    /// Read a file (subject to permissions)
    fn read_file(&self, path: &str) -> Result<Vec<u8>, PluginError>;

    /// Write a file (subject to permissions)
    fn write_file(&self, path: &str, data: &[u8]) -> Result<(), PluginError>;

    /// Execute a command (subject to permissions)
    fn execute_command(&self, command: &str) -> Result<CommandOutput, PluginError>;

    /// Get terminal state
    fn get_terminal_state(&self) -> TerminalState;

    /// Show a notification
    fn show_notification(&self, notification: Notification) -> Result<(), PluginError>;

    /// Store data persistently
    fn store_data(&self, key: &str, value: &[u8]) -> Result<(), PluginError>;

    /// Retrieve stored data
    fn retrieve_data(&self, key: &str) -> Result<Option<Vec<u8>>, PluginError>;
}

/// Log levels for plugin logging
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// Terminal state information
#[derive(Debug, Clone)]
pub struct TerminalState {
    pub current_dir: String,
    pub environment: std::collections::HashMap<String, String>,
    pub shell: String,
    pub terminal_size: (u16, u16),
    pub is_interactive: bool,
    pub command_history: Vec<String>,
}

/// Notification to display to the user
#[derive(Debug, Clone)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub icon: Option<String>,
}

/// Add host functions to WASM linker (placeholder)
#[cfg(feature = "wasm-runtime")]
pub fn add_host_functions(
    _linker: &mut wasmtime::Linker<crate::WasmPluginContext>,
    _host_interface: Option<Arc<dyn HostInterface>>,
) -> AnyResult<()> {
    // This would add all the host functions that plugins can call
    // For now, this is a placeholder
    Ok(())
}
