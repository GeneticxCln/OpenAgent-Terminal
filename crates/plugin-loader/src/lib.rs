//! WebAssembly Plugin Loader for OpenAgent Terminal
//!
//! This module provides a secure, sandboxed environment for loading and executing
//! WebAssembly plugins with enforced permissions and resource limits.

use anyhow::{Context, Result};
use plugin_api::{PluginMetadata, PluginPermissions};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use wasmtime::*;
use wasmtime_wasi::{Dir, WasiCtx, WasiCtxBuilder};

/// Plugin loader error types
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    NotFound(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),
    
    #[error("Invalid plugin format: {0}")]
    InvalidFormat(String),
    
    #[error("Plugin initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Runtime error: {0}")]
    RuntimeError(String),
}

/// Loaded plugin instance
pub struct LoadedPlugin {
    /// Plugin metadata
    pub metadata: PluginMetadata,
    /// WASM instance
    instance: Instance,
    /// WASM store with context
    store: Store<PluginContext>,
    /// Plugin's exported functions
    exports: PluginExports,
}

/// Plugin context stored in WASM store
struct PluginContext {
    wasi: WasiCtx,
    permissions: PluginPermissions,
    resource_tracker: ResourceTracker,
}

/// Exported functions from a plugin
struct PluginExports {
    init: Option<TypedFunc<(), i32>>,
    get_metadata: Option<TypedFunc<(), i32>>,
    handle_event: Option<TypedFunc<(i32, i32), i32>>,
    cleanup: Option<TypedFunc<(), i32>>,
}

/// Resource usage tracker
#[derive(Default)]
struct ResourceTracker {
    memory_used: usize,
    cpu_time_ms: u64,
    api_calls: u64,
}

/// Plugin manager for loading and managing plugins
pub struct PluginManager {
    engine: Engine,
    plugins: Arc<RwLock<HashMap<String, Arc<LoadedPlugin>>>>,
    plugin_dir: PathBuf,
    enforce_permissions: bool,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(plugin_dir: impl AsRef<Path>, enforce_permissions: bool) -> Result<Self> {
        // Configure the WASM engine
        let mut config = Config::new();
        config.wasm_threads(false); // Disable threads for security
        config.wasm_simd(true);
        config.wasm_bulk_memory(true);
        config.consume_fuel(true); // Enable fuel for CPU limiting
        
        let engine = Engine::new(&config)?;
        
        Ok(Self {
            engine,
            plugins: Arc::new(RwLock::new(HashMap::new())),
            plugin_dir: plugin_dir.as_ref().to_path_buf(),
            enforce_permissions,
        })
    }
    
    /// Load a plugin from a WASM file
    pub async fn load_plugin(&self, path: impl AsRef<Path>) -> Result<String, PluginError> {
        let path = path.as_ref();
        let plugin_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PluginError::InvalidFormat("Invalid plugin filename".into()))?;
        
        info!("Loading plugin: {} from {:?}", plugin_name, path);
        
        // Load the WASM module
        let module = Module::from_file(&self.engine, path)
            .map_err(|e| PluginError::InvalidFormat(e.to_string()))?;
        
        // Create plugin context with permissions
        let permissions = self.read_plugin_permissions(path)?;
        let mut store = self.create_plugin_store(permissions.clone())?;
        
        // Instantiate the module
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|e| PluginError::InitializationFailed(e.to_string()))?;
        
        // Get exported functions
        let exports = self.get_plugin_exports(&instance, &mut store)?;
        
        // Initialize the plugin
        if let Some(init) = exports.init {
            store.add_fuel(1_000_000)?; // Add initial fuel
            let result = init.call(&mut store, ())
                .map_err(|e| PluginError::InitializationFailed(e.to_string()))?;
            
            if result != 0 {
                return Err(PluginError::InitializationFailed(
                    format!("Plugin init returned error code: {}", result)
                ));
            }
        }
        
        // Get metadata
        let metadata = self.get_plugin_metadata(&exports, &mut store)?;
        
        // Validate permissions match metadata
        if self.enforce_permissions {
            self.validate_permissions(&metadata.permissions, &permissions)?;
        }
        
        let loaded_plugin = Arc::new(LoadedPlugin {
            metadata,
            instance,
            store,
            exports,
        });
        
        // Store the plugin
        let mut plugins = self.plugins.write().await;
        plugins.insert(plugin_name.to_string(), loaded_plugin);
        
        info!("Successfully loaded plugin: {}", plugin_name);
        Ok(plugin_name.to_string())
    }
    
    /// Unload a plugin
    pub async fn unload_plugin(&self, name: &str) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().await;
        
        if let Some(plugin) = plugins.remove(name) {
            // Call cleanup if available
            if let Some(cleanup) = plugin.exports.cleanup {
                let mut store = &mut plugin.store;
                cleanup.call(&mut store, ())
                    .map_err(|e| PluginError::RuntimeError(e.to_string()))?;
            }
            
            info!("Unloaded plugin: {}", name);
            Ok(())
        } else {
            Err(PluginError::NotFound(name.to_string()))
        }
    }
    
    /// List all loaded plugins
    pub async fn list_plugins(&self) -> Vec<PluginMetadata> {
        let plugins = self.plugins.read().await;
        plugins.values().map(|p| p.metadata.clone()).collect()
    }
    
    /// Discover plugins in the plugin directory
    pub async fn discover_plugins(&self) -> Result<Vec<PathBuf>> {
        let mut discovered = Vec::new();
        
        if !self.plugin_dir.exists() {
            warn!("Plugin directory does not exist: {:?}", self.plugin_dir);
            return Ok(discovered);
        }
        
        let entries = std::fs::read_dir(&self.plugin_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                discovered.push(path);
            }
        }
        
        debug!("Discovered {} plugins", discovered.len());
        Ok(discovered)
    }
    
    /// Create a plugin store with WASI context
    fn create_plugin_store(&self, permissions: PluginPermissions) -> Result<Store<PluginContext>> {
        let mut wasi_builder = WasiCtxBuilder::new();
        
        // Configure WASI based on permissions
        if permissions.network {
            // Network access would be configured here if WASI supported it
            debug!("Network access requested but not yet implemented in WASI");
        }
        
        // Add allowed environment variables
        for var in &permissions.environment_variables {
            if let Ok(value) = std::env::var(var) {
                wasi_builder = wasi_builder.env(var, value)?;
            }
        }
        
        // Add file system access
        for pattern in &permissions.read_files {
            // In a real implementation, we'd parse glob patterns
            // For now, treat them as direct paths
            if let Ok(dir) = Dir::open_ambient_dir(pattern, ambient_authority()) {
                wasi_builder = wasi_builder.preopened_dir(dir, pattern)?;
            }
        }
        
        let wasi = wasi_builder.build();
        
        let context = PluginContext {
            wasi,
            permissions,
            resource_tracker: ResourceTracker::default(),
        };
        
        let mut store = Store::new(&self.engine, context);
        
        // Set resource limits
        store.limiter(|ctx| &mut ctx.resource_tracker as &mut dyn ResourceLimiter);
        
        Ok(store)
    }
    
    /// Get exported functions from a plugin
    fn get_plugin_exports(
        &self,
        instance: &Instance,
        store: &mut Store<PluginContext>,
    ) -> Result<PluginExports, PluginError> {
        Ok(PluginExports {
            init: instance.get_typed_func(store, "plugin_init").ok(),
            get_metadata: instance.get_typed_func(store, "plugin_get_metadata").ok(),
            handle_event: instance.get_typed_func(store, "plugin_handle_event").ok(),
            cleanup: instance.get_typed_func(store, "plugin_cleanup").ok(),
        })
    }
    
    /// Get plugin metadata
    fn get_plugin_metadata(
        &self,
        exports: &PluginExports,
        store: &mut Store<PluginContext>,
    ) -> Result<PluginMetadata, PluginError> {
        // This is simplified - in reality we'd need to handle memory passing
        // between WASM and host for complex data structures
        
        // For now, return a default metadata
        Ok(PluginMetadata {
            name: "unknown".to_string(),
            version: "0.0.0".to_string(),
            author: "unknown".to_string(),
            description: "Plugin metadata not available".to_string(),
            license: "unknown".to_string(),
            homepage: None,
            capabilities: Default::default(),
            permissions: Default::default(),
        })
    }
    
    /// Read plugin permissions from manifest
    fn read_plugin_permissions(&self, path: &Path) -> Result<PluginPermissions, PluginError> {
        // Look for a manifest file next to the WASM file
        let manifest_path = path.with_extension("toml");
        
        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path)
                .map_err(|e| PluginError::InvalidFormat(e.to_string()))?;
            
            // Parse TOML manifest
            // This is simplified - would need proper TOML parsing
            debug!("Reading permissions from {:?}", manifest_path);
        }
        
        // Return default permissions for now
        Ok(PluginPermissions::default())
    }
    
    /// Validate that requested permissions match allowed permissions
    fn validate_permissions(
        &self,
        requested: &PluginPermissions,
        allowed: &PluginPermissions,
    ) -> Result<(), PluginError> {
        if requested.network && !allowed.network {
            return Err(PluginError::PermissionDenied(
                "Network access not allowed".into()
            ));
        }
        
        if requested.execute_commands && !allowed.execute_commands {
            return Err(PluginError::PermissionDenied(
                "Command execution not allowed".into()
            ));
        }
        
        // Check file access patterns
        for pattern in &requested.write_files {
            if !allowed.write_files.iter().any(|p| p == pattern) {
                return Err(PluginError::PermissionDenied(
                    format!("Write access to {} not allowed", pattern)
                ));
            }
        }
        
        Ok(())
    }
}

/// Resource limiter implementation
impl ResourceLimiter for ResourceTracker {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        const MAX_MEMORY: usize = 50 * 1024 * 1024; // 50MB default limit
        
        if desired > MAX_MEMORY {
            return Ok(false);
        }
        
        self.memory_used = desired;
        Ok(true)
    }
    
    fn table_growing(
        &mut self,
        _current: u32,
        desired: u32,
        _maximum: Option<u32>,
    ) -> anyhow::Result<bool> {
        const MAX_TABLES: u32 = 10;
        Ok(desired <= MAX_TABLES)
    }
}

/// Get the ambient authority for directory access
fn ambient_authority() -> wasmtime_wasi::ambient_authority::AmbientAuthority {
    wasmtime_wasi::ambient_authority::ambient_authority()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_plugin_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PluginManager::new(temp_dir.path(), true).unwrap();
        
        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 0);
    }
    
    #[tokio::test]
    async fn test_plugin_discovery() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create some dummy WASM files
        std::fs::write(temp_dir.path().join("plugin1.wasm"), b"fake").unwrap();
        std::fs::write(temp_dir.path().join("plugin2.wasm"), b"fake").unwrap();
        std::fs::write(temp_dir.path().join("not_plugin.txt"), b"fake").unwrap();
        
        let manager = PluginManager::new(temp_dir.path(), true).unwrap();
        let discovered = manager.discover_plugins().await.unwrap();
        
        assert_eq!(discovered.len(), 2);
    }
}
