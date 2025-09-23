//! Communication functionality for plugin runtime

use crate::{RuntimeResult, RuntimeConfig};

/// Plugin communication manager
#[derive(Debug)]
pub struct PluginCommunication {
    #[allow(dead_code)]
    config: RuntimeConfig,
}

impl PluginCommunication {
    pub fn new(config: &RuntimeConfig) -> RuntimeResult<Self> {
        tracing::info!("Initializing plugin communication manager");
        Ok(Self {
            config: config.clone(),
        })
    }
    
    pub fn send_message(&self, plugin_id: &str, message: &str) -> RuntimeResult<()> {
        tracing::debug!("Sending message to plugin {}: {}", plugin_id, message);
        // TODO: Implement plugin message sending
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_message_ok() {
        let cfg = RuntimeConfig::default();
        let comm = PluginCommunication::new(&cfg).expect("comm");
        let res = comm.send_message("p1", "hello");
        assert!(res.is_ok());
    }
}
