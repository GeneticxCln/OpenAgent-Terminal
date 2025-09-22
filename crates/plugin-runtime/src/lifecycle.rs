//! Lifecycle functionality for plugin runtime

use crate::{RuntimeError, RuntimeResult, RuntimeConfig};
use std::collections::HashMap;

/// Plugin lifecycle manager
#[derive(Debug)]
pub struct PluginLifecycle {
    config: RuntimeConfig,
    loaded_plugins: HashMap<String, PluginInfo>,
}

#[derive(Debug, Clone)]
struct PluginInfo {
    id: String,
    loaded_at: std::time::Instant,
}

impl PluginLifecycle {
    pub fn new(config: &RuntimeConfig) -> RuntimeResult<Self> {
        tracing::info!("Initializing plugin lifecycle manager");
        Ok(Self {
            config: config.clone(),
            loaded_plugins: HashMap::new(),
        })
    }
    
    pub fn register_plugin(&mut self, plugin_id: &str) -> RuntimeResult<()> {
        let plugin_info = PluginInfo {
            id: plugin_id.to_string(),
            loaded_at: std::time::Instant::now(),
        };
        self.loaded_plugins.insert(plugin_id.to_string(), plugin_info);
        Ok(())
    }
    
    pub fn unload_plugin(&mut self, plugin_id: &str) -> RuntimeResult<()> {
        if self.loaded_plugins.remove(plugin_id).is_some() {
            tracing::info!("Plugin {} unloaded from lifecycle manager", plugin_id);
            Ok(())
        } else {
            Err(RuntimeError::Loading(format!("Plugin {} not found", plugin_id)))
        }
    }
}
