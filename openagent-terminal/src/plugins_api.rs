//! Compatibility wrapper to unify legacy plugin-loader API with the new plugin-runtime.
//! This allows openagent-terminal to depend on plugin-runtime + plugin-sdk only.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

pub use plugin_sdk::{CommandOutput, PluginError};

/// Log levels for plugin logging (matched to legacy plugin_loader::LogLevel)
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// Host interface exposed to plugins
pub trait PluginHost: Send + Sync {
    fn log(&self, level: LogLevel, message: &str);
    fn read_file(&self, path: &str) -> Result<Vec<u8>, PluginError>;
    fn write_file(&self, path: &str, data: &[u8]) -> Result<(), PluginError>;
    fn execute_command(&self, command: &str) -> Result<CommandOutput, PluginError>;
    fn store_data_for(&self, plugin_id: &str, key: &str, value: &[u8]) -> Result<(), PluginError>;
    fn retrieve_data_for(&self, plugin_id: &str, key: &str)
        -> Result<Option<Vec<u8>>, PluginError>;
    fn store_document_for(
        &self,
        plugin_id: &str,
        namespace: &str,
        doc_id: &str,
        doc_json: &str,
    ) -> Result<(), PluginError>;
    fn retrieve_document_for(
        &self,
        plugin_id: &str,
        namespace: &str,
        doc_id: &str,
    ) -> Result<Option<String>, PluginError>;
}

/// Signature policy placeholder compatible with legacy code
#[derive(Debug, Clone)]
pub struct SignaturePolicy {
    pub require_signatures_for_all: bool,
    pub require_system: bool,
    pub require_user: bool,
    pub require_project: bool,
    pub system_dir: Option<PathBuf>,
    pub user_dir: Option<PathBuf>,
    pub project_dir: Option<PathBuf>,
}

/// Plugin event/request to wasm plugin
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginEvent {
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: u64,
}

/// Response from plugin
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginEventResponse {
    pub success: bool,
    pub result: Option<String>,
    pub error: Option<String>,
}

/// Minimal manager built on top of plugin-runtime
pub struct PluginManager {
    runtime: Arc<Mutex<plugin_runtime::PluginRuntime>>,
    plugin_dirs: Vec<PathBuf>,
    loaded: Arc<Mutex<HashMap<String, PathBuf>>>,
    #[allow(dead_code)]
    host: Option<Arc<dyn PluginHost>>, // reserved for future host callback wiring
    #[allow(dead_code)]
    enforce_signatures: bool,
    #[allow(dead_code)]
    signature_policy: Option<SignaturePolicy>,
}

impl PluginManager {
    pub fn with_host_and_dirs(
        plugin_dirs: Vec<PathBuf>,
        host: Option<Arc<dyn PluginHost>>,
    ) -> anyhow::Result<Self> {
        let rt = plugin_runtime::PluginRuntime::new(plugin_runtime::RuntimeConfig::default())
            .map_err(|e| anyhow::anyhow!("plugin runtime init failed: {}", e))?;
        Ok(Self {
            runtime: Arc::new(Mutex::new(rt)),
            plugin_dirs,
            loaded: Arc::new(Mutex::new(HashMap::new())),
            host,
            enforce_signatures: false,
            signature_policy: None,
        })
    }

    pub fn set_enforce_signatures(&mut self, enforce: bool) {
        self.enforce_signatures = enforce;
    }

    pub fn configure_signature_policy(&mut self, policy: SignaturePolicy) {
        self.signature_policy = Some(policy);
    }

    /// Discover *.wasm under configured plugin directories
    pub async fn discover_plugins(&self) -> anyhow::Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        // Manual read-dir loop to avoid holding directory readers across awaits
        for dir in &self.plugin_dirs {
            let mut it = match tokio::fs::read_dir(dir).await {
                Ok(i) => i,
                Err(_) => continue,
            };
            while let Ok(Some(entry)) = it.next_entry().await {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) == Some("wasm") {
                    out.push(p);
                }
            }
        }
        Ok(out)
    }

    pub async fn load_plugin(&self, path: &Path) -> anyhow::Result<String> {
        let mut rt = self.runtime.lock().await;
        let id = rt
            .load_plugin(path)
            .map_err(|e| anyhow::anyhow!("load_plugin failed: {}", e))?;
        self.loaded.lock().await.insert(id.clone(), path.to_path_buf());
        Ok(id)
    }

    pub async fn unload_plugin(&self, plugin_id: &str) -> anyhow::Result<()> {
        let mut rt = self.runtime.lock().await;
        rt.unload_plugin(plugin_id)
            .map_err(|e| anyhow::anyhow!("unload_plugin failed: {}", e))?;
        self.loaded.lock().await.remove(plugin_id);
        Ok(())
    }

    pub async fn loaded_names_and_paths(&self) -> Vec<(String, PathBuf)> {
        self.loaded.lock().await.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    pub async fn send_event_to_plugin(
        &self,
        plugin_id: &str,
        event: &PluginEvent,
    ) -> anyhow::Result<PluginEventResponse> {
        // Encode event as JSON and call a conventional export
        let args = serde_json::to_vec(event)?;
        let mut rt = self.runtime.lock().await;
        // Call into plugin; ignore returned bytes for now
        let _ = rt
            .execute_plugin(plugin_id, "plugin_handle_event", &args)
            .map_err(|e| anyhow::anyhow!("execute_plugin failed: {}", e))?;
        Ok(PluginEventResponse { success: true, result: None, error: None })
    }

    /// Convenience: list plugin names
    pub async fn list_plugins(&self) -> Vec<String> {
        self.loaded.lock().await.keys().cloned().collect()
    }

    /// Broadcast an event to all loaded plugins (best-effort)
    pub async fn broadcast_event(&self, event: &PluginEvent) -> anyhow::Result<()> {
        let ids: Vec<String> = self.list_plugins().await;
        for id in ids {
            let _ = self.send_event_to_plugin(&id, event).await;
        }
        Ok(())
    }
}
