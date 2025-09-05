//! OpenAgent Terminal Plugin SDK
//!
//! This crate provides ergonomic tools for developing WebAssembly plugins for OpenAgent Terminal.
//! It includes macros for easy plugin definition and safe wrappers for host function calls.

use plugin_api::PluginMetadata;

pub use plugin_api;

// Re-export common types for convenience
pub use plugin_api::{
    CommandOutput, Completion, CompletionContext, CompletionKind, Context, ContextRequest,
    HookData, HookEvent, HookResponse, HookType, Plugin, PluginCapabilities, PluginConfig,
    PluginError, SensitivityLevel, TerminalInfo,
};

/// Global storage for plugin metadata (allocated in WASM linear memory)
static mut PLUGIN_METADATA_JSON: Option<Vec<u8>> = None;

/// Global event buffer for host-to-plugin data transfer
static mut PLUGIN_EVENT_BUFFER: Option<Vec<u8>> = None;

/// Global storage for the last plugin response payload (JSON)
static mut LAST_RESPONSE_JSON: Option<Vec<u8>> = None;

/// Result codes for plugin functions
pub mod result_codes {
    pub const SUCCESS: i32 = 0;
    pub const ERROR_GENERIC: i32 = -1;
    pub const ERROR_INVALID_INPUT: i32 = -2;
    pub const ERROR_PERMISSION_DENIED: i32 = -3;
    pub const ERROR_NOT_FOUND: i32 = -4;
    pub const ERROR_TIMEOUT: i32 = -5;
}

// Host function imports - these are provided by the runtime
extern "C" {
    // Log a message to the host
    fn host_log(level: i32, ptr: *const u8, len: usize);

    // Read a file from the host filesystem
    fn host_read_file(
        path_ptr: *const u8,
        path_len: usize,
        result_ptr: *mut u8,
        result_len_ptr: *mut u32,
    ) -> i32;

    // Write a file to the host filesystem
    fn host_write_file(
        path_ptr: *const u8,
        path_len: usize,
        data_ptr: *const u8,
        data_len: usize,
    ) -> i32;

    // Execute a command on the host; two-call variable-size result (JSON CommandOutput)
    fn host_execute_command(
        cmd_ptr: *const u8,
        cmd_len: usize,
        result_ptr: *mut u8,
        result_len_ptr: *mut u32,
    ) -> i32;

    // Persistent storage
    fn host_store_data(key_ptr: *const u8, key_len: usize, data_ptr: *const u8, data_len: usize) -> i32;
    fn host_retrieve_data(key_ptr: *const u8, key_len: usize, result_ptr: *mut u8, result_len_ptr: *mut u32) -> i32;
}

/// Safe wrapper for host logging
pub fn log(level: LogLevel, message: &str) {
    let level_int = match level {
        LogLevel::Debug => 0,
        LogLevel::Info => 1,
        LogLevel::Warning => 2,
        LogLevel::Error => 3,
    };

    unsafe {
        host_log(level_int, message.as_ptr(), message.len());
    }
}

/// Log levels for plugin logging
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// Safe wrapper for file reading
pub fn read_file(path: &str) -> Result<Vec<u8>, PluginError> {
    let mut result_len: u32 = 0;

    // First call to get the required buffer size
    let result = unsafe {
        host_read_file(path.as_ptr(), path.len(), std::ptr::null_mut(), &mut result_len as *mut u32)
    };

    match result {
        0 => {
            // Success, allocate buffer and read data
            let mut buffer = vec![0u8; result_len as usize];
            let read_result = unsafe {
                host_read_file(
                    path.as_ptr(),
                    path.len(),
                    buffer.as_mut_ptr(),
                    &mut result_len as *mut u32,
                )
            };

            if read_result == 0 {
                Ok(buffer)
            } else {
                Err(PluginError::IoError(std::io::Error::other("Failed to read file data")))
            }
        },
        -1 => Err(PluginError::PermissionDenied("File read not permitted".into())),
        -2 => Err(PluginError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        ))),
        _ => Err(PluginError::Unknown("Unknown file read error".into())),
    }
}

/// Safe wrapper for file writing
pub fn write_file(path: &str, data: &[u8]) -> Result<(), PluginError> {
    let result = unsafe { host_write_file(path.as_ptr(), path.len(), data.as_ptr(), data.len()) };

    match result {
        0 => Ok(()),
        -1 => Err(PluginError::PermissionDenied("File write not permitted".into())),
        -2 => Err(PluginError::IoError(std::io::Error::other("Failed to write file"))),
        _ => Err(PluginError::Unknown("Unknown file write error".into())),
    }
}

/// Safe wrapper for command execution
pub fn execute_command(command: &str) -> Result<CommandOutput, PluginError> {
    let mut len: u32 = 0;
    // First call: ask for required buffer size
    let rc = unsafe { host_execute_command(command.as_ptr(), command.len(), core::ptr::null_mut(), &mut len as *mut u32) };
    match rc {
        0 => {
            // Allocate buffer and fetch JSON payload
            let mut buf = vec![0u8; len as usize];
            let rc2 = unsafe { host_execute_command(command.as_ptr(), command.len(), buf.as_mut_ptr(), &mut len as *mut u32) };
            if rc2 == 0 {
                serde_json::from_slice::<CommandOutput>(&buf).map_err(PluginError::SerializationError)
            } else if rc2 == -2 {
                Err(PluginError::CommandFailed("Command execution failed".into()))
            } else if rc2 == -1 {
                Err(PluginError::PermissionDenied("Command execution not permitted".into()))
            } else if rc2 == -3 {
                Err(PluginError::Unknown("Host not available".into()))
            } else {
                Err(PluginError::Unknown("Unknown command execution error".into()))
            }
        }
        -1 => Err(PluginError::PermissionDenied("Command execution not permitted".into())),
        -2 => Err(PluginError::CommandFailed("Command execution failed".into())),
        -3 => Err(PluginError::Unknown("Host not available".into())),
        _ => Err(PluginError::Unknown("Unknown command execution error".into())),
    }
}

/// Persistent storage: store data by key
pub fn store_data(key: &str, data: &[u8]) -> Result<(), PluginError> {
    let rc = unsafe { host_store_data(key.as_ptr(), key.len(), data.as_ptr(), data.len()) };
    match rc {
        0 => Ok(()),
        -2 => Err(PluginError::IoError(std::io::Error::other("Store failed"))),
        _ => Err(PluginError::Unknown("Storage host unavailable".into())),
    }
}

/// Persistent storage: retrieve data by key
pub fn retrieve_data(key: &str) -> Result<Option<Vec<u8>>, PluginError> {
    let mut len: u32 = 0;
    let rc = unsafe { host_retrieve_data(key.as_ptr(), key.len(), std::ptr::null_mut(), &mut len as *mut u32) };
    match rc {
        0 => {
            let mut buf = vec![0u8; len as usize];
            let rc2 = unsafe { host_retrieve_data(key.as_ptr(), key.len(), buf.as_mut_ptr(), &mut len as *mut u32) };
            if rc2 == 0 { Ok(Some(buf)) } else { Err(PluginError::IoError(std::io::Error::other("Retrieve failed"))) }
        },
        -1 => Ok(None),
        -2 => Err(PluginError::IoError(std::io::Error::other("Retrieve failed"))),
        _ => Err(PluginError::Unknown("Storage host unavailable".into())),
    }
}

/// Set the plugin metadata (called during initialization)
pub fn set_plugin_metadata(metadata: &PluginMetadata) -> Result<(), PluginError> {
    let json = serde_json::to_vec(metadata).map_err(PluginError::SerializationError)?;

    unsafe {
        PLUGIN_METADATA_JSON = Some(json);
    }

    Ok(())
}

/// Export: Get plugin metadata as JSON in memory
#[no_mangle]
pub extern "C" fn plugin_get_metadata() -> i64 {
    unsafe {
        if let Some(ref json) = PLUGIN_METADATA_JSON {
            let ptr = json.as_ptr() as u32;
            let len = json.len() as u32;
            // Pack pointer and length into i64: high 32 bits = len, low 32 bits = ptr
            ((len as i64) << 32) | (ptr as i64)
        } else {
            0 // No metadata available
        }
    }
}

/// Export: Handle an event (placeholder implementation)
#[no_mangle]
pub extern "C" fn plugin_handle_event(ptr: i32, len: i32) -> i32 {
    // For now, just log that an event was received and set a default response
    log(LogLevel::Info, &format!("Received event at ptr={}, len={}", ptr, len));

    // Best-effort: capture a small preview to aid debugging
    unsafe {
        if ptr > 0 && len > 0 {
            let src = core::slice::from_raw_parts(ptr as *const u8, len as usize);
            let preview = if src.len() > 64 { &src[..64] } else { src };
            let mut msg = b"{\"status\":\"ok\",\"preview\":\"".to_vec();
            for &b in preview {
                // escape simple quotes for readability
                let ch = match b {
                    b'\\' => b"\\\\".to_vec(),
                    b'"' => b"\\\"".to_vec(),
                    _ => vec![b],
                };
                msg.extend_from_slice(&ch);
            }
            msg.extend_from_slice(b"\"}");
            LAST_RESPONSE_JSON = Some(msg);
        } else {
            LAST_RESPONSE_JSON = Some(b"{\"status\":\"ok\"}".to_vec());
        }
    }

    result_codes::SUCCESS
}

/// Export: Initialize plugin (default implementation)
#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
    log(LogLevel::Info, "Plugin initialized via SDK");
    result_codes::SUCCESS
}

/// Export: Allocate a buffer inside plugin linear memory for host to write into
#[no_mangle]
pub extern "C" fn plugin_alloc(size: i32) -> i32 {
    if size <= 0 { return 0; }
    let size = size as usize;
    unsafe {
        // Allocate or resize buffer; keep ownership to ensure pointer stability
        let buf = PLUGIN_EVENT_BUFFER.get_or_insert_with(|| vec![0u8; size]);
        if buf.len() < size {
            buf.resize(size, 0);
        }
        buf.as_mut_ptr() as i32
    }
}

/// Export: Retrieve the last response set by the plugin (packed ptr|len as i64)
#[no_mangle]
pub extern "C" fn plugin_get_last_response() -> i64 {
    unsafe {
        if let Some(ref json) = LAST_RESPONSE_JSON {
            let ptr = json.as_ptr() as u32;
            let len = json.len() as u32;
            ((len as i64) << 32) | (ptr as i64)
        } else {
            0
        }
    }
}

/// Helper: Set the last response from plugin code using a JSON string
pub fn set_last_response_str(json: &str) {
    unsafe {
        LAST_RESPONSE_JSON = Some(json.as_bytes().to_vec());
    }
}

/// Helper: Set the last response from a serializable value
pub fn set_last_response_json<T: serde::Serialize>(value: &T) -> Result<(), PluginError> {
    let bytes = serde_json::to_vec(value).map_err(PluginError::SerializationError)?;
    unsafe {
        LAST_RESPONSE_JSON = Some(bytes);
    }
    Ok(())
}

/// Export: Cleanup plugin (default implementation)
#[no_mangle]
pub extern "C" fn plugin_cleanup() -> i32 {
    log(LogLevel::Info, "Plugin cleaned up via SDK");
    result_codes::SUCCESS
}

/// Macro to define a plugin with automatic export generation
#[macro_export]
macro_rules! define_plugin {
    (
        name: $name:expr,
        version: $version:expr,
        author: $author:expr,
        description: $desc:expr,
        $(capabilities: { $($cap_key:ident: $cap_val:expr),* },)?
        $(permissions: { $($perm_key:ident: $perm_val:expr),* },)?
        init: $init_fn:expr,
        $(event_handler: $event_fn:expr,)?
        $(cleanup: $cleanup_fn:expr,)?
    ) => {
        use $crate::{PluginMetadata, PluginCapabilities, PluginPermissions, result_codes, log, LogLevel, set_plugin_metadata};

        // Plugin metadata
        static PLUGIN_META: std::sync::LazyLock<PluginMetadata> = std::sync::LazyLock::new(|| {
            PluginMetadata {
                name: $name.to_string(),
                version: $version.to_string(),
                author: $author.to_string(),
                description: $desc.to_string(),
                license: "MIT".to_string(),
                homepage: None,
                capabilities: PluginCapabilities {
                    $($(
                        $cap_key: $cap_val,
                    )*)?
                    ..Default::default()
                },
                permissions: PluginPermissions {
                    $($(
                        $perm_key: $perm_val,
                    )*)?
                    ..Default::default()
                },
            }
        });

        #[no_mangle]
        pub extern "C" fn plugin_init() -> i32 {
            // Set metadata for JSON ABI
            if let Err(e) = set_plugin_metadata(&PLUGIN_META) {
                log(LogLevel::Error, &format!("Failed to set plugin metadata: {:?}", e));
                return result_codes::ERROR_GENERIC;
            }

            // Call custom initialization
            match $init_fn() {
                Ok(()) => {
                    log(LogLevel::Info, &format!("Plugin '{}' initialized successfully", $name));
                    result_codes::SUCCESS
                },
                Err(e) => {
                    log(LogLevel::Error, &format!("Plugin '{}' initialization failed: {:?}", $name, e));
                    result_codes::ERROR_GENERIC
                }
            }
        }

        $(
            #[no_mangle]
            pub extern "C" fn plugin_handle_event(ptr: i32, len: i32) -> i32 {
                // Read event JSON from memory (simplified for now)
                log(LogLevel::Info, &format!("Handling event at ptr={}, len={}", ptr, len));

                // Call custom event handler
                match $event_fn(ptr, len) {
                    Ok(()) => result_codes::SUCCESS,
                    Err(e) => {
                        log(LogLevel::Error, &format!("Event handling failed: {:?}", e));
                        result_codes::ERROR_GENERIC
                    }
                }
            }
        )?

        $(
            #[no_mangle]
            pub extern "C" fn plugin_cleanup() -> i32 {
                match $cleanup_fn() {
                    Ok(()) => {
                        log(LogLevel::Info, &format!("Plugin '{}' cleaned up successfully", $name));
                        result_codes::SUCCESS
                    },
                    Err(e) => {
                        log(LogLevel::Error, &format!("Plugin '{}' cleanup failed: {:?}", $name, e));
                        result_codes::ERROR_GENERIC
                    }
                }
            }
        )?
    };
}

/// Convenience macro for simple plugins
#[macro_export]
macro_rules! simple_plugin {
    ($name:expr, $version:expr, $author:expr, $description:expr) => {
        $crate::define_plugin! {
            name: $name,
            version: $version,
            author: $author,
            description: $description,
            init: || Ok(()),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_result_codes() {
        assert_eq!(result_codes::SUCCESS, 0);
        assert_eq!(result_codes::ERROR_GENERIC, -1);
    }

    #[test]
    fn test_log_levels() {
        // Just ensure the enum compiles and works
        let level = LogLevel::Info;
        assert!(matches!(level, LogLevel::Info));
    }
}
