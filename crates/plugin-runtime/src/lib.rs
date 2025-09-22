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
