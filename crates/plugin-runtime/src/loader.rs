//! Plugin loading functionality

use crate::{RuntimeError, RuntimeResult, RuntimeConfig};
use std::path::Path;
use wasmtime::{Engine, Module, Store};

/// Plugin loader manager
#[derive(Debug)]
pub struct PluginLoader {
    config: RuntimeConfig,
    engine: Engine,
}

impl PluginLoader {
    pub fn new(config: &RuntimeConfig) -> RuntimeResult<Self> {
        tracing::info!("Initializing plugin loader");
        let engine = Engine::default();
        
        Ok(Self {
            config: config.clone(),
            engine,
        })
    }
    
    pub fn load_plugin(&mut self, path: &Path) -> RuntimeResult<String> {
        tracing::info!("Loading WASM plugin from: {:?}", path);
        
        // Read the WASM file
        let wasm_bytes = std::fs::read(path)
            .map_err(|e| RuntimeError::Loading(format!("Failed to read WASM file: {}", e)))?;
        
        // Compile the module
        let _module = Module::from_binary(&self.engine, &wasm_bytes)
            .map_err(|e| RuntimeError::Loading(format!("Failed to compile WASM: {}", e)))?;
        
        // Generate a plugin ID (simplified - use filename for now)
        let plugin_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        tracing::info!("Plugin loaded successfully: {}", plugin_id);
        Ok(plugin_id)
    }
}
