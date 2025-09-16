//! Plugin runtime management
//!
//! This module provides runtime management for plugins.
//! It implements a concrete WASM instance runner that drives the plugin
//! lifecycle (init/cleanup) and command execution via the unified ABI
//! defined in `lib.rs`.

#[cfg(feature = "wasm-runtime")]
use std::sync::Arc;

#[cfg(feature = "wasm-runtime")]
use crate::{
    api::{CommandOutput, Completion, Context, ContextRequest, HookEvent, HookResponse},
    PluginAbi, PluginSystemError,
};

// Re-export from main lib for now
pub use crate::UnifiedPluginManager as PluginManager;

/// Plugin runtime interface (placeholder)
///
/// Note: This trait is intentionally minimal and synchronous for now to avoid
/// introducing new async trait dependencies. Concrete runtimes in this module
/// expose async methods directly.
pub trait PluginRuntime {
    fn start(&mut self) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<()>;
}

// ===== Concrete WASM instance runner =====

#[cfg(feature = "wasm-runtime")]
use crate::{LoadedPlugin, WasmPluginContext, WasmPluginData};

/// A lightweight runtime handle that operates on a single WASM plugin instance.
#[cfg(feature = "wasm-runtime")]
pub struct WasmInstanceRuntime {
    _plugin_id: String,
    plugin: Arc<LoadedPlugin>,
}

#[cfg(feature = "wasm-runtime")]
impl WasmInstanceRuntime {
    /// Create a runtime handle for a loaded plugin
    pub fn new(plugin_id: impl Into<String>, plugin: Arc<LoadedPlugin>) -> Self {
        Self {
            _plugin_id: plugin_id.into(),
            plugin,
        }
    }

    /// Initialize the plugin via ABI (if available)
    pub async fn start(&self) -> Result<(), PluginSystemError> {
        let wasm = self
            .plugin
            .wasm_data
            .as_ref()
            .ok_or_else(|| PluginSystemError::Runtime("WASM data not available".into()))?;

        if let Some(init_fn) = &self.plugin.abi.init {
            let mut store = wasm.store.lock().await;
            // Optionally set a deadline if the engine is configured for epoch-based timeouts.
            store.set_epoch_deadline(1000);
            let res = init_fn
                .call(&mut *store, ())
                .map_err(|e| PluginSystemError::Runtime(e.to_string()));
            store.set_epoch_deadline(u64::MAX);
            res.map(|_| ())
        } else {
            Ok(())
        }
    }

    /// Cleanup the plugin via ABI (if available)
    pub async fn stop(&self) -> Result<(), PluginSystemError> {
        let wasm = self
            .plugin
            .wasm_data
            .as_ref()
            .ok_or_else(|| PluginSystemError::Runtime("WASM data not available".into()))?;

        if let Some(cleanup_fn) = &self.plugin.abi.cleanup {
            let mut store = wasm.store.lock().await;
            cleanup_fn
                .call(&mut *store, ())
                .map_err(|e| PluginSystemError::Runtime(e.to_string()))
                .map(|_| ())
        } else {
            Ok(())
        }
    }

    /// Execute a command using the plugin ABI. Automatically falls back between
    /// the extended and basic command entrypoints, and returns a structured
    /// CommandOutput parsed from the plugin's last response.
    pub async fn execute_command(
        &self,
        command: &str,
        args: &[String],
    ) -> Result<CommandOutput, PluginSystemError> {
        let wasm = self
            .plugin
            .wasm_data
            .as_ref()
            .ok_or_else(|| PluginSystemError::Runtime("WASM data not available".into()))?;

        execute_wasm_command_internal(&self.plugin.abi, wasm, command, args).await
    }
}

#[cfg(feature = "wasm-runtime")]
fn unpack_ptr_len(packed: i64) -> (u32, u32) {
    let ptr = (packed & 0xFFFF_FFFF) as u32;
    let len = (packed >> 32) as u32;
    (ptr, len)
}

#[cfg(feature = "wasm-runtime")]
fn get_memory(
    instance: &wasmtime::Instance,
    store: &mut wasmtime::Store<WasmPluginContext>,
) -> Result<wasmtime::Memory, PluginSystemError> {
    instance
        .get_export(&mut *store, "memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| PluginSystemError::Runtime("Plugin missing memory export".to_string()))
}

#[cfg(feature = "wasm-runtime")]
fn alloc_and_write(
    abi: &PluginAbi,
    instance: &wasmtime::Instance,
    store: &mut wasmtime::Store<WasmPluginContext>,
    bytes: &[u8],
) -> Result<u32, PluginSystemError> {
    let alloc = abi
        .alloc
        .as_ref()
        .ok_or_else(|| PluginSystemError::Abi("plugin_alloc not exported".into()))?;

    let ptr = alloc
        .call(&mut *store, bytes.len() as i32)
        .map_err(|e| PluginSystemError::Runtime(e.to_string()))? as u32;

    let memory = get_memory(instance, store)?;
    memory
        .write(&mut *store, ptr as usize, bytes)
        .map_err(|_| PluginSystemError::Runtime("Failed to write to plugin memory".into()))?;

    Ok(ptr)
}

#[cfg(feature = "wasm-runtime")]
fn read_from_memory(
    instance: &wasmtime::Instance,
    store: &mut wasmtime::Store<WasmPluginContext>,
    ptr: u32,
    len: u32,
) -> Result<Vec<u8>, PluginSystemError> {
    let memory = get_memory(instance, store)?;
    let mut buffer = vec![0u8; len as usize];
    memory
        .read(&mut *store, ptr as usize, &mut buffer)
        .map_err(|_| PluginSystemError::Runtime("Failed to read from plugin memory".into()))?;
    Ok(buffer)
}

#[cfg(feature = "wasm-runtime")]
fn dealloc_if_available(
    abi: &PluginAbi,
    store: &mut wasmtime::Store<WasmPluginContext>,
    ptr: u32,
    len: u32,
) {
    if let Some(dealloc) = &abi.dealloc {
        let _ = dealloc.call(&mut *store, (ptr as i32, len as i32));
    }
}

/// Execute command on a WASM plugin instance using the provided ABI and data.
#[cfg(feature = "wasm-runtime")]
pub async fn execute_wasm_command_internal(
    abi: &PluginAbi,
    wasm: &WasmPluginData,
    command: &str,
    args: &[String],
) -> Result<CommandOutput, PluginSystemError> {
    let mut store = wasm.store.lock().await;
    let instance = &wasm.instance;

    // Prepare inputs
    let cmd_bytes = command.as_bytes();
    let args_json =
        serde_json::to_vec(args).map_err(|e| PluginSystemError::Runtime(e.to_string()))?;

    let cmd_ptr = alloc_and_write(abi, instance, &mut store, cmd_bytes)?;
    let args_ptr = if abi.execute_command_ex.is_some() {
        alloc_and_write(abi, instance, &mut store, &args_json)?
    } else {
        0
    };

    let exit_code: i32;

    if let Some(exec_ex) = &abi.execute_command_ex {
        // Call extended command: (cmd_ptr, cmd_len, args_ptr, args_len, r0, r1, r2)
        exit_code = exec_ex
            .call(
                &mut *store,
                (
                    cmd_ptr as i32,
                    cmd_bytes.len() as i32,
                    args_ptr as i32,
                    args_json.len() as i32,
                    0,
                    0,
                    0,
                ),
            )
            .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
    } else if let Some(exec) = &abi.execute_command {
        // Fallback: (cmd_ptr, cmd_len, result_ptr) -> i32
        exit_code = exec
            .call(&mut *store, (cmd_ptr as i32, cmd_bytes.len() as i32, 0))
            .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
    } else {
        dealloc_if_available(abi, &mut store, cmd_ptr, cmd_bytes.len() as u32);
        if args_ptr != 0 {
            dealloc_if_available(abi, &mut store, args_ptr, args_json.len() as u32);
        }
        return Err(PluginSystemError::Abi(
            "Plugin missing execute_command(_ex) export".into(),
        ));
    }

    // Free input buffers if dealloc is available
    dealloc_if_available(abi, &mut store, cmd_ptr, cmd_bytes.len() as u32);
    if args_ptr != 0 {
        dealloc_if_available(abi, &mut store, args_ptr, args_json.len() as u32);
    }

    if exit_code != 0 {
        // Try to retrieve error details
        if let Some(get_err) = &abi.get_error_message {
            let packed = get_err
                .call(&mut *store, ())
                .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
            let (ptr, len) = unpack_ptr_len(packed);
            if ptr != 0 && len != 0 {
                let bytes = read_from_memory(instance, &mut store, ptr, len)?;
                let _ = String::from_utf8(bytes.clone());
                // Try best-effort to free error buffer
                dealloc_if_available(abi, &mut store, ptr, len);
                return Err(PluginSystemError::Runtime(
                    String::from_utf8(bytes).unwrap_or_else(|_| "Plugin command failed".into()),
                ));
            }
        }
        return Err(PluginSystemError::Runtime(format!(
            "Plugin command failed with code {}",
            exit_code
        )));
    }

    // Retrieve last response
    if let Some(get_last) = &abi.get_last_response {
        let packed = get_last
            .call(&mut *store, ())
            .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
        let (ptr, len) = unpack_ptr_len(packed);
        if ptr == 0 || len == 0 {
            return Err(PluginSystemError::Runtime(
                "Invalid response pointer/length".into(),
            ));
        }
        let bytes = read_from_memory(instance, &mut store, ptr, len)?;
        // Try best-effort to free response buffer
        dealloc_if_available(abi, &mut store, ptr, len);

        // Parse into standardized output
        let output: CommandOutput =
            serde_json::from_slice(&bytes).map_err(PluginSystemError::Serialization)?;
        return Ok(output);
    }

    // If no response function, return a generic success
    Ok(CommandOutput {
        stdout: "".into(),
        stderr: "".into(),
        exit_code: 0,
        execution_time_ms: 0,
    })
}

#[cfg(feature = "wasm-runtime")]
pub async fn provide_completions_internal(
    abi: &PluginAbi,
    wasm: &WasmPluginData,
    context_json: &[u8],
) -> Result<Vec<Completion>, PluginSystemError> {
    let mut store = wasm.store.lock().await;
    let instance = &wasm.instance;

    let ctx_ptr = alloc_and_write(abi, instance, &mut store, context_json)?;

    if let Some(provide) = &abi.provide_completions {
        let rc = provide
            .call(&mut *store, (ctx_ptr as i32, context_json.len() as i32, 0))
            .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
        dealloc_if_available(abi, &mut store, ctx_ptr, context_json.len() as u32);
        if rc != 0 {
            return Err(PluginSystemError::Runtime(format!(
                "provide_completions failed with code {}",
                rc
            )));
        }
    } else {
        dealloc_if_available(abi, &mut store, ctx_ptr, context_json.len() as u32);
        return Ok(vec![]);
    }

    if let Some(get_last) = &abi.get_last_response {
        let packed = get_last
            .call(&mut *store, ())
            .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
        let (ptr, len) = unpack_ptr_len(packed);
        if ptr == 0 || len == 0 {
            return Ok(vec![]);
        }
        let bytes = read_from_memory(instance, &mut store, ptr, len)?;
        dealloc_if_available(abi, &mut store, ptr, len);
        let completions: Vec<Completion> =
            serde_json::from_slice(&bytes).map_err(PluginSystemError::Serialization)?;
        return Ok(completions);
    }

    Ok(vec![])
}

#[cfg(feature = "wasm-runtime")]
pub async fn collect_context_internal(
    abi: &PluginAbi,
    wasm: &WasmPluginData,
    request: &ContextRequest,
) -> Result<Option<Context>, PluginSystemError> {
    let mut store = wasm.store.lock().await;
    let instance = &wasm.instance;

    if let Some(collect) = &abi.collect_context {
        let req_bytes =
            serde_json::to_vec(request).map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
        let req_ptr = alloc_and_write(abi, instance, &mut store, &req_bytes)?;
        let rc = collect
            .call(&mut *store, (req_ptr as i32, req_bytes.len() as i32, 0))
            .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
        dealloc_if_available(abi, &mut store, req_ptr, req_bytes.len() as u32);
        if rc != 0 {
            return Err(PluginSystemError::Runtime(format!(
                "collect_context failed with code {}",
                rc
            )));
        }
    } else {
        return Ok(None);
    }

    if let Some(get_last) = &abi.get_last_response {
        let packed = get_last
            .call(&mut *store, ())
            .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
        let (ptr, len) = unpack_ptr_len(packed);
        if ptr == 0 || len == 0 {
            return Ok(None);
        }
        let bytes = read_from_memory(instance, &mut store, ptr, len)?;
        dealloc_if_available(abi, &mut store, ptr, len);
        let context: Context =
            serde_json::from_slice(&bytes).map_err(PluginSystemError::Serialization)?;
        return Ok(Some(context));
    }

    Ok(None)
}

#[cfg(feature = "wasm-runtime")]
pub async fn handle_event_internal(
    abi: &PluginAbi,
    wasm: &WasmPluginData,
    event: &HookEvent,
) -> Result<HookResponse, PluginSystemError> {
    let mut store = wasm.store.lock().await;
    let instance = &wasm.instance;

    if let Some(handle) = &abi.handle_event {
        let evt_bytes =
            serde_json::to_vec(event).map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
        let evt_ptr = alloc_and_write(abi, instance, &mut store, &evt_bytes)?;
        let rc = handle
            .call(&mut *store, (evt_ptr as i32, evt_bytes.len() as i32))
            .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
        dealloc_if_available(abi, &mut store, evt_ptr, evt_bytes.len() as u32);
        if rc != 0 {
            return Err(PluginSystemError::Runtime(format!(
                "handle_event failed with code {}",
                rc
            )));
        }
    } else {
        return Ok(HookResponse {
            modified_command: None,
            prevent_execution: false,
            messages: vec![],
        });
    }

    if let Some(get_last) = &abi.get_last_response {
        let packed = get_last
            .call(&mut *store, ())
            .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
        let (ptr, len) = unpack_ptr_len(packed);
        if ptr == 0 || len == 0 {
            return Ok(HookResponse {
                modified_command: None,
                prevent_execution: false,
                messages: vec![],
            });
        }
        let bytes = read_from_memory(instance, &mut store, ptr, len)?;
        dealloc_if_available(abi, &mut store, ptr, len);
        let resp: HookResponse =
            serde_json::from_slice(&bytes).map_err(PluginSystemError::Serialization)?;
        return Ok(resp);
    }

    Ok(HookResponse {
        modified_command: None,
        prevent_execution: false,
        messages: vec![],
    })
}
