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

    /// Toggle zoom for the active pane in the active tab
    pub fn toggle_zoom(&mut self) -> bool {
        if let Some(tab) = self.active_tab_mut() {
            if let Some(saved) = tab.zoom_saved_layout.take() {
                // Restore
                tab.split_layout = saved;
                true
            } else {
                // Save and zoom
                let saved = tab.split_layout.clone();
                tab.zoom_saved_layout = Some(saved);
                tab.split_layout = split_manager::SplitLayout::Single(tab.active_pane);
                true
            }
        } else {
            false
        }
    }

    /// Check if the active tab is currently zoomed
    pub fn active_tab_zoomed(&self) -> bool {
        self.tabs
            .active_tab_id()
            .map(|id| self.tabs.is_tab_zoomed(id))
            .unwrap_or(false)
    }

    /// Hit test for split divider given mouse position (in pixels)
    pub fn hit_test_split_divider(
        &self,
        x: f32,
        y: f32,
        tol: f32,
    ) -> Option<crate::workspace::split_manager::SplitDividerHit> {
        let active = self.tabs.active_tab()?;
        // Compute grid content container with padding and reserved rows
        let si = self.size_info;
        let x0 = si.padding_x();
        let mut y0 = si.padding_y();
        let w = si.width() - 2.0 * si.padding_x();
        let mut h = si.height() - 2.0 * si.padding_y();
        if self.config.workspace.tab_bar.show
            && self.config.workspace.tab_bar.reserve_row
            && self.config.workspace.tab_bar.position != crate::config::workspace::TabBarPosition::Hidden
        {
            let ch = si.cell_height();
            match self.config.workspace.tab_bar.position {
                crate::config::workspace::TabBarPosition::Top => {
                    y0 += ch;
                    h = (h - ch).max(0.0);
                },
                crate::config::workspace::TabBarPosition::Bottom => {
                    h = (h - ch).max(0.0);
                },
                _ => {},
            }
        }
        let container = split_manager::PaneRect::new(x0, y0, w, h);
        active.split_layout.hit_test_divider(container, x, y, tol)
    }

    /// Apply new ratio at a divider path
    pub fn set_split_ratio_at_path(
        &mut self,
        path: &[split_manager::SplitChild],
        axis: split_manager::SplitAxis,
        new_ratio: f32,
    ) -> bool {
        if let Some(tab) = self.active_tab_mut() {
            tab.split_layout.set_ratio_at_path_internal(path, axis, new_ratio)
        } else {
            false
        }
    }

    /// Update the size information for the workspace
    pub fn update_size(&mut self, size_info: SizeInfo) {
        // Ratios are layout-relative; pane rectangles are computed on demand during draw/hit-test
        // based on current SizeInfo and configuration (padding, reserved rows). Updating size_info
        // here is sufficient to make splits reflow on resize.
        self.size_info = size_info;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    use crate::config::workspace::TabBarPosition;
    use crate::config::UiConfig;
    use crate::display::SizeInfo;

    fn make_size_info(width: f32, height: f32, cell_w: f32, cell_h: f32, pad_x: f32, pad_y: f32) -> SizeInfo {
        // dynamic_padding = false for predictable pixel math in tests
        SizeInfo::new(width, height, cell_w, cell_h, pad_x, pad_y, false)
    }

    fn make_workspace(config: UiConfig, size_info: SizeInfo) -> WorkspaceManager {
        WorkspaceManager::new(WorkspaceId(0), Rc::new(config), size_info)
    }

    fn set_simple_horizontal_split(wm: &mut WorkspaceManager, ratio: f32) {
        if let Some(tab) = wm.tabs.active_tab_mut() {
            let pid = tab.active_pane;
            tab.split_layout = split_manager::SplitLayout::Horizontal {
                left: Box::new(split_manager::SplitLayout::Single(pid)),
                right: Box::new(split_manager::SplitLayout::Single(pid)),
                ratio,
            };
        }
    }

    fn set_simple_vertical_split(wm: &mut WorkspaceManager, ratio: f32) {
        if let Some(tab) = wm.tabs.active_tab_mut() {
            let pid = tab.active_pane;
            tab.split_layout = split_manager::SplitLayout::Vertical {
                top: Box::new(split_manager::SplitLayout::Single(pid)),
                bottom: Box::new(split_manager::SplitLayout::Single(pid)),
                ratio,
            };
        }
    }

    #[test]
    fn hit_test_excludes_reserved_top_row() {
        let mut config = UiConfig::default();
        config.workspace.tab_bar.show = true;
        config.workspace.tab_bar.reserve_row = true;
        config.workspace.tab_bar.position = TabBarPosition::Top;

        // Geometry: w=1000,h=600, cell 10x20, padding 20x10
        let si = make_size_info(1000.0, 600.0, 10.0, 20.0, 20.0, 10.0);
        let mut wm = make_workspace(config.clone(), si);
        let _tab = wm.create_tab("Test".into(), None);
        set_simple_horizontal_split(&mut wm, 0.5);

        // Reconstruct expected container
        let x0 = si.padding_x();
        let mut y0 = si.padding_y();
        let w = si.width() - 2.0 * si.padding_x();
        let mut h = si.height() - 2.0 * si.padding_y();
        if config.workspace.tab_bar.show && config.workspace.tab_bar.reserve_row {
            match config.workspace.tab_bar.position {
                TabBarPosition::Top => {
                    y0 += si.cell_height();
                    h = (h - si.cell_height()).max(0.0);
                },
                TabBarPosition::Bottom => {
                    h = (h - si.cell_height()).max(0.0);
                },
                TabBarPosition::Hidden => {},
            }
        }

        let split_x = x0 + w * 0.5;
        let tol = 3.0;

        // Y in reserved top row: should be outside container and not hit
        let y_reserved_top = si.padding_y() + si.cell_height() * 0.5;
        assert!(
            wm.hit_test_split_divider(split_x, y_reserved_top, tol).is_none(),
            "divider should not be hittable in reserved top row",
        );

        // Y inside container center: should hit
        let y_inside = y0 + h * 0.5;
        let hit = wm.hit_test_split_divider(split_x, y_inside, tol);
        assert!(hit.is_some(), "divider should be hittable inside content area");
        let hit = hit.unwrap();
        assert_eq!(hit.axis, split_manager::SplitAxis::Horizontal);
    }

    #[test]
    fn hit_test_excludes_reserved_bottom_row() {
        let mut config = UiConfig::default();
        config.workspace.tab_bar.show = true;
        config.workspace.tab_bar.reserve_row = true;
        config.workspace.tab_bar.position = TabBarPosition::Bottom;

        let si = make_size_info(800.0, 500.0, 8.0, 16.0, 12.0, 6.0);
        let mut wm = make_workspace(config.clone(), si);
        let _tab = wm.create_tab("Test".into(), None);
        set_simple_vertical_split(&mut wm, 0.5);

        let x0 = si.padding_x();
        let y0 = si.padding_y();
        let w = si.width() - 2.0 * si.padding_x();
        let mut h = si.height() - 2.0 * si.padding_y();
        if config.workspace.tab_bar.show && config.workspace.tab_bar.reserve_row {
            if config.workspace.tab_bar.position == TabBarPosition::Bottom {
                h = (h - si.cell_height()).max(0.0);
            }
        }
        let split_y = y0 + h * 0.5;
        let tol = 3.0;

        // Y in reserved bottom row: pick a y below container range; should not hit
        let y_reserved_bottom = y0 + h + (si.cell_height() * 0.5);
        let x_inside = x0 + w * 0.5;
        assert!(
            wm.hit_test_split_divider(x_inside, y_reserved_bottom, tol).is_none(),
            "divider should not be hittable in reserved bottom row",
        );

        // Inside container
        let hit = wm.hit_test_split_divider(x_inside, split_y, tol);
        assert!(hit.is_some(), "divider should be hittable inside content area");
        let hit = hit.unwrap();
        assert_eq!(hit.axis, split_manager::SplitAxis::Vertical);
    }

    #[test]
    fn hit_test_respects_padding() {
        let mut config = UiConfig::default();
        config.workspace.tab_bar.show = false; // ignore reserved rows in this test

        let si = make_size_info(640.0, 480.0, 10.0, 20.0, 30.0, 40.0);
        let mut wm = make_workspace(config.clone(), si);
        let _tab = wm.create_tab("Test".into(), None);
        set_simple_horizontal_split(&mut wm, 0.25);

        let x0 = si.padding_x();
        let y0 = si.padding_y();
        let w = si.width() - 2.0 * si.padding_x();
        let h = si.height() - 2.0 * si.padding_y();

        // Divider at 25% of content width from left padding
        let split_x = x0 + w * 0.25;
        let y_inside = y0 + h * 0.5;
        let tol = 2.0;

        let hit = wm.hit_test_split_divider(split_x, y_inside, tol);
        assert!(hit.is_some(), "divider should be hittable at correct x considering padding");
        assert_eq!(hit.unwrap().axis, split_manager::SplitAxis::Horizontal);
    }
}
