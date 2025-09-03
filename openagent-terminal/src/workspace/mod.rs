//! Workspace management for tabs and split panes
//!
//! This module provides built-in tab and split pane functionality for OpenAgent Terminal,
//! allowing users to manage multiple terminal sessions within a single window.

#![allow(dead_code)]

use std::path::PathBuf;
use std::rc::Rc;

use crate::config::UiConfig;
use crate::display::SizeInfo;
use openagent_terminal_core::grid::Dimensions;

pub mod split_manager;
pub mod tab_manager;
pub mod warp_integration;
pub mod warp_split_manager;
pub mod warp_tab_manager;
// Warp modules
pub mod warp_bindings {
    pub use crate::config::warp_bindings::*;
}
pub mod warp_ui {
    pub use crate::display::warp_ui::*;
}
#[cfg(test)]
mod warp_integration_test;

pub use split_manager::{PaneId, SplitManager};
pub use tab_manager::{TabContext, TabId, TabManager};
pub use warp_integration::{WarpAction, WarpIntegration, WarpIntegrationError, WarpUiUpdateType};

/// Unique identifier for a workspace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceId(pub usize);

/// Workspace manager handles tabs and split panes for a window
pub struct WorkspaceManager {
    /// Unique identifier for this workspace
    pub id: WorkspaceId,

    /// Tab manager for handling multiple tabs
    pub tabs: TabManager,

    /// Split manager for handling pane layouts
    pub splits: SplitManager,

    /// Warp-style enhanced functionality (optional)
    pub warp: Option<WarpIntegration>,

    /// Configuration for the workspace
    pub config: Rc<UiConfig>,

    /// Window size information
    pub size_info: SizeInfo,
}

impl WorkspaceManager {
    /// Create a new workspace manager
    pub fn new(id: WorkspaceId, config: Rc<UiConfig>, size_info: SizeInfo) -> Self {
        Self {
            id,
            tabs: TabManager::new(),
            splits: SplitManager::new(),
            warp: None,
            config,
            size_info,
        }
    }

    /// Create workspace manager with Warp-style functionality enabled
    pub fn with_warp(
        id: WorkspaceId,
        config: Rc<UiConfig>,
        size_info: SizeInfo,
        session_file: Option<PathBuf>,
    ) -> Self {
        let warp_integration = WarpIntegration::new(config.clone(), session_file);
        Self {
            id,
            tabs: TabManager::new(),
            splits: SplitManager::new(),
            warp: Some(warp_integration),
            config,
            size_info,
        }
    }

    /// Initialize Warp functionality if enabled
    pub fn initialize_warp(
        &mut self,
        window_context: std::sync::Arc<crate::window_context::WindowContext>,
        event_proxy: winit::event_loop::EventLoopProxy<crate::event::Event>,
    ) -> Result<(), WarpIntegrationError> {
        if let Some(warp) = &mut self.warp {
            warp.initialize(window_context, event_proxy)?;
        }
        Ok(())
    }

    /// Execute a Warp action if Warp functionality is enabled
    pub fn execute_warp_action(
        &mut self,
        action: &WarpAction,
    ) -> Result<bool, WarpIntegrationError> {
        if let Some(warp) = &mut self.warp {
            warp.execute_warp_action(action)
        } else {
            Ok(false)
        }
    }

    /// Check if Warp functionality is available
    pub fn has_warp(&self) -> bool {
        self.warp.is_some()
    }

    fn ratio_step_horizontal(&self) -> f32 {
        let cells = self.size_info.columns().max(1) as f32;
        let step_cells = self.config.workspace.splits.resize_increment as f32;
        (step_cells / cells).clamp(0.005, 0.2)
    }

    fn ratio_step_vertical(&self) -> f32 {
        let cells = self.size_info.screen_lines().max(1) as f32;
        let step_cells = self.config.workspace.splits.resize_increment as f32;
        (step_cells / cells).clamp(0.005, 0.2)
    }

    /// Get the currently active tab
    pub fn active_tab(&self) -> Option<&TabContext> {
        self.tabs.active_tab()
    }

    /// Get the currently active tab mutably
    pub fn active_tab_mut(&mut self) -> Option<&mut TabContext> {
        self.tabs.active_tab_mut()
    }

    /// Create a new tab
    pub fn create_tab(&mut self, title: String, working_dir: Option<PathBuf>) -> TabId {
        self.tabs.create_tab(title, working_dir)
    }

    /// Close a tab by ID
    pub fn close_tab(&mut self, tab_id: TabId) -> bool {
        self.tabs.close_tab(tab_id)
    }

    /// Switch to a specific tab
    pub fn switch_to_tab(&mut self, tab_id: TabId) -> bool {
        self.tabs.switch_to_tab(tab_id)
    }

    /// Switch to the next tab
    pub fn next_tab(&mut self) -> bool {
        self.tabs.next_tab()
    }

    /// Switch to the previous tab
    pub fn previous_tab(&mut self) -> bool {
        self.tabs.previous_tab()
    }

    /// Get the number of tabs
    pub fn tab_count(&self) -> usize {
        self.tabs.tab_count()
    }

    /// Split the current pane horizontally
    pub fn split_horizontal(&mut self, ratio: f32) -> Option<PaneId> {
        if let Some(tab) = self.active_tab_mut() {
            SplitManager::split_horizontal_static(&mut tab.split_layout, tab.active_pane, ratio)
        } else {
            None
        }
    }

    /// Split the current pane vertically
    pub fn split_vertical(&mut self, ratio: f32) -> Option<PaneId> {
        if let Some(tab) = self.active_tab_mut() {
            SplitManager::split_vertical_static(&mut tab.split_layout, tab.active_pane, ratio)
        } else {
            None
        }
    }

    /// Focus the next pane in the current tab
    pub fn focus_next_pane(&mut self) -> bool {
        if let Some(tab) = self.active_tab_mut() {
            SplitManager::focus_next_pane_static(&tab.split_layout, &mut tab.active_pane)
        } else {
            false
        }
    }

    /// Focus the previous pane in the current tab
    pub fn focus_previous_pane(&mut self) -> bool {
        if let Some(tab) = self.active_tab_mut() {
            SplitManager::focus_previous_pane_static(&tab.split_layout, &mut tab.active_pane)
        } else {
            false
        }
    }

    /// Close the current pane
    pub fn close_pane(&mut self) -> bool {
        if let Some(tab) = self.active_tab_mut() {
            // If this is the last pane in the tab, close the tab instead
            if SplitManager::pane_count_static(&tab.split_layout) <= 1 {
                let tab_id = tab.id;
                return self.close_tab(tab_id);
            }

            SplitManager::close_pane_static(&mut tab.split_layout, tab.active_pane)
        } else {
            false
        }
    }

    /// Resize the active pane horizontally; positive delta moves divider right, negative left
    pub fn resize_horizontal(&mut self, delta: f32) -> bool {
        if let Some(tab) = self.active_tab_mut() {
            SplitManager::resize_split_static(&mut tab.split_layout, tab.active_pane, delta)
        } else {
            false
        }
    }

    /// Resize the active pane vertically; positive delta moves divider down, negative up
    pub fn resize_vertical(&mut self, delta: f32) -> bool {
        if let Some(tab) = self.active_tab_mut() {
            SplitManager::resize_split_static(&mut tab.split_layout, tab.active_pane, delta)
        } else {
            false
        }
    }

    pub fn resize_left(&mut self) -> bool {
        self.resize_horizontal(-self.ratio_step_horizontal())
    }
    pub fn resize_right(&mut self) -> bool {
        self.resize_horizontal(self.ratio_step_horizontal())
    }
    pub fn resize_up(&mut self) -> bool {
        self.resize_vertical(-self.ratio_step_vertical())
    }
    pub fn resize_down(&mut self) -> bool {
        self.resize_vertical(self.ratio_step_vertical())
    }

    /// Update the size information for the workspace
    pub fn update_size(&mut self, size_info: SizeInfo) {
        self.size_info = size_info;
        // TODO: Recalculate pane sizes
    }

    /// Check if workspace features are enabled
    pub fn is_enabled(&self) -> bool {
        // Check configuration for workspace.enabled
        true // TODO: Read from config
    }
}

/// Persistent state for a tab that can be serialized
#[derive(Debug, Clone)]
pub struct PersistentTabState {
    pub title: String,
    pub working_directory: PathBuf,
    pub shell_command: Option<String>,
    #[cfg(feature = "ai")]
    pub ai_conversation: Option<Vec<String>>,
}

// WorkspaceConfig is now imported from crate::config::workspace
pub use crate::config::workspace::TabBarPosition;
