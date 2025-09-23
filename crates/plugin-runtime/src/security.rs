//! Security functionality for plugin runtime

use crate::{RuntimeConfig, RuntimeResult};

/// Security manager
#[derive(Debug)]
pub struct Security {
    _config: RuntimeConfig,
}

impl Security {
    pub fn new(config: &RuntimeConfig) -> RuntimeResult<Self> {
        tracing::info!("Initializing security manager");
        Ok(Self { _config: config.clone() })
    }
}
