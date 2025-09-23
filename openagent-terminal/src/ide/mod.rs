//! Warp-style Inline IDE Module
//!
//! Replaces traditional IDE features (LSP, DAP, separate editors) with a
//! streamlined, terminal-integrated inline IDE experience inspired by Warp terminal.

pub mod warp_ide;

use std::path::PathBuf;
use std::sync::RwLock;

pub use warp_ide::*;

/// Warp-style inline IDE manager (native only; AI optional and integrated elsewhere)
pub struct IdeManager {
    /// Core Warp IDE engine
    warp_ide: RwLock<WarpIde>,
    /// Current project root
    project_root: Option<PathBuf>,
}

impl IdeManager {
    /// Create new IDE manager (native Warp-style behavior)
    pub fn new() -> Self {
        let config = WarpIdeConfig::default();
        let warp_ide = WarpIde::new(config);
        Self { warp_ide: RwLock::new(warp_ide), project_root: None }
    }

    /// Handle terminal input for inline completions (synchronous)
    #[allow(dead_code)]
    pub fn handle_input(&self, input: &str, cursor_pos: usize) -> Vec<CompletionItem> {
        let mut ide = self.warp_ide.write().unwrap();
        ide.handle_input(input, cursor_pos)
    }

    /// Handle command errors with native suggestions (synchronous)
    pub fn handle_error(
        &self,
        command: &str,
        exit_code: i32,
        output: &str,
    ) -> Vec<ErrorSuggestion> {
        let mut ide = self.warp_ide.write().unwrap();
        ide.handle_error(command, exit_code, output)
    }

    /// Set project root for context-aware features
    pub fn set_project_root(&mut self, root: PathBuf) {
        self.project_root = Some(root);
        // Future: pass to WarpIde if needed
    }
}

impl Default for IdeManager {
    fn default() -> Self {
        Self::new()
    }
}
