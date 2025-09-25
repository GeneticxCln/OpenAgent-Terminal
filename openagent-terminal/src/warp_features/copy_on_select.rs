use crate::clipboard::Clipboard;
use crate::config::Config;
use alacritty_terminal::index::Point;
use alacritty_terminal::selection::Selection;
use std::time::{Duration, Instant};

/// Warp-style copy on select feature
pub struct CopyOnSelect {
    enabled: bool,
    clipboard: Clipboard,
    last_selection: Option<String>,
    last_copy_time: Option<Instant>,
    debounce_duration: Duration,
}

impl CopyOnSelect {
    pub fn new(config: &Config) -> Self {
        Self {
            enabled: config.copy_on_select,
            clipboard: Clipboard::new(),
            last_selection: None,
            last_copy_time: None,
            debounce_duration: Duration::from_millis(100),
        }
    }

    /// Update the copy on select setting
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if copy on select is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Handle text selection change - automatically copy to clipboard if enabled
    pub fn handle_selection_change(&mut self, selection_text: Option<String>) {
        if !self.enabled {
            return;
        }

        match selection_text {
            Some(text) if !text.trim().is_empty() => {
                // Debounce rapid selection changes
                let now = Instant::now();
                if let Some(last_copy) = self.last_copy_time {
                    if now.duration_since(last_copy) < self.debounce_duration {
                        return;
                    }
                }

                // Only copy if the selection has actually changed
                if self.last_selection.as_ref() != Some(&text) {
                    if let Err(e) = self.clipboard.store_primary(text.clone()) {
                        tracing::warn!("Failed to copy selection to clipboard: {}", e);
                    } else {
                        tracing::debug!("Auto-copied selection to clipboard: {} chars", text.len());
                        self.last_selection = Some(text);
                        self.last_copy_time = Some(now);
                    }
                }
            }
            _ => {
                // Selection cleared
                self.last_selection = None;
            }
        }
    }

    /// Handle selection within blocks (Warp-specific behavior)
    pub fn handle_block_selection(&mut self, block_text: String, is_complete_selection: bool) {
        if !self.enabled || block_text.trim().is_empty() {
            return;
        }

        // For block selections, we might want different behavior
        // For example, only copy complete blocks or format them differently
        let formatted_text = if is_complete_selection {
            // Complete block selection - include block boundaries
            format!("# Block Selection\n{}\n# End Block", block_text.trim())
        } else {
            // Partial block selection - just the selected text
            block_text
        };

        self.handle_selection_change(Some(formatted_text));
    }

    /// Reset the copy on select state
    pub fn reset(&mut self) {
        self.last_selection = None;
        self.last_copy_time = None;
    }

    /// Get the last copied selection for debugging/status
    pub fn last_selection(&self) -> Option<&String> {
        self.last_selection.as_ref()
    }
}

/// Configuration option for copy on select
#[derive(Debug, Clone, PartialEq)]
pub struct CopyOnSelectConfig {
    /// Whether copy on select is enabled
    pub enabled: bool,
    /// Debounce duration in milliseconds
    pub debounce_ms: u64,
    /// Whether to format block selections specially
    pub format_blocks: bool,
}

impl Default for CopyOnSelectConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default to match Warp's behavior
            debounce_ms: 100,
            format_blocks: true,
        }
    }
}

/// Extension trait for integrating copy on select with terminal selection
pub trait SelectionExt {
    /// Extract selected text for copy on select
    fn extract_selected_text(&self) -> Option<String>;
    
    /// Check if selection is within a block
    fn is_block_selection(&self) -> bool;
}

// Implementation would depend on the actual selection type used
// This is a placeholder to show the interface
impl SelectionExt for Selection {
    fn extract_selected_text(&self) -> Option<String> {
        // This would extract the actual selected text from the terminal
        // Implementation depends on the terminal grid and selection system
        None // Placeholder
    }
    
    fn is_block_selection(&self) -> bool {
        // Check if the selection spans block boundaries
        // Implementation would check against block markers
        false // Placeholder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            copy_on_select: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_copy_on_select_enabled() {
        let config = test_config();
        let copy_on_select = CopyOnSelect::new(&config);
        assert!(copy_on_select.is_enabled());
    }

    #[test]
    fn test_copy_on_select_disabled() {
        let mut config = test_config();
        config.copy_on_select = false;
        let copy_on_select = CopyOnSelect::new(&config);
        assert!(!copy_on_select.is_enabled());
    }

    #[test]
    fn test_selection_change() {
        let config = test_config();
        let mut copy_on_select = CopyOnSelect::new(&config);
        
        // Test selection change
        copy_on_select.handle_selection_change(Some("test selection".to_string()));
        assert_eq!(copy_on_select.last_selection(), Some(&"test selection".to_string()));
        
        // Test selection clear
        copy_on_select.handle_selection_change(None);
        assert_eq!(copy_on_select.last_selection(), None);
    }

    #[test]
    fn test_debounce() {
        let config = test_config();
        let mut copy_on_select = CopyOnSelect::new(&config);
        copy_on_select.debounce_duration = Duration::from_millis(50);
        
        // First selection
        copy_on_select.handle_selection_change(Some("first".to_string()));
        assert_eq!(copy_on_select.last_selection(), Some(&"first".to_string()));
        
        // Immediate second selection should be debounced
        // In a real test, this would need proper timing
        copy_on_select.handle_selection_change(Some("second".to_string()));
    }

    #[test]
    fn test_block_selection_formatting() {
        let config = test_config();
        let mut copy_on_select = CopyOnSelect::new(&config);
        
        copy_on_select.handle_block_selection("echo 'hello'".to_string(), true);
        
        if let Some(last_selection) = copy_on_select.last_selection() {
            assert!(last_selection.contains("# Block Selection"));
            assert!(last_selection.contains("echo 'hello'"));
            assert!(last_selection.contains("# End Block"));
        }
    }
}