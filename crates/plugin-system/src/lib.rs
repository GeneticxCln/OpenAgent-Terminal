//! Unified Plugin System for OpenAgent Terminal
//!
//! This module provides a complete plugin system that combines:
//! - Standardized plugin API and metadata
//! - Secure WASM runtime with sandboxing
//! - Host integration capabilities
//! - Unified ABI for command execution and data exchange

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result as AnyResult;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{error, info};

// Re-export core types
pub use api::{
    CommandOutput, Completion, CompletionKind, Context, ContextRequest, HookEvent, HookResponse,
    HookType, PluginConfig, PluginError, SensitivityLevel,
};
pub use permissions::{PluginPermissions, SecurityPolicy};
pub use runtime::{PluginManager, PluginRuntime};

pub mod api;
pub mod host;
pub mod permissions;
pub mod runtime;

/// Alias to simplify complex wasmtime typed function signatures in the public API
pub type ExecCommandExFn = wasmtime::TypedFunc<(i32, i32, i32, i32, i32, i32, i32), i32>;

/// Plugin system error types
#[derive(Debug, Error)]
pub enum PluginSystemError {
    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid plugin format: {0}")]
    InvalidFormat(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("ABI error: {0}")]
    Abi(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Plugin metadata with standardized structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin identification
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub license: String,
    pub homepage: Option<String>,

    /// Plugin capabilities
    pub capabilities: PluginCapabilities,

    /// Security requirements
    pub permissions: PluginPermissions,

    /// ABI version compatibility
    pub abi_version: String,

    /// Minimum host version required
    pub min_host_version: Option<String>,
}

impl Default for PluginMetadata {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: "0.0.0".to_string(),
            author: String::new(),
            description: String::new(),
            license: "unknown".to_string(),
            homepage: None,
            capabilities: PluginCapabilities::default(),
            permissions: PluginPermissions::default(),
            abi_version: "1.0.0".to_string(),
            min_host_version: None,
        }
    }
}

/// Plugin capabilities definition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginCapabilities {
    /// Provides command completions
    pub completions: bool,

    /// Provides context for AI or other systems
    pub context_provider: bool,

    /// Custom commands provided
    pub commands: Vec<String>,

    /// Event hooks supported
    pub hooks: Vec<HookType>,

    /// File type associations
    pub file_associations: Vec<String>,

    /// Background services
    pub services: Vec<String>,
}

/// Standardized ABI for plugin communication
pub struct PluginAbi {
    /// Memory management functions
    pub alloc: Option<wasmtime::TypedFunc<i32, i32>>,
    pub dealloc: Option<wasmtime::TypedFunc<(i32, i32), ()>>,

    /// Core plugin lifecycle
    pub init: Option<wasmtime::TypedFunc<(), i32>>,
    pub cleanup: Option<wasmtime::TypedFunc<(), i32>>,

    /// Metadata and introspection
    pub get_metadata: Option<wasmtime::TypedFunc<(), i64>>, // ptr:len packed
    pub get_capabilities: Option<wasmtime::TypedFunc<(), i64>>,

    /// Command execution
    pub execute_command: Option<wasmtime::TypedFunc<(i32, i32, i32), i32>>, /* cmd_ptr, cmd_len,
                                                                             * result_ptr */
    pub execute_command_ex: Option<ExecCommandExFn>,

    /// Event handling
    pub handle_event: Option<wasmtime::TypedFunc<(i32, i32), i32>>,

    /// Context and completions
    pub provide_completions: Option<wasmtime::TypedFunc<(i32, i32, i32), i32>>,
    pub collect_context: Option<wasmtime::TypedFunc<(i32, i32, i32), i32>>,

    /// Response retrieval
    pub get_last_response: Option<wasmtime::TypedFunc<(), i64>>,
    pub get_error_message: Option<wasmtime::TypedFunc<(), i64>>,
}

impl PluginAbi {
    /// Extract ABI functions from a WASM instance
    #[cfg(feature = "wasm-runtime")]
    pub fn from_instance(
        instance: &wasmtime::Instance,
        store: &mut wasmtime::Store<impl Send>,
    ) -> Result<Self, PluginSystemError> {
        Ok(Self {
            // Memory management
            alloc: instance.get_typed_func(&mut *store, "plugin_alloc").ok(),
            dealloc: instance.get_typed_func(&mut *store, "plugin_dealloc").ok(),

            // Lifecycle
            init: instance.get_typed_func(&mut *store, "plugin_init").ok(),
            cleanup: instance.get_typed_func(&mut *store, "plugin_cleanup").ok(),

            // Metadata
            get_metadata: instance
                .get_typed_func(&mut *store, "plugin_get_metadata")
                .ok(),
            get_capabilities: instance
                .get_typed_func(&mut *store, "plugin_get_capabilities")
                .ok(),

            // Commands
            execute_command: instance
                .get_typed_func(&mut *store, "plugin_execute_command")
                .ok(),
            execute_command_ex: instance
                .get_typed_func(&mut *store, "plugin_execute_command_ex")
                .ok(),

            // Events
            handle_event: instance
                .get_typed_func(&mut *store, "plugin_handle_event")
                .ok(),

            // Context and completions
            provide_completions: instance
                .get_typed_func(&mut *store, "plugin_provide_completions")
                .ok(),
            collect_context: instance
                .get_typed_func(&mut *store, "plugin_collect_context")
                .ok(),

            // Response retrieval
            get_last_response: instance
                .get_typed_func(&mut *store, "plugin_get_last_response")
                .ok(),
            get_error_message: instance
                .get_typed_func(&mut *store, "plugin_get_error_message")
                .ok(),
        })
    }
}

/// Plugin lifecycle state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    /// Plugin is loaded but not initialized
    Loaded,
    /// Plugin is initialized and ready
    Ready,
    /// Plugin is currently executing
    Running,
    /// Plugin encountered an error
    Error(String),
    /// Plugin is being unloaded
    Unloading,
}

/// Loaded plugin instance with unified runtime
pub struct LoadedPlugin {
    /// Plugin metadata
    pub metadata: PluginMetadata,

    /// Current state
    pub state: PluginState,

    /// Runtime-specific data
    #[cfg(feature = "wasm-runtime")]
    pub wasm_data: Option<WasmPluginData>,

    /// Host integration data
    #[cfg(feature = "host-integration")]
    pub host_data: Option<HostPluginData>,

    /// Plugin ABI
    pub abi: PluginAbi,

    /// Security policy
    pub security: SecurityPolicy,

    /// Performance metrics
    pub metrics: PluginMetrics,
}

/// WASM-specific plugin data
#[cfg(feature = "wasm-runtime")]
pub struct WasmPluginData {
    pub instance: wasmtime::Instance,
    pub store: tokio::sync::Mutex<wasmtime::Store<WasmPluginContext>>,
}

/// Host plugin data for native plugins
#[cfg(feature = "host-integration")]
pub struct HostPluginData {
    pub library: libloading::Library,
    pub plugin_handle: Box<dyn Send + Sync>,
}

/// WASM plugin execution context
#[cfg(feature = "wasm-runtime")]
pub struct WasmPluginContext {
    pub permissions: PluginPermissions,
    pub security: SecurityPolicy,
    pub resource_tracker: ResourceTracker,
    pub host_interface: Option<Arc<dyn host::HostInterface>>,
    pub wasi_ctx: wasmtime_wasi::preview1::WasiP1Ctx,
}

/// Resource usage tracking
#[derive(Debug, Default)]
pub struct ResourceTracker {
    pub memory_used: usize,
    pub cpu_time_ms: u64,
    pub api_calls: u64,
    pub files_accessed: Vec<PathBuf>,
}

/// Plugin performance metrics
#[derive(Debug, Default)]
pub struct PluginMetrics {
    pub load_time_ms: u64,
    pub init_time_ms: u64,
    pub avg_command_time_ms: u64,
    pub total_commands: u64,
    pub total_errors: u64,
    pub memory_peak_kb: u64,
}

/// Unified plugin manager
pub struct UnifiedPluginManager {
    /// Loaded plugins
    plugins: Arc<RwLock<HashMap<String, Arc<LoadedPlugin>>>>,

    /// Plugin directory
    #[allow(dead_code)]
    plugin_dir: PathBuf,

    /// Host interface
    host_interface: Option<Arc<dyn host::HostInterface>>,

    /// Security policies
    #[allow(dead_code)]
    security_policies: HashMap<String, SecurityPolicy>,

    /// Runtime configuration
    #[cfg(feature = "wasm-runtime")]
    wasm_engine: wasmtime::Engine,
}

impl UnifiedPluginManager {
    /// Create a new unified plugin manager
    pub fn new(plugin_dir: impl AsRef<Path>) -> AnyResult<Self> {
        #[cfg(feature = "wasm-runtime")]
        let wasm_engine = {
            let mut config = wasmtime::Config::new();
            config.wasm_threads(false);
            config.wasm_simd(true);
            config.wasm_bulk_memory(true);
            config.epoch_interruption(true);
            wasmtime::Engine::new(&config)?
        };

        Ok(Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            plugin_dir: plugin_dir.as_ref().to_path_buf(),
            host_interface: None,
            security_policies: HashMap::new(),
            #[cfg(feature = "wasm-runtime")]
            wasm_engine,
        })
    }

    /// Set host interface for plugin communication
    pub fn set_host_interface(&mut self, interface: Arc<dyn host::HostInterface>) {
        self.host_interface = Some(interface);
    }

    /// Load a plugin from path (auto-detect WASM vs native)
    pub async fn load_plugin(&self, path: impl AsRef<Path>) -> Result<String, PluginSystemError> {
        let path = path.as_ref();
        let plugin_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PluginSystemError::InvalidFormat("Invalid plugin filename".into()))?;

        info!("Loading plugin: {} from {:?}", plugin_name, path);

        // Auto-detect plugin type
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let loaded_plugin = match extension {
            #[cfg(feature = "wasm-runtime")]
            "wasm" => self.load_wasm_plugin(path, plugin_name).await?,

            #[cfg(feature = "host-integration")]
            "so" | "dll" | "dylib" => self.load_native_plugin(path, plugin_name).await?,

            _ => {
                return Err(PluginSystemError::InvalidFormat(format!(
                    "Unsupported plugin type: {}",
                    extension
                )))
            }
        };

        let plugin_id = loaded_plugin.metadata.id.clone();
        let mut plugins = self.plugins.write().await;
        plugins.insert(plugin_id.clone(), Arc::new(loaded_plugin));

        info!("Successfully loaded plugin: {}", plugin_id);
        Ok(plugin_id)
    }

    /// Load WASM plugin
    #[cfg(feature = "wasm-runtime")]
    async fn load_wasm_plugin(
        &self,
        path: &Path,
        _plugin_name: &str,
    ) -> Result<LoadedPlugin, PluginSystemError> {
        let module = wasmtime::Module::from_file(&self.wasm_engine, path)
            .map_err(|e| PluginSystemError::InvalidFormat(e.to_string()))?;

        // Load permissions and security policy
        let permissions = self.load_plugin_permissions(path)?;
        let security = SecurityPolicy::from_permissions(&permissions);

        // Create plugin context
        // Build a minimal WASI context (no preopened dirs by default). We inherit stdio
        // to allow plugins to print logs; file and env access should be done via host functions
        // which enforce SecurityPolicy.
        let wasi_ctx = wasmtime_wasi::WasiCtxBuilder::new()
            .inherit_stdio()
            .build_p1();

        let context = WasmPluginContext {
            permissions: permissions.clone(),
            security: security.clone(),
            resource_tracker: ResourceTracker::default(),
            host_interface: self.host_interface.clone(),
            wasi_ctx,
        };

        let mut store = wasmtime::Store::new(&self.wasm_engine, context);

        // Set up resource limits
        store.limiter(|ctx| &mut ctx.resource_tracker as &mut dyn wasmtime::ResourceLimiter);

        // Create linker and add host + WASI functions
        let mut linker = wasmtime::Linker::new(&self.wasm_engine);
        // Add WASI to linker
        wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |cx: &mut WasmPluginContext| &mut cx.wasi_ctx)
            .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
        // Add host functions after WASI
        self.add_host_functions(&mut linker)?;

        // Instantiate the module
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;

        // Extract ABI
        let abi = PluginAbi::from_instance(&instance, &mut store)?;

        // Initialize plugin
        if let Some(init_fn) = &abi.init {
            store.set_epoch_deadline(1000); // 1 second timeout
            let result = init_fn
                .call(&mut store, ())
                .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;
            store.set_epoch_deadline(u64::MAX);

            if result != 0 {
                return Err(PluginSystemError::Runtime(format!(
                    "Plugin init failed with code: {}",
                    result
                )));
            }
        }

        // Get metadata
        let metadata = self.get_plugin_metadata(&instance, &abi, &mut store)?;

        Ok(LoadedPlugin {
            metadata,
            state: PluginState::Ready,
            wasm_data: Some(WasmPluginData {
                instance,
                store: tokio::sync::Mutex::new(store),
            }),
            #[cfg(feature = "host-integration")]
            host_data: None,
            abi,
            security,
            metrics: PluginMetrics::default(),
        })
    }

    /// Load native plugin
    #[cfg(feature = "host-integration")]
    async fn load_native_plugin(
        &self,
        _path: &Path,
        _plugin_name: &str,
    ) -> Result<LoadedPlugin, PluginSystemError> {
        // This would load native plugins using libloading
        // For now, return an error as it's not implemented
        Err(PluginSystemError::Runtime(
            "Native plugin loading not yet implemented".to_string(),
        ))
    }

    /// Load plugin permissions from manifest
    fn load_plugin_permissions(
        &self,
        wasm_path: &Path,
    ) -> Result<PluginPermissions, PluginSystemError> {
        let manifest_path = wasm_path.with_extension("toml");

        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path)?;
            let manifest: permissions::PluginManifest = toml::from_str(&content)
                .map_err(|e| PluginSystemError::InvalidFormat(e.to_string()))?;

            Ok(manifest.permissions.unwrap_or_default())
        } else {
            Ok(PluginPermissions::default())
        }
    }

    /// Add host functions to linker
    #[cfg(feature = "wasm-runtime")]
    fn add_host_functions(
        &self,
        linker: &mut wasmtime::Linker<WasmPluginContext>,
    ) -> Result<(), PluginSystemError> {
        // Add all the host functions from the host module
        host::add_host_functions(linker, self.host_interface.clone())
            .map_err(|e| PluginSystemError::Runtime(e.to_string()))
    }

    /// Get plugin metadata using the unified ABI
    #[cfg(feature = "wasm-runtime")]
    fn get_plugin_metadata(
        &self,
        instance: &wasmtime::Instance,
        abi: &PluginAbi,
        store: &mut wasmtime::Store<WasmPluginContext>,
    ) -> Result<PluginMetadata, PluginSystemError> {
        if let Some(get_metadata_fn) = &abi.get_metadata {
            let packed_result = get_metadata_fn
                .call(&mut *store, ())
                .map_err(|e| PluginSystemError::Runtime(e.to_string()))?;

            let ptr = (packed_result & 0xFFFF_FFFF) as u32;
            let len = (packed_result >> 32) as u32;

            if ptr == 0 || len == 0 {
                return Err(PluginSystemError::Runtime(
                    "Invalid metadata pointer/length".to_string(),
                ));
            }

            let memory = instance
                .get_export(&mut *store, "memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| {
                    PluginSystemError::Runtime("Plugin missing memory export".to_string())
                })?;

            let mut buffer = vec![0u8; len as usize];
            memory
                .read(&mut *store, ptr as usize, &mut buffer)
                .map_err(|_| {
                    PluginSystemError::Runtime("Failed to read plugin metadata".to_string())
                })?;

            let metadata: PluginMetadata = serde_json::from_slice(&buffer)?;
            Ok(metadata)
        } else {
            // Return default metadata if function not available
            Ok(PluginMetadata {
                name: "Unknown Plugin".to_string(),
                ..Default::default()
            })
        }
    }

    /// Execute a command on a plugin
    pub async fn execute_command(
        &self,
        plugin_id: &str,
        command: &str,
        args: &[String],
    ) -> Result<CommandOutput, PluginSystemError> {
        let plugins = self.plugins.read().await;
        let plugin = plugins
            .get(plugin_id)
            .ok_or_else(|| PluginSystemError::NotFound(plugin_id.to_string()))?
            .clone();
        drop(plugins);

        match plugin.state {
            PluginState::Ready => {}
            PluginState::Error(ref e) => {
                return Err(PluginSystemError::Runtime(format!(
                    "Plugin in error state: {}",
                    e
                )));
            }
            _ => {
                return Err(PluginSystemError::Runtime(
                    "Plugin not in ready state".to_string(),
                ));
            }
        }

        #[cfg(feature = "wasm-runtime")]
        if let Some(wasm_data) = &plugin.wasm_data {
            return self
                .execute_wasm_command(plugin_id, command, args, wasm_data, &plugin.abi)
                .await;
        }

        #[cfg(feature = "host-integration")]
        if let Some(host_data) = &plugin.host_data {
            return self
                .execute_host_command(plugin_id, command, args, host_data)
                .await;
        }

        Err(PluginSystemError::Runtime(
            "No execution context available".to_string(),
        ))
    }

    /// Execute command on WASM plugin
    #[cfg(feature = "wasm-runtime")]
    async fn execute_wasm_command(
        &self,
        _plugin_id: &str,
        command: &str,
        args: &[String],
        wasm_data: &WasmPluginData,
        abi: &PluginAbi,
    ) -> Result<CommandOutput, PluginSystemError> {
        // Delegate to the concrete runtime implementation
        crate::runtime::execute_wasm_command_internal(abi, wasm_data, command, args).await
    }

    /// Provide completions via plugin
    #[cfg(feature = "wasm-runtime")]
    pub async fn provide_completions_json(
        &self,
        plugin_id: &str,
        context_json: &[u8],
    ) -> Result<Vec<Completion>, PluginSystemError> {
        let plugins = self.plugins.read().await;
        let plugin = plugins
            .get(plugin_id)
            .ok_or_else(|| PluginSystemError::NotFound(plugin_id.to_string()))?
            .clone();
        drop(plugins);
        if let Some(wasm) = &plugin.wasm_data {
            return crate::runtime::provide_completions_internal(&plugin.abi, wasm, context_json)
                .await;
        }
        Err(PluginSystemError::Runtime("No WASM data available".into()))
    }

    /// Collect context via plugin
    #[cfg(feature = "wasm-runtime")]
    pub async fn collect_context_via_plugin(
        &self,
        plugin_id: &str,
        request: &ContextRequest,
    ) -> Result<Option<Context>, PluginSystemError> {
        let plugins = self.plugins.read().await;
        let plugin = plugins
            .get(plugin_id)
            .ok_or_else(|| PluginSystemError::NotFound(plugin_id.to_string()))?
            .clone();
        drop(plugins);
        if let Some(wasm) = &plugin.wasm_data {
            return crate::runtime::collect_context_internal(&plugin.abi, wasm, request).await;
        }
        Err(PluginSystemError::Runtime("No WASM data available".into()))
    }

    /// Handle event via plugin
    #[cfg(feature = "wasm-runtime")]
    pub async fn handle_event_via_plugin(
        &self,
        plugin_id: &str,
        event: &HookEvent,
    ) -> Result<HookResponse, PluginSystemError> {
        let plugins = self.plugins.read().await;
        let plugin = plugins
            .get(plugin_id)
            .ok_or_else(|| PluginSystemError::NotFound(plugin_id.to_string()))?
            .clone();
        drop(plugins);
        if let Some(wasm) = &plugin.wasm_data {
            return crate::runtime::handle_event_internal(&plugin.abi, wasm, event).await;
        }
        Err(PluginSystemError::Runtime("No WASM data available".into()))
    }

    /// Execute command on native plugin
    #[cfg(feature = "host-integration")]
    async fn execute_host_command(
        &self,
        _plugin_id: &str,
        _command: &str,
        _args: &[String],
        _host_data: &HostPluginData,
    ) -> Result<CommandOutput, PluginSystemError> {
        // This would implement native plugin command execution
        Err(PluginSystemError::Runtime(
            "Native plugin execution not implemented".to_string(),
        ))
    }

    /// List all loaded plugins
    pub async fn list_plugins(&self) -> Vec<PluginMetadata> {
        let plugins = self.plugins.read().await;
        plugins.values().map(|p| p.metadata.clone()).collect()
    }

    /// Unload a plugin
    pub async fn unload_plugin(&self, plugin_id: &str) -> Result<(), PluginSystemError> {
        let mut plugins = self.plugins.write().await;

        if let Some(plugin) = plugins.remove(plugin_id) {
            // Call cleanup if available
            #[cfg(feature = "wasm-runtime")]
            if let Some(wasm_data) = &plugin.wasm_data {
                if let Some(cleanup_fn) = &plugin.abi.cleanup {
                    let mut store = wasm_data.store.lock().await;
                    let _ = cleanup_fn.call(&mut *store, ());
                }
            }

            info!("Unloaded plugin: {}", plugin_id);
            Ok(())
        } else {
            Err(PluginSystemError::NotFound(plugin_id.to_string()))
        }
    }
}

#[cfg(feature = "wasm-runtime")]
impl wasmtime::ResourceLimiter for ResourceTracker {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        const MAX_MEMORY: usize = 100 * 1024 * 1024; // 100MB
        if desired > MAX_MEMORY {
            return Ok(false);
        }
        self.memory_used = desired;
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        const MAX_TABLES: usize = 10;
        Ok(desired <= MAX_TABLES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_plugin_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = UnifiedPluginManager::new(temp_dir.path()).unwrap();

        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 0);
    }

    #[tokio::test]
    async fn test_load_minimal_wasm_plugin() {
        // Build a minimal WASM module with just an exported memory. This exercises
        // the WASM runtime path without requiring full plugin ABI.
        // The manager should load it and fall back to default metadata when
        // plugin_get_metadata is not present.
        let wat_src = r#"(module (memory (export "memory") 1))"#;
        let wasm_bytes = wat::parse_str(wat_src).expect("wat to wasm parse failed");

        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("minimal.wasm");
        std::fs::write(&wasm_path, wasm_bytes).expect("write wasm");

        let manager = UnifiedPluginManager::new(temp_dir.path()).expect("manager");
        let load_res = manager.load_plugin(&wasm_path).await;
        assert!(load_res.is_ok(), "failed to load minimal wasm plugin: {:?}", load_res);

        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 1, "expected one loaded plugin");
        // When metadata export is missing, name defaults to "Unknown Plugin".
        assert_eq!(plugins[0].name, "Unknown Plugin");
    }
}
