//! Host integration for plugin system
//!
//! This module provides the host interface for plugins to interact with the terminal

use std::sync::Arc;

use anyhow::{anyhow, Result as AnyResult};

use crate::api::{CommandOutput, PluginError};

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

#[cfg(feature = "wasm-runtime")]
fn read_caller_mem(
    caller: &mut wasmtime::Caller<crate::WasmPluginContext>,
    ptr: i32,
    len: i32,
) -> AnyResult<Vec<u8>> {
    let export = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| anyhow!("WASM module missing exported memory"))?;
    let mut buf = vec![0u8; len as usize];
    export
        .read(caller, ptr as usize, &mut buf)
        .map_err(|e| anyhow!("Failed to read guest memory: {e}"))?;
    Ok(buf)
}

/// Add host functions to WASM linker (minimal stable surface)
#[cfg(feature = "wasm-runtime")]
pub fn add_host_functions(
    linker: &mut wasmtime::Linker<crate::WasmPluginContext>,
    host_interface: Option<Arc<dyn HostInterface>>,
) -> AnyResult<()> {
    // host_log(level: i32, ptr: i32, len: i32) -> i32
    // level: 0=Debug,1=Info,2=Warning,3=Error
    let hi = host_interface.clone();
    linker.func_wrap(
        "host",
        "host_log",
        move |
              mut caller: wasmtime::Caller<crate::WasmPluginContext>,
              level: i32,
              ptr: i32,
              len: i32|
              -> i32 {
            let msg_bytes = match read_caller_mem(&mut caller, ptr, len) {
                Ok(b) => b,
                Err(_) => return -1,
            };
            let message = match String::from_utf8(msg_bytes) {
                Ok(s) => s,
                Err(e) => {
                    let s = String::from_utf8_lossy(e.as_bytes()).to_string();
                    s
                }
            };
            let level_map = match level {
                0 => LogLevel::Debug,
                1 => LogLevel::Info,
                2 => LogLevel::Warning,
                3 => LogLevel::Error,
                _ => LogLevel::Info,
            };
            if let Some(ref iface) = hi {
                iface.log(level_map, &message);
            }
            0
        },
    )?;

    // Future host functions (read_file/write_file/execute_command/etc.) can be added here with
    // stable signatures. Keeping the surface minimal ensures API stability for v1.

    Ok(())
}
