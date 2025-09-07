// Environment management for blocks

use std::collections::HashMap;

/// Environment manager for blocks
#[allow(dead_code)]
pub struct EnvironmentManager {
    base_environment: HashMap<String, String>,
}

impl Default for EnvironmentManager {
    fn default() -> Self { Self::new() }
}

impl EnvironmentManager {
    pub fn new() -> Self {
        Self { base_environment: std::env::vars().collect() }
    }

    pub fn capture_current(&self) -> HashMap<String, String> {
        std::env::vars().collect()
    }
}
