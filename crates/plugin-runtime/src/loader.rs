//! Plugin loading functionality

use crate::{RuntimeConfig, RuntimeError, RuntimeResult};
use std::path::Path;
use wasmtime::{Engine, Module};

/// Plugin loader manager
#[derive(Debug)]
pub struct PluginLoader {
    _config: RuntimeConfig,
    engine: Engine,
}

impl PluginLoader {
    pub fn new(config: &RuntimeConfig) -> RuntimeResult<Self> {
        tracing::info!("Initializing plugin loader");
        let engine = Engine::default();

        Ok(Self { _config: config.clone(), engine })
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
        let plugin_id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();

        tracing::info!("Plugin loaded successfully: {}", plugin_id);
        Ok(plugin_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_minimal_wasm_module() {
        // Create minimal wasm from WAT
        let wat = "(module)";
        let wasm = wat::parse_str(wat).expect("valid wat");
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("mini.wasm");
        std::fs::write(&path, wasm).expect("write wasm");

        let cfg = RuntimeConfig::default();
        let mut loader = PluginLoader::new(&cfg).expect("loader");
        let id = loader.load_plugin(&path).expect("load");
        assert_eq!(id, "mini");
    }
}
