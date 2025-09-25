//! Workspace management for tabs and split panes
//!
//! This module provides built-in tab and split pane functionality for OpenAgent Terminal,
//! allowing users to manage multiple terminal sessions within a single window.

#![allow(dead_code)]

use std::collections::HashMap;
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
    #[allow(unused_imports)]
    pub use crate::config::warp_bindings::*;
}
pub mod warp_ui {
    #[allow(unused_imports)]
    pub use crate::display::warp_ui::*;
}
#[cfg(test)]
mod session_restoration_test;
#[cfg(test)]
mod warp_integration_test;

pub use split_manager::{PaneId, SplitManager};
pub use tab_manager::{TabContext, TabId, TabManager};
pub use warp_integration::{WarpAction, WarpIntegration, WarpIntegrationError, WarpUiUpdateType};

/// Unique identifier for a workspace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceId(pub usize);

/// Pending security confirmation data
#[derive(Debug, Clone)]
pub struct PendingSecurityConfirmation {
    pub command: String,
    pub dry_run: bool,
    pub timestamp: std::time::Instant,
}

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

    /// Pending security confirmations awaiting user response
    pending_security_confirmations: HashMap<String, PendingSecurityConfirmation>,
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
            pending_security_confirmations: HashMap::new(),
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
            pending_security_confirmations: HashMap::new(),
        }
    }

    /// Broadcast input bytes to all panes in the active tab (Warp mode only), skipping `skip`.
    /// Returns (attempted_writes, successful_writes).
    pub fn broadcast_input_active_tab(
        &mut self,
        bytes: &[u8],
        skip: Option<PaneId>,
    ) -> (usize, usize) {
        if let Some(warp) = &mut self.warp {
            warp.broadcast_input_active_tab(bytes, skip)
        } else {
            (0, 0)
        }
    }

    /// Initialize Warp functionality if enabled
    pub fn initialize_warp(
        &mut self,
        window_id: winit::window::WindowId,
        event_proxy: winit::event_loop::EventLoopProxy<crate::event::Event>,
        restore_on_startup: bool,
    ) -> Result<(), WarpIntegrationError> {
        if let Some(warp) = &mut self.warp {
            let size_info = self.size_info;
            warp.initialize(window_id, event_proxy, size_info, restore_on_startup)?;
        }
        Ok(())
    }

    /// Test-only initializer that does not require an EventLoopProxy. Intended for CI where building
    /// a winit EventLoop is problematic. Requires test env vars to control PTY behavior.
    pub fn initialize_warp_for_tests_no_eventloop(
        &mut self,
        window_id: winit::window::WindowId,
        restore_on_startup: bool,
    ) -> Result<(), WarpIntegrationError> {
        if let Some(warp) = &mut self.warp {
            let size_info = self.size_info;
            warp.initialize_for_tests(window_id, size_info, restore_on_startup)?;
        }
        // In test mode without an event loop, ensure the native TabManager has a default active tab
        if self.tabs.active_tab().is_none() {
            let title = "Tab 1".to_string();
            self.tabs.create_tab(title, None);
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
        // Allocate new pane id before borrowing the active tab
        let new_id = self.tabs.allocate_pane_id();

        // Extract the current layout and active pane, then release the borrow on `self`
        let (mut layout, active_pane) = if let Some(tab) = self.active_tab_mut() {
            let ap = tab.active_pane;
            let old_layout =
                std::mem::replace(&mut tab.split_layout, split_manager::SplitLayout::Single(ap));
            (old_layout, ap)
        } else {
            return None;
        };

        // Perform the split using the SplitManager while no borrow to `tab` is held
        let ok = self.splits.split_horizontal_with_id(&mut layout, active_pane, ratio, new_id);

        // Write the possibly-updated layout back to the active tab
        if let Some(tab) = self.active_tab_mut() {
            tab.split_layout = layout;
        }

        if ok {
            Some(new_id)
        } else {
            None
        }
    }

    /// Split the current pane vertically
    pub fn split_vertical(&mut self, ratio: f32) -> Option<PaneId> {
        // Allocate new pane id before borrowing the active tab
        let new_id = self.tabs.allocate_pane_id();

        // Extract the current layout and active pane, then release the borrow on `self`
        let (mut layout, active_pane) = if let Some(tab) = self.active_tab_mut() {
            let ap = tab.active_pane;
            let old_layout =
                std::mem::replace(&mut tab.split_layout, split_manager::SplitLayout::Single(ap));
            (old_layout, ap)
        } else {
            return None;
        };

        // Perform the split using the SplitManager while no borrow to `tab` is held
        let ok = self.splits.split_vertical_with_id(&mut layout, active_pane, ratio, new_id);

        // Write the possibly-updated layout back to the active tab
        if let Some(tab) = self.active_tab_mut() {
            tab.split_layout = layout;
        }

        if ok {
            Some(new_id)
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

    /// Focus pane in a direction using geometry; returns true if focus changed
    pub fn focus_pane_direction(
        &mut self,
        dir: crate::workspace::warp_split_manager::WarpNavDirection,
    ) -> bool {
        // Compute content container rect similar to hit_test_split_divider using read-only borrows
        let si = self.size_info;
        let x0 = si.padding_x();
        let mut y0 = si.padding_y();
        let w = si.width() - 2.0 * si.padding_x();
        let mut h = si.height() - 2.0 * si.padding_y();
        if self.config.workspace.tab_bar.show
            && !self.config.workspace.warp_overlay_only
            && self.config.workspace.tab_bar.position
                != crate::config::workspace::TabBarPosition::Hidden
        {
            let is_fs = false;
            let eff_vis = match self.config.workspace.tab_bar.visibility {
                crate::config::workspace::TabBarVisibility::Always => {
                    crate::config::workspace::TabBarVisibility::Always
                }
                crate::config::workspace::TabBarVisibility::Hover => {
                    crate::config::workspace::TabBarVisibility::Hover
                }
                crate::config::workspace::TabBarVisibility::Auto => {
                    if is_fs {
                        crate::config::workspace::TabBarVisibility::Hover
                    } else {
                        crate::config::workspace::TabBarVisibility::Always
                    }
                }
            };
            if matches!(eff_vis, crate::config::workspace::TabBarVisibility::Always) {
                let ch = si.cell_height();
                match self.config.workspace.tab_bar.position {
                    crate::config::workspace::TabBarPosition::Top => {
                        y0 += ch;
                        h = (h - ch).max(0.0);
                    }
                    crate::config::workspace::TabBarPosition::Bottom => {
                        h = (h - ch).max(0.0);
                    }
                    _ => {}
                }
            }
        }
        let container = split_manager::PaneRect::new(x0, y0, w, h);

        // Snapshot active pane and compute rects without holding a mutable borrow
        let (current, rects) = if let Some(tab) = self.active_tab() {
            (tab.active_pane, self.splits.calculate_pane_rects(&tab.split_layout, container))
        } else {
            return false;
        };

        // Determine the target in the requested direction
        if let Some(cur) = rects.iter().find(|(id, _)| *id == current).map(|(_, r)| *r) {
            if let Some(target_id) = Self::find_pane_in_direction(&rects, cur, dir) {
                if target_id != current {
                    if let Some(tab_mut) = self.active_tab_mut() {
                        tab_mut.active_pane = target_id;
                        return true;
                    }
                }
            }
        }
        false
    }

    fn find_pane_in_direction(
        rects: &[(PaneId, split_manager::PaneRect)],
        cur: split_manager::PaneRect,
        dir: crate::workspace::warp_split_manager::WarpNavDirection,
    ) -> Option<PaneId> {
        use crate::workspace::warp_split_manager::WarpNavDirection as D;
        let cx = cur.x + cur.width / 2.0;
        let cy = cur.y + cur.height / 2.0;
        let mut candidates: Vec<(PaneId, f32, f32)> = Vec::new();
        for &(id, r) in rects {
            if r.x == cur.x && r.y == cur.y && r.width == cur.width && r.height == cur.height {
                continue;
            }
            let px = r.x + r.width / 2.0;
            let py = r.y + r.height / 2.0;
            let valid = match dir {
                D::Left => px < cx,
                D::Right => px > cx,
                D::Up => py < cy,
                D::Down => py > cy,
            };
            if !valid {
                continue;
            }
            let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            let align = match dir {
                D::Left | D::Right => 1.0 / (1.0 + (py - cy).abs()),
                D::Up | D::Down => 1.0 / (1.0 + (px - cx).abs()),
            };
            candidates.push((id, dist, align));
        }
        candidates.sort_by(|a, b| {
            b.2.partial_cmp(&a.2)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        });
        candidates.first().map(|(id, _, _)| *id)
    }

    pub fn focus_pane_left(&mut self) -> bool {
        self.focus_pane_direction(crate::workspace::warp_split_manager::WarpNavDirection::Left)
    }
    pub fn focus_pane_right(&mut self) -> bool {
        self.focus_pane_direction(crate::workspace::warp_split_manager::WarpNavDirection::Right)
    }
    pub fn focus_pane_up(&mut self) -> bool {
        self.focus_pane_direction(crate::workspace::warp_split_manager::WarpNavDirection::Up)
    }
    pub fn focus_pane_down(&mut self) -> bool {
        self.focus_pane_direction(crate::workspace::warp_split_manager::WarpNavDirection::Down)
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
        self.tabs.active_tab_id().map(|id| self.tabs.is_tab_zoomed(id)).unwrap_or(false)
    }

    /// Mark active tab as having last command error (non-zero exit)
    pub fn mark_active_tab_error(&mut self, non_zero: bool) {
        self.tabs.set_active_tab_last_exit(non_zero);
    }

    pub fn toggle_active_tab_sync(&mut self) -> bool {
        self.tabs.toggle_active_tab_sync()
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
            && !self.config.workspace.warp_overlay_only
            && self.config.workspace.tab_bar.position
                != crate::config::workspace::TabBarPosition::Hidden
        {
            // Only reserve a row when the tab bar is effectively visible (Always). Hover overlays
            // content.
            let is_fs = false; // WorkspaceManager doesn't know window fullscreen; assume non-FS for now
            let eff_vis = match self.config.workspace.tab_bar.visibility {
                crate::config::workspace::TabBarVisibility::Always => {
                    crate::config::workspace::TabBarVisibility::Always
                }
                crate::config::workspace::TabBarVisibility::Hover => {
                    crate::config::workspace::TabBarVisibility::Hover
                }
                crate::config::workspace::TabBarVisibility::Auto => {
                    if is_fs {
                        crate::config::workspace::TabBarVisibility::Hover
                    } else {
                        crate::config::workspace::TabBarVisibility::Always
                    }
                }
            };
            if matches!(eff_vis, crate::config::workspace::TabBarVisibility::Always) {
                let ch = si.cell_height();
                match self.config.workspace.tab_bar.position {
                    crate::config::workspace::TabBarPosition::Top => {
                        y0 += ch;
                        h = (h - ch).max(0.0);
                    }
                    crate::config::workspace::TabBarPosition::Bottom => {
                        h = (h - ch).max(0.0);
                    }
                    _ => {}
                }
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
        // Read from configuration flag `workspace.enabled`.
        self.config.workspace.enabled
    }

    /// Store a pending security confirmation
    pub fn store_pending_security_confirmation(
        &mut self,
        confirmation_id: String,
        command: String,
        dry_run: bool,
    ) {
        let confirmation =
            PendingSecurityConfirmation { command, dry_run, timestamp: std::time::Instant::now() };
        self.pending_security_confirmations.insert(confirmation_id, confirmation);

        // Clean up old confirmations (older than 5 minutes)
        self.cleanup_expired_confirmations();
    }

    /// Retrieve and remove a pending security confirmation
    pub fn consume_pending_security_confirmation(
        &mut self,
        confirmation_id: &str,
    ) -> Option<PendingSecurityConfirmation> {
        self.pending_security_confirmations.remove(confirmation_id)
    }

    /// Check if a security confirmation is pending
    pub fn has_pending_security_confirmation(&self, confirmation_id: &str) -> bool {
        self.pending_security_confirmations.contains_key(confirmation_id)
    }

    /// Clean up expired security confirmations (older than 5 minutes)
    fn cleanup_expired_confirmations(&mut self) {
        let expiry_duration = std::time::Duration::from_secs(300); // 5 minutes
        let now = std::time::Instant::now();

        self.pending_security_confirmations
            .retain(|_, confirmation| now.duration_since(confirmation.timestamp) < expiry_duration);
    }

    /// Get the number of pending security confirmations
    pub fn pending_security_confirmations_count(&self) -> usize {
        self.pending_security_confirmations.len()
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

    use crate::config::UiConfig;
    use crate::display::SizeInfo;

    fn make_size_info(
        width: f32,
        height: f32,
        cell_w: f32,
        cell_h: f32,
        pad_x: f32,
        pad_y: f32,
    ) -> SizeInfo {
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

    #[test]
    fn is_enabled_respects_config_flag() {
        // When workspace.enabled = true, is_enabled() returns true
        let mut cfg_true = UiConfig::default();
        cfg_true.workspace.enabled = true;
        let wm_true =
            make_workspace(cfg_true, make_size_info(640.0, 480.0, 10.0, 20.0, 30.0, 40.0));
        assert!(wm_true.is_enabled(), "expected workspace to be enabled");

        // When workspace.enabled = false, is_enabled() returns false
        let mut cfg_false = UiConfig::default();
        cfg_false.workspace.enabled = false;
        let wm_false =
            make_workspace(cfg_false, make_size_info(640.0, 480.0, 10.0, 20.0, 30.0, 40.0));
        assert!(
            !wm_false.is_enabled(),
            "expected workspace to be disabled when config flag is false"
        );
    }

    #[test]
    fn test_security_confirmation_management() {
        let config = UiConfig::default();
        let si = make_size_info(800.0, 600.0, 8.0, 16.0, 10.0, 10.0);
        let mut wm = make_workspace(config, si);

        // Test storing and retrieving confirmations
        let confirm_id = "test_confirmation_123".to_string();
        let command = "rm -rf /important/data".to_string();

        // Initially no confirmations
        assert_eq!(wm.pending_security_confirmations_count(), 0);
        assert!(!wm.has_pending_security_confirmation(&confirm_id));

        // Store a confirmation
        wm.store_pending_security_confirmation(confirm_id.clone(), command.clone(), false);

        // Verify it was stored
        assert_eq!(wm.pending_security_confirmations_count(), 1);
        assert!(wm.has_pending_security_confirmation(&confirm_id));

        // Consume the confirmation
        let confirmation = wm.consume_pending_security_confirmation(&confirm_id).unwrap();
        assert_eq!(confirmation.command, command);
        assert!(!confirmation.dry_run);

        // Verify it was removed
        assert_eq!(wm.pending_security_confirmations_count(), 0);
        assert!(!wm.has_pending_security_confirmation(&confirm_id));

        // Consuming again should return None
        assert!(wm.consume_pending_security_confirmation(&confirm_id).is_none());
    }

    #[test]
    fn test_security_confirmation_dry_run_flag() {
        let config = UiConfig::default();
        let si = make_size_info(800.0, 600.0, 8.0, 16.0, 10.0, 10.0);
        let mut wm = make_workspace(config, si);

        // Store a dry-run confirmation
        let confirm_id = "dry_run_test".to_string();
        wm.store_pending_security_confirmation(
            confirm_id.clone(),
            "test command".to_string(),
            true, // dry_run = true
        );

        let confirmation = wm.consume_pending_security_confirmation(&confirm_id).unwrap();
        assert!(confirmation.dry_run);
    }

    #[test]
    fn test_multiple_security_confirmations() {
        let config = UiConfig::default();
        let si = make_size_info(800.0, 600.0, 8.0, 16.0, 10.0, 10.0);
        let mut wm = make_workspace(config, si);

        // Store multiple confirmations
        wm.store_pending_security_confirmation("id1".to_string(), "cmd1".to_string(), false);
        wm.store_pending_security_confirmation("id2".to_string(), "cmd2".to_string(), true);
        wm.store_pending_security_confirmation("id3".to_string(), "cmd3".to_string(), false);

        assert_eq!(wm.pending_security_confirmations_count(), 3);

        // Consume one by one
        let conf1 = wm.consume_pending_security_confirmation("id1").unwrap();
        assert_eq!(conf1.command, "cmd1");
        assert_eq!(wm.pending_security_confirmations_count(), 2);

        let conf3 = wm.consume_pending_security_confirmation("id3").unwrap();
        assert_eq!(conf3.command, "cmd3");
        assert_eq!(wm.pending_security_confirmations_count(), 1);

        let conf2 = wm.consume_pending_security_confirmation("id2").unwrap();
        assert_eq!(conf2.command, "cmd2");
        assert!(conf2.dry_run);
        assert_eq!(wm.pending_security_confirmations_count(), 0);
    }

    #[test]
    fn test_security_confirmation_timestamp() {
        let config = UiConfig::default();
        let si = make_size_info(800.0, 600.0, 8.0, 16.0, 10.0, 10.0);
        let mut wm = make_workspace(config, si);

        let before = std::time::Instant::now();

        wm.store_pending_security_confirmation(
            "timestamp_test".to_string(),
            "test command".to_string(),
            false,
        );

        let after = std::time::Instant::now();

        let confirmation = wm.consume_pending_security_confirmation("timestamp_test").unwrap();

        // Timestamp should be between before and after
        assert!(confirmation.timestamp >= before);
        assert!(confirmation.timestamp <= after);
    }

    #[test]
    fn test_security_confirmation_cleanup_on_store() {
        let config = UiConfig::default();
        let si = make_size_info(800.0, 600.0, 8.0, 16.0, 10.0, 10.0);
        let mut wm = make_workspace(config, si);

        // Store a confirmation
        wm.store_pending_security_confirmation(
            "normal_test".to_string(),
            "normal command".to_string(),
            false,
        );

        assert_eq!(wm.pending_security_confirmations_count(), 1);

        // The cleanup is called internally, but we can't easily test expiration
        // without manipulating time, so we just verify the call doesn't break anything
        wm.store_pending_security_confirmation(
            "another_test".to_string(),
            "another command".to_string(),
            true,
        );

        assert_eq!(wm.pending_security_confirmations_count(), 2);
    }
}
