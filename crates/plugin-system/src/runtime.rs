//! Plugin runtime management
//!
//! This module provides runtime management for plugins

// Re-export from main lib for now
pub use crate::UnifiedPluginManager as PluginManager;

/// Plugin runtime interface (placeholder)
pub trait PluginRuntime {
    fn start(&mut self) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<()>;
}
