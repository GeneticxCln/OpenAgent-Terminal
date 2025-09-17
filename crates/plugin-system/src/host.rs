//! Host integration for plugin system
//!
//! This module provides the host interface for plugins to interact with the terminal

#[cfg(feature = "wasm-runtime")]
use std::sync::Arc;

#[cfg(feature = "wasm-runtime")]
use anyhow::{anyhow, Result as AnyResult};

use crate::api::{CommandOutput, PluginError};
#[cfg(feature = "wasm-runtime")]
use serde_json;
#[cfg(feature = "wasm-runtime")]
use std::time::Duration;

/// Host interface that plugins can call into
pub trait HostInterface: Send + Sync {
    /// Log a message from the plugin
    fn log(&self, level: LogLevel, message: &str);

    /// Read a file (subject to permissions)
    fn read_file(&self, path: &str) -> Result<Vec<u8>, PluginError>;

    /// Write a file (subject to permissions)
    fn write_file(&self, path: &str, data: &[u8]) -> Result<(), PluginError>;

    /// Execute a command (subject to permissions). Convenience for simple commands.
    fn execute_command(&self, command: &str) -> Result<CommandOutput, PluginError>;

    /// Execute a command with args and cwd (policy-gated spawn)
    fn spawn(&self, cmd: &str, args: &[String], cwd: Option<&str>) -> Result<CommandOutput, PluginError>;

    /// Network fetch with policy enforcement (domain/method/timeout/size caps)
    fn net_fetch(&self, req: NetRequest) -> Result<NetResponse, PluginError>;

    /// Get terminal state
    fn get_terminal_state(&self) -> TerminalState;

    /// Show a notification
    fn show_notification(&self, notification: Notification) -> Result<(), PluginError>;

    /// Store data persistently
    fn store_data(&self, key: &str, value: &[u8]) -> Result<(), PluginError>;

    /// Retrieve stored data
    fn retrieve_data(&self, key: &str) -> Result<Option<Vec<u8>>, PluginError>;
}

#[derive(Debug, Clone)]
pub struct NetRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub timeout_ms: Option<u64>,
    pub max_response_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct NetResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
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
    // Helper to allocate guest memory and write bytes, returning packed (ptr,len) as i64
    fn alloc_and_write_packed(
        caller: &mut wasmtime::Caller<crate::WasmPluginContext>,
        bytes: &[u8],
    ) -> Result<i64, anyhow::Error> {
        // Resolve memory
        let memory = caller
            .get_export("memory")
            .and_then(|e| e.into_memory())
            .ok_or_else(|| anyhow!("WASM module missing exported memory"))?;
        // Resolve plugin_alloc
        let func = caller
            .get_export("plugin_alloc")
            .and_then(|e| e.into_func())
            .ok_or_else(|| anyhow!("WASM module missing plugin_alloc export"))?;
        let alloc = func.typed::<i32, i32>(&mut *caller)?;
        let ptr = alloc.call(&mut *caller, bytes.len() as i32)? as u32;
        // Write bytes
        memory.write(&mut *caller, ptr as usize, bytes)?;
        let len = bytes.len() as u32;
        // Pack ptr:len into i64 (low 32 = ptr, high 32 = len)
        let packed: i64 = ((len as u64) << 32 | (ptr as u64)) as i64;
        Ok(packed)
    }
    // host_log(level: i32, ptr: i32, len: i32) -> i32
    // level: 0=Debug,1=Info,2=Warning,3=Error
    let hi = host_interface.clone();
    linker.func_wrap(
        "host",
        "host_log",
        move |mut caller: wasmtime::Caller<crate::WasmPluginContext>,
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

    // host_read_file(ptr,len) -> packed i64 (ptr:len) of result bytes
    let hi = host_interface.clone();
    linker.func_wrap(
        "host",
        "host_read_file",
        move |mut caller: wasmtime::Caller<crate::WasmPluginContext>, ptr: i32, len: i32| -> i64 {
            let path_bytes = match read_caller_mem(&mut caller, ptr, len) { Ok(b) => b, Err(_) => return -1 };
            let path = match String::from_utf8(path_bytes) { Ok(s) => s, Err(_) => return -2 };
            let result = if let Some(ref iface) = hi { iface.read_file(&path) } else { Err(PluginError::Internal("No host".into())) };
            match result {
                Ok(bytes) => match alloc_and_write_packed(&mut caller, &bytes) {
                    Ok(packed) => packed,
                    Err(_) => -6,
                },
                Err(e) => {
                    let msg = format!("{{\"error\":\"{}\"}}", e);
                    match alloc_and_write_packed(&mut caller, msg.as_bytes()) {
                        Ok(packed) => packed,
                        Err(_) => -7,
                    }
                }
            }
        },
    )?;

    // host_net_fetch(req_json_ptr,len) -> i64 packed
    let hi = host_interface.clone();
    linker.func_wrap(
        "host",
        "host_net_fetch",
        move |mut caller: wasmtime::Caller<crate::WasmPluginContext>, ptr: i32, len: i32| -> i64 {
            let req_bytes = match read_caller_mem(&mut caller, ptr, len) { Ok(b) => b, Err(_) => return -1 };
            // Expect JSON-encoded NetRequest; parse manually to avoid derive requirements
            let mut req = NetRequest { url: String::new(), method: "GET".into(), headers: Vec::new(), body: None, timeout_ms: None, max_response_bytes: None };
            match serde_json::from_slice::<serde_json::Value>(&req_bytes) {
                Ok(v) => {
                    if let Some(u) = v.get("url").and_then(|x| x.as_str()) { req.url = u.to_string(); }
                    if let Some(m) = v.get("method").and_then(|x| x.as_str()) { req.method = m.to_string(); }
                    if let Some(h) = v.get("headers").and_then(|x| x.as_array()) {
                        for pair in h {
                            if let Some(arr) = pair.as_array() {
                                if arr.len() == 2 {
                                    if let (Some(k), Some(val)) = (arr[0].as_str(), arr[1].as_str()) {
                                        req.headers.push((k.to_string(), val.to_string()));
                                    }
                                }
                            }
                        }
                    }
                    if let Some(b) = v.get("body") {
                        if b.is_string() { req.body = Some(b.as_str().unwrap().as_bytes().to_vec()); }
                        else if b.is_array() { req.body = Some(serde_json::to_vec(b).unwrap_or_default()); }
                        else if b.is_object() { req.body = Some(serde_json::to_vec(b).unwrap_or_default()); }
                        else if b.is_null() { req.body = None; }
                    }
                    if let Some(t) = v.get("timeout_ms").and_then(|x| x.as_u64()) { req.timeout_ms = Some(t); }
                    if let Some(mx) = v.get("max_response_bytes").and_then(|x| x.as_u64()) { req.max_response_bytes = Some(mx); }
                }
                Err(_) => return -2,
            };
            let result = if let Some(ref iface) = hi { iface.net_fetch(req) } else { Err(PluginError::Internal("No host".into())) };
            let resp_json = match result {
                Ok(resp) => match serde_json::to_vec(&resp_json_from(resp)) { Ok(b) => b, Err(_) => return -3 },
                Err(e) => format!("{{\"error\":\"{}\"}}", e).into_bytes(),
            };
            match alloc_and_write_packed(&mut caller, &resp_json) {
                Ok(packed) => packed,
                Err(_) => -6,
            }
        },
    )?;

    // host_spawn(cmd_json_ptr,len) -> i64 packed
    let hi = host_interface.clone();
    linker.func_wrap(
        "host",
        "host_spawn",
        move |mut caller: wasmtime::Caller<crate::WasmPluginContext>, ptr: i32, len: i32| -> i64 {
            #[derive(serde::Deserialize)]
            struct SpawnReq { cmd: String, args: Vec<String>, cwd: Option<String> }
            let req_bytes = match read_caller_mem(&mut caller, ptr, len) { Ok(b) => b, Err(_) => return -1 };
            let req: SpawnReq = match serde_json::from_slice(&req_bytes) { Ok(r) => r, Err(_) => return -2 };
            let result = if let Some(ref iface) = hi { iface.spawn(&req.cmd, &req.args, req.cwd.as_deref()) } else { Err(PluginError::Internal("No host".into())) };
            let out_json = match result {
                Ok(out) => match serde_json::to_vec(&out) { Ok(b) => b, Err(_) => return -3 },
                Err(e) => format!("{{\"error\":\"{}\"}}", e).into_bytes(),
            };
            match alloc_and_write_packed(&mut caller, &out_json) {
                Ok(packed) => packed,
                Err(_) => -6,
            }
        },
    )?;

    // Helper: convert NetResponse to a JSON-friendly struct (avoids adding base64 dep)
    fn resp_json_from(resp: NetResponse) -> serde_json::Value {
        serde_json::json!({
            "status": resp.status,
            "headers": resp.headers,
            "body": resp.body, // raw bytes array
        })
    }

    Ok(())
}
