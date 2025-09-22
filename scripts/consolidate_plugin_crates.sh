#!/bin/bash
set -euo pipefail

echo "🔧 Starting plugin crates consolidation..."

# Define the consolidation plan
# plugin-api + plugin-sdk -> plugin-sdk (developer-facing)
# plugin-loader + plugin-system -> plugin-runtime (runtime engine)

# Create plugin-runtime (loader + system)
NEW_RUNTIME_CRATE="plugin-runtime" 
RUNTIME_CRATE_PATH="crates/$NEW_RUNTIME_CRATE"

# Create plugin-sdk (api + sdk - developer-facing)
# Note: plugin-sdk already exists, we'll enhance it with plugin-api content

echo "📁 Creating plugin-runtime crate..."
mkdir -p "$RUNTIME_CRATE_PATH/src"

# Create the plugin-runtime Cargo.toml
cat > "$RUNTIME_CRATE_PATH/Cargo.toml" << 'EOF'
[package]
name = "plugin-runtime"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
description = "Plugin runtime engine for OpenAgent Terminal (WASM loading and execution)"

[dependencies]
# Core dependencies
anyhow.workspace = true
thiserror.workspace = true
tokio.workspace = true
tracing.workspace = true
serde.workspace = true
serde_json.workspace = true

# WASM runtime
wasmtime.workspace = true
wasmtime-wasi.workspace = true

# JSON-RPC for plugin communication
jsonrpc-core.workspace = true
jsonrpc-ipc-server.workspace = true

# Security and sandboxing
parking_lot.workspace = true

# File system operations
dirs = "6.0.0"

[features]
default = ["wasi"]
wasi = []
security-audit = []
hot-reload = []
all = ["wasi", "security-audit", "hot-reload"]
EOF

# Create the plugin-runtime lib.rs
cat > "$RUNTIME_CRATE_PATH/src/lib.rs" << 'EOF'
//! Plugin runtime engine for OpenAgent Terminal
//! 
//! This crate provides the plugin loading and execution system including:
//! - WASM plugin loading and compilation
//! - WASI sandboxing and security
//! - Plugin lifecycle management
//! - Inter-plugin communication

#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

pub mod loader;
pub mod sandbox;
pub mod lifecycle;
pub mod communication;
pub mod security;

pub use loader::PluginLoader;
pub use sandbox::WasmSandbox;
pub use lifecycle::PluginLifecycle;
pub use communication::PluginCommunication;

/// Plugin runtime error types
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("Plugin loading error: {0}")]
    Loading(String),
    
    #[error("WASM execution error: {0}")]
    WasmExecution(String),
    
    #[error("Sandbox violation: {0}")]
    SandboxViolation(String),
    
    #[error("Communication error: {0}")]
    Communication(String),
    
    #[error("Security error: {0}")]
    Security(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    
    #[error("Wasmtime error: {0}")]
    Wasmtime(String),
}

impl From<wasmtime::Error> for RuntimeError {
    fn from(error: wasmtime::Error) -> Self {
        Self::Wasmtime(error.to_string())
    }
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;

/// Plugin runtime configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeConfig {
    pub enable_wasi: bool,
    pub enable_security_audit: bool,
    pub enable_hot_reload: bool,
    pub max_memory_mb: u64,
    pub max_execution_time_ms: u64,
    pub sandbox_filesystem: bool,
    pub sandbox_network: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            enable_wasi: true,
            enable_security_audit: true,
            enable_hot_reload: false,
            max_memory_mb: 64,
            max_execution_time_ms: 5000,
            sandbox_filesystem: true,
            sandbox_network: true,
        }
    }
}

/// Main plugin runtime system
#[derive(Debug)]
pub struct PluginRuntime {
    config: RuntimeConfig,
    loader: PluginLoader,
    sandbox: WasmSandbox,
    lifecycle: PluginLifecycle,
    communication: PluginCommunication,
}

impl PluginRuntime {
    pub fn new(config: RuntimeConfig) -> RuntimeResult<Self> {
        let loader = PluginLoader::new(&config)?;
        let sandbox = WasmSandbox::new(&config)?;
        let lifecycle = PluginLifecycle::new(&config)?;
        let communication = PluginCommunication::new(&config)?;
        
        Ok(Self {
            config,
            loader,
            sandbox,
            lifecycle,
            communication,
        })
    }
    
    pub fn load_plugin(&mut self, path: &std::path::Path) -> RuntimeResult<String> {
        tracing::info!("Loading plugin from: {:?}", path);
        self.loader.load_plugin(path)
    }
    
    pub fn execute_plugin(&mut self, plugin_id: &str, function: &str, args: &[u8]) -> RuntimeResult<Vec<u8>> {
        tracing::debug!("Executing plugin {} function {}", plugin_id, function);
        self.sandbox.execute_plugin(plugin_id, function, args)
    }
    
    pub fn unload_plugin(&mut self, plugin_id: &str) -> RuntimeResult<()> {
        tracing::info!("Unloading plugin: {}", plugin_id);
        self.lifecycle.unload_plugin(plugin_id)
    }
}
EOF

# Create individual module files for plugin-runtime
for module in loader sandbox lifecycle communication security; do
    cat > "$RUNTIME_CRATE_PATH/src/${module}.rs" << EOF
//! ${module^} functionality for plugin runtime

use crate::{RuntimeError, RuntimeResult, RuntimeConfig};

/// ${module^} manager
#[derive(Debug)]
pub struct $(echo ${module^} | sed 's/.*/\u&/') {
    config: RuntimeConfig,
}

impl $(echo ${module^} | sed 's/.*/\u&/') {
    pub fn new(config: &RuntimeConfig) -> RuntimeResult<Self> {
        tracing::info!("Initializing ${module} manager");
        Ok(Self {
            config: config.clone(),
        })
    }
}
EOF
    echo "✅ Created ${module}.rs"
done

# Add specific implementations for key modules
cat > "$RUNTIME_CRATE_PATH/src/loader.rs" << 'EOF'
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
EOF

echo "✅ Plugin runtime crate structure created"
echo "📁 New plugin-runtime crate: $RUNTIME_CRATE_PATH"
echo ""
echo "Next steps:"
echo "1. Update workspace Cargo.toml to include plugin-runtime"
echo "2. Update references in other crates"
echo "3. Test compilation"
echo "4. Remove old plugin-loader and plugin-system crates after validation"
echo ""
echo "Note: plugin-sdk will be enhanced with plugin-api content as needed"