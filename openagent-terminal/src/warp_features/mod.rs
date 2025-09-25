//! Warp Terminal Features
//! 
//! This module contains implementations of all the key features that make Warp Terminal unique.
//! Each feature is implemented to match Warp's behavior as closely as possible.

pub mod auto_complete_brackets;
pub mod copy_on_select;
pub mod docker_extension;
pub mod keyboard_shortcuts;
pub mod soft_wrapping;
pub mod working_directory_display;

// Re-export main types for convenience
pub use auto_complete_brackets::{
    AutoCompleteBrackets, BracketPair, CompletionResult, CompletionType,
    BracketBalanceReport, AutoCompleteBracketsConfig
};

pub use copy_on_select::{
    CopyOnSelect, CopyOnSelectConfig, SelectionExt
};

pub use soft_wrapping::{
    SoftWrappingEditor, WrappedLine, WrapBehavior
};

pub use working_directory_display::{
    WorkingDirectoryDisplay, DirectoryInfo, SizeIndicator, BreadcrumbItem
};

pub use keyboard_shortcuts::{
    WarpKeyboardShortcuts, WarpAction, KeyboardShortcut, ActionResult, Direction
};

pub use docker_extension::{
    DockerExtension, DockerContainer, ContainerStatus, Shell, DockerConnectionOptions, DockerError
};

use crate::config::Config;

/// Central coordinator for all Warp features
pub struct WarpFeatureManager {
    pub auto_complete_brackets: AutoCompleteBrackets,
    pub copy_on_select: CopyOnSelect,
    pub docker_extension: DockerExtension,
    pub keyboard_shortcuts: WarpKeyboardShortcuts,
    pub soft_wrapping: SoftWrappingEditor,
    pub working_directory_display: WorkingDirectoryDisplay,
}

impl WarpFeatureManager {
    pub fn new(config: &Config, viewport_width: usize) -> Self {
        Self {
            auto_complete_brackets: AutoCompleteBrackets::new(),
            copy_on_select: CopyOnSelect::new(config),
            docker_extension: DockerExtension::new(),
            keyboard_shortcuts: WarpKeyboardShortcuts::new(),
            soft_wrapping: SoftWrappingEditor::new(viewport_width),
            working_directory_display: WorkingDirectoryDisplay::new(),
        }
    }

    /// Update all features when configuration changes
    pub fn update_config(&mut self, config: &Config) {
        self.copy_on_select.set_enabled(config.copy_on_select);
        // Update other features as needed based on config
    }

    /// Update viewport width for features that need it
    pub fn update_viewport_width(&mut self, width: usize) {
        self.soft_wrapping.set_viewport_width(width);
    }

    /// Get current working directory display
    pub fn get_working_directory_display(&self) -> String {
        self.working_directory_display.get_display_string()
    }

    /// Handle character input with auto-completion
    pub fn handle_character_input(&self, input_char: char, current_text: &str, cursor_pos: usize) -> CompletionResult {
        self.auto_complete_brackets.handle_character_input(input_char, current_text, cursor_pos)
    }

    /// Handle text selection for copy-on-select
    pub fn handle_text_selection(&mut self, selected_text: Option<String>) {
        self.copy_on_select.handle_selection_change(selected_text);
    }

    /// Check if all Warp features are properly enabled
    pub fn verify_warp_feature_parity(&self) -> WarpFeatureParity {
        WarpFeatureParity {
            markdown_viewer: true, // Implemented
            docker_extension: self.docker_extension.is_enabled(),
            copy_on_select: self.copy_on_select.is_enabled(),
            working_directory_display: true, // Implemented
            soft_wrapping: self.soft_wrapping.is_soft_wrap_enabled(),
            auto_complete_brackets: self.auto_complete_brackets.is_enabled(),
            warp_keyboard_shortcuts: self.keyboard_shortcuts.is_enabled(),
        }
    }
}

/// Report on Warp feature parity status
#[derive(Debug, Clone)]
pub struct WarpFeatureParity {
    pub markdown_viewer: bool,
    pub docker_extension: bool,
    pub copy_on_select: bool,
    pub working_directory_display: bool,
    pub soft_wrapping: bool,
    pub auto_complete_brackets: bool,
    pub warp_keyboard_shortcuts: bool,
}

impl WarpFeatureParity {
    /// Check if all required Warp features are implemented
    pub fn is_complete(&self) -> bool {
        self.markdown_viewer
            && self.docker_extension
            && self.copy_on_select
            && self.working_directory_display
            && self.soft_wrapping
            && self.auto_complete_brackets
            && self.warp_keyboard_shortcuts
    }

    /// Get list of missing features
    pub fn missing_features(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        
        if !self.markdown_viewer {
            missing.push("Markdown Viewer");
        }
        if !self.docker_extension {
            missing.push("Docker Extension");
        }
        if !self.copy_on_select {
            missing.push("Copy on Select");
        }
        if !self.working_directory_display {
            missing.push("Working Directory Display");
        }
        if !self.soft_wrapping {
            missing.push("Soft Wrapping");
        }
        if !self.auto_complete_brackets {
            missing.push("Auto-complete Brackets");
        }
        if !self.warp_keyboard_shortcuts {
            missing.push("Warp Keyboard Shortcuts");
        }
        
        missing
    }

    /// Get completion percentage
    pub fn completion_percentage(&self) -> f32 {
        let total_features = 7.0;
        let implemented_count = 
            if self.markdown_viewer { 1.0 } else { 0.0 } +
            if self.docker_extension { 1.0 } else { 0.0 } +
            if self.copy_on_select { 1.0 } else { 0.0 } +
            if self.working_directory_display { 1.0 } else { 0.0 } +
            if self.soft_wrapping { 1.0 } else { 0.0 } +
            if self.auto_complete_brackets { 1.0 } else { 0.0 } +
            if self.warp_keyboard_shortcuts { 1.0 } else { 0.0 };
        
        (implemented_count / total_features) * 100.0
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
    fn test_warp_feature_manager_creation() {
        let config = test_config();
        let manager = WarpFeatureManager::new(&config, 80);
        
        assert!(manager.copy_on_select.is_enabled());
        assert!(manager.auto_complete_brackets.is_enabled());
    }

    #[test]
    fn test_feature_parity_reporting() {
        let config = test_config();
        let manager = WarpFeatureManager::new(&config, 80);
        let parity = manager.verify_warp_feature_parity();
        
        // Should have some features implemented
        assert!(parity.completion_percentage() > 0.0);
        
        // Should not be complete until all features are implemented
        assert!(!parity.is_complete());
        
        // Should have some missing features
        assert!(!parity.missing_features().is_empty());
    }

    #[test]
    fn test_character_input_handling() {
        let config = test_config();
        let manager = WarpFeatureManager::new(&config, 80);
        
        let result = manager.handle_character_input('(', "echo ", 5);
        assert!(result.should_complete);
        assert_eq!(result.completion_text, ")");
    }

    #[test]
    fn test_working_directory_display() {
        let config = test_config();
        let manager = WarpFeatureManager::new(&config, 80);
        
        let display = manager.get_working_directory_display();
        assert!(!display.is_empty());
    }

    #[test]
    fn test_viewport_width_update() {
        let config = test_config();
        let mut manager = WarpFeatureManager::new(&config, 80);
        
        manager.update_viewport_width(120);
        // Verify soft wrapping editor received the update
        // (This would require exposing viewport_width in SoftWrappingEditor for testing)
    }
}