use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use wasmtime::*;
use tracing::{debug, warn, error, info};

use crate::{Plugin, PluginMetadata, PluginPermissions, PluginError, PluginConfig};

/// Sandboxed plugin host using WASM runtime
pub struct SandboxedPluginHost {
    engine: Engine,
    plugins: HashMap<String, SandboxedPlugin>,
    permission_manager: PermissionManager,
}

/// A single sandboxed plugin instance
struct SandboxedPlugin {
    store: Store<PluginState>,
    instance: Instance,
    metadata: PluginMetadata,
    limits: ResourceLimits,
}

/// Plugin state within the WASM store
struct PluginState {
    permissions: PluginPermissions,
    start_time: Instant,
    memory_usage: usize,
    call_depth: usize,
}

/// Resource limits for plugins
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_memory_bytes: usize,
    pub max_execution_time: Duration,
    pub max_call_depth: usize,
    pub max_file_size: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 50 * 1024 * 1024, // 50MB
            max_execution_time: Duration::from_secs(5),
            max_call_depth: 100,
            max_file_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

/// Permission manager for validating plugin requests
pub struct PermissionManager {
    allowed_paths: Vec<PathBuf>,
    allowed_networks: Vec<String>,
    allowed_env_vars: Vec<String>,
}

impl SandboxedPluginHost {
    pub fn new() -> Result<Self, PluginError> {
        let mut config = Config::new();
        config.wasm_simd(true);
        config.wasm_bulk_memory(true);
        config.wasm_multi_value(true);
        config.wasm_reference_types(true);
        
        // Enable fuel-based execution limits
        config.consume_fuel(true);
        
        let engine = Engine::new(&config)
            .map_err(|e| PluginError::InitError(format!("Failed to create WASM engine: {}", e)))?;
        
        Ok(Self {
            engine,
            plugins: HashMap::new(),
            permission_manager: PermissionManager::default(),
        })
    }
    
    /// Load a plugin from WASM file with sandboxing
    pub fn load_plugin(&mut self, path: &Path) -> Result<String, PluginError> {
        info!("Loading plugin from {:?}", path);
        
        // Read and validate WASM module
        let module_bytes = std::fs::read(path)
            .map_err(|e| PluginError::IoError(e))?;
        
        let module = Module::new(&self.engine, module_bytes)
            .map_err(|e| PluginError::InitError(format!("Invalid WASM module: {}", e)))?;
        
        // Create sandboxed store with limits
        let mut store = Store::new(&self.engine, PluginState {
            permissions: PluginPermissions::default(),
            start_time: Instant::now(),
            memory_usage: 0,
            call_depth: 0,
        });
        
        // Set fuel limit for execution
        store.add_fuel(1_000_000)
            .map_err(|e| PluginError::InitError(format!("Failed to set fuel: {}", e)))?;
        
        // Create WASI context with restricted capabilities
        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .build();
        
        // Link WASI and create instance
        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)
            .map_err(|e| PluginError::InitError(format!("Failed to link WASI: {}", e)))?;
        
        // Add host functions with permission checks
        self.add_host_functions(&mut linker)?;
        
        let instance = linker.instantiate(&mut store, &module)
            .map_err(|e| PluginError::InitError(format!("Failed to instantiate module: {}", e)))?;
        
        // Get plugin metadata
        let metadata = self.get_plugin_metadata(&mut store, &instance)?;
        let plugin_id = metadata.name.clone();
        
        // Validate permissions
        self.validate_permissions(&metadata.permissions)?;
        
        // Store the plugin
        let plugin = SandboxedPlugin {
            store,
            instance,
            metadata,
            limits: ResourceLimits::default(),
        };
        
        self.plugins.insert(plugin_id.clone(), plugin);
        info!("Successfully loaded plugin: {}", plugin_id);
        
        Ok(plugin_id)
    }
    
    /// Add host functions that plugins can call
    fn add_host_functions(&self, linker: &mut Linker<PluginState>) -> Result<(), PluginError> {
        // Logging function with rate limiting
        linker.func_wrap("env", "host_log", |mut caller: Caller<'_, PluginState>, level: i32, ptr: i32, len: i32| {
            let state = caller.data();
            
            // Check rate limit
            if state.start_time.elapsed() < Duration::from_millis(100) {
                return; // Rate limit logging
            }
            
            // Read message from WASM memory
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| PluginError::Unknown("No memory export".into()))?;
            
            let mut buffer = vec![0u8; len as usize];
            memory.read(&caller, ptr as usize, &mut buffer)
                .map_err(|e| PluginError::Unknown(format!("Memory read failed: {}", e)))?;
            
            let message = String::from_utf8_lossy(&buffer);
            
            match level {
                0 => debug!("[Plugin] {}", message),
                1 => info!("[Plugin] {}", message),
                2 => warn!("[Plugin] {}", message),
                _ => error!("[Plugin] {}", message),
            }
            
            Ok(())
        }).map_err(|e| PluginError::InitError(format!("Failed to add host_log: {}", e)))?;
        
        // File read with permission check
        linker.func_wrap("env", "host_read_file", |mut caller: Caller<'_, PluginState>, path_ptr: i32, path_len: i32| -> Result<i32, Trap> {
            let state = caller.data();
            
            // Read path from WASM memory
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| Trap::new("No memory export"))?;
            
            let mut path_buffer = vec![0u8; path_len as usize];
            memory.read(&caller, path_ptr as usize, &mut path_buffer)
                .map_err(|_| Trap::new("Memory read failed"))?;
            
            let path = String::from_utf8_lossy(&path_buffer);
            
            // Check permissions
            if !self.permission_manager.can_read_file(&path, &state.permissions) {
                return Err(Trap::new("Permission denied: file read"));
            }
            
            // Additional checks...
            Ok(0)
        }).map_err(|e| PluginError::InitError(format!("Failed to add host_read_file: {}", e)))?;
        
        // Network request with permission check
        linker.func_wrap("env", "host_network_request", |caller: Caller<'_, PluginState>, url_ptr: i32, url_len: i32| -> Result<i32, Trap> {
            let state = caller.data();
            
            if !state.permissions.network {
                return Err(Trap::new("Permission denied: network access"));
            }
            
            // Additional network request handling...
            Ok(0)
        }).map_err(|e| PluginError::InitError(format!("Failed to add host_network_request: {}", e)))?;
        
        Ok(())
    }
    
    /// Get plugin metadata from WASM exports
    fn get_plugin_metadata(&self, store: &mut Store<PluginState>, instance: &Instance) -> Result<PluginMetadata, PluginError> {
        // Call plugin's metadata export function
        let metadata_fn = instance.get_typed_func::<(), i32>(&mut *store, "plugin_get_metadata")
            .map_err(|e| PluginError::InitError(format!("Plugin missing metadata export: {}", e)))?;
        
        // For now, return a default metadata
        // In production, this would deserialize from WASM memory
        Ok(PluginMetadata {
            name: "unknown".to_string(),
            version: "0.0.0".to_string(),
            author: "unknown".to_string(),
            description: "No description".to_string(),
            license: "unknown".to_string(),
            homepage: None,
            capabilities: Default::default(),
            permissions: Default::default(),
        })
    }
    
    /// Validate that requested permissions are acceptable
    fn validate_permissions(&self, permissions: &PluginPermissions) -> Result<(), PluginError> {
        // Check memory limits
        if permissions.max_memory_mb > 100 {
            return Err(PluginError::PermissionDenied(
                "Requested memory exceeds maximum allowed (100MB)".to_string()
            ));
        }
        
        // Check timeout
        if permissions.timeout_ms > 10000 {
            return Err(PluginError::PermissionDenied(
                "Requested timeout exceeds maximum allowed (10s)".to_string()
            ));
        }
        
        // Validate file paths
        for path in &permissions.read_files {
            if path.contains("..") || path.starts_with("/etc") || path.starts_with("/sys") {
                return Err(PluginError::PermissionDenied(
                    format!("Invalid file path pattern: {}", path)
                ));
            }
        }
        
        Ok(())
    }
    
    /// Execute a plugin function with resource limits
    pub fn call_plugin_function(&mut self, plugin_id: &str, function: &str, args: &[Val]) -> Result<Vec<Val>, PluginError> {
        let plugin = self.plugins.get_mut(plugin_id)
            .ok_or_else(|| PluginError::Unknown(format!("Plugin not found: {}", plugin_id)))?;
        
        // Check execution time limit
        let start = Instant::now();
        
        // Get function
        let func = plugin.instance.get_typed_func::<(), ()>(&mut plugin.store, function)
            .map_err(|e| PluginError::Unknown(format!("Function not found: {}", e)))?;
        
        // Set interrupt handle for timeout
        let engine = plugin.store.engine().clone();
        let handle = plugin.store.interrupt_handle()
            .map_err(|e| PluginError::Unknown(format!("Failed to get interrupt handle: {}", e)))?;
        
        // Spawn timeout watcher
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(5));
            handle.interrupt();
        });
        
        // Execute with fuel consumption
        match func.call(&mut plugin.store, ()) {
            Ok(result) => Ok(vec![]),
            Err(e) => {
                if start.elapsed() > plugin.limits.max_execution_time {
                    Err(PluginError::Timeout)
                } else {
                    Err(PluginError::Unknown(format!("Execution failed: {}", e)))
                }
            }
        }
    }
    
    /// Unload a plugin
    pub fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        self.plugins.remove(plugin_id)
            .ok_or_else(|| PluginError::Unknown(format!("Plugin not found: {}", plugin_id)))?;
        
        info!("Unloaded plugin: {}", plugin_id);
        Ok(())
    }
}

impl PermissionManager {
    fn can_read_file(&self, path: &str, permissions: &PluginPermissions) -> bool {
        // Check against permission patterns
        for pattern in &permissions.read_files {
            if self.matches_pattern(path, pattern) {
                return true;
            }
        }
        false
    }
    
    fn matches_pattern(&self, path: &str, pattern: &str) -> bool {
        // Simple glob pattern matching
        // In production, use a proper glob library
        if pattern == "*" {
            return true;
        }
        path.starts_with(pattern.trim_end_matches("*"))
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self {
            allowed_paths: vec![],
            allowed_networks: vec![],
            allowed_env_vars: vec![],
        }
    }
}
