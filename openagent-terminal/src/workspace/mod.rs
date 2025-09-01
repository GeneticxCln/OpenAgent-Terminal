//! Workspace management for tabs and split panes
//!
//! This module provides built-in tab and split pane functionality for OpenAgent Terminal,
//! allowing users to manage multiple terminal sessions within a single window.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use winit::window::WindowId;

use crate::window_context::WindowContext;
use crate::config::UiConfig;
use crate::display::SizeInfo;

pub mod tab_manager;
pub mod split_manager;

pub use tab_manager::{TabContext, TabId, TabManager};
pub use split_manager::{SplitLayout, SplitManager, PaneId};

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
            config,
            size_info,
        }
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
            self.splits.split_horizontal(&mut tab.split_layout, tab.active_pane, ratio)
        } else {
            None
        }
    }
    
    /// Split the current pane vertically
    pub fn split_vertical(&mut self, ratio: f32) -> Option<PaneId> {
        if let Some(tab) = self.active_tab_mut() {
            self.splits.split_vertical(&mut tab.split_layout, tab.active_pane, ratio)
        } else {
            None
        }
    }
    
    /// Focus the next pane in the current tab
    pub fn focus_next_pane(&mut self) -> bool {
        if let Some(tab) = self.active_tab_mut() {
            self.splits.focus_next_pane(&tab.split_layout, &mut tab.active_pane)
        } else {
            false
        }
    }
    
    /// Focus the previous pane in the current tab
    pub fn focus_previous_pane(&mut self) -> bool {
        if let Some(tab) = self.active_tab_mut() {
            self.splits.focus_previous_pane(&tab.split_layout, &mut tab.active_pane)
        } else {
            false
        }
    }
    
    /// Close the current pane
    pub fn close_pane(&mut self) -> bool {
        if let Some(tab) = self.active_tab_mut() {
            // If this is the last pane in the tab, close the tab instead
            if self.splits.pane_count(&tab.split_layout) <= 1 {
                let tab_id = tab.id;
                return self.close_tab(tab_id);
            }
            
            self.splits.close_pane(&mut tab.split_layout, tab.active_pane)
        } else {
            false
        }
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

/// Workspace configuration
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    pub enabled: bool,
    pub tab_bar_position: TabBarPosition,
    pub show_tab_bar: bool,
    pub show_tab_close_button: bool,
    pub show_modified_indicator: bool,
    pub max_tab_title_length: usize,
    pub new_tab_action: NewTabAction,
    pub split_borders: bool,
    pub border_thickness: f32,
    pub default_split_ratio: f32,
    pub minimum_pane_size: usize,
    pub resize_increment: usize,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tab_bar_position: TabBarPosition::Top,
            show_tab_bar: true,
            show_tab_close_button: true,
            show_modified_indicator: true,
            max_tab_title_length: 20,
            new_tab_action: NewTabAction::InheritWorkingDir,
            split_borders: true,
            border_thickness: 1.0,
            default_split_ratio: 0.5,
            minimum_pane_size: 10,
            resize_increment: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabBarPosition {
    Top,
    Bottom,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewTabAction {
    InheritWorkingDir,
    HomeDir,
    LastUsedDir,
}
