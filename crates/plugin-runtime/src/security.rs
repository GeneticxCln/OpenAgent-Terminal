//! Security functionality for plugin runtime

use crate::{RuntimeError, RuntimeResult, RuntimeConfig};

/// Security manager
#[derive(Debug)]
pub struct Security {
    config: RuntimeConfig,
}

impl Security {
    pub fn new(config: &RuntimeConfig) -> RuntimeResult<Self> {
        tracing::info!("Initializing security manager");
        Ok(Self {
            config: config.clone(),
        })
    }
}
