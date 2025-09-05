//! Tab management for OpenAgent Terminal
//!
//! This module handles the creation, switching, and lifecycle of tabs within a workspace.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use openagent_terminal_core::sync::FairMutex;
use openagent_terminal_core::term::Term;

use super::split_manager::{PaneId, SplitLayout};
use crate::event::EventProxy;
use crate::window_context::WindowContext;

/// Unique identifier for a tab
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TabId(pub usize);

/// Context for a single tab
pub struct TabContext {
    /// Unique identifier for this tab
    pub id: TabId,

    /// Title of the tab
    pub title: String,

    /// Working directory for this tab
    pub working_directory: PathBuf,

    /// Split layout for this tab
    pub split_layout: SplitLayout,

    /// Currently active pane in this tab
    pub active_pane: PaneId,

    /// Terminal contexts for each pane (mapped by PaneId)
    pub panes: HashMap<PaneId, PaneContext>,

    /// AI runtime state isolated to this tab
    #[cfg(feature = "ai")]
    pub ai_runtime: Option<crate::ai_runtime::AiRuntime>,

    /// Whether this tab has unsaved changes
    pub modified: bool,

    /// Shell command used to create this tab
    pub shell_command: Option<String>,

    /// Saved split layout when zoom is active; None when not zoomed
    pub zoom_saved_layout: Option<SplitLayout>,

    /// True when the last completed command in this tab exited non-zero (for error badge)
    pub last_exit_nonzero: bool,

    /// The last exit code observed in this tab, if any
    pub last_exit_code: Option<i32>,

    /// True when panes/tabs are synchronized (placeholder for sync indicator)
    pub panes_synced: bool,

    /// True while the active terminal in this tab is executing a command (heuristic)
    pub command_running: bool,
}

/// Context for a single pane within a tab
pub struct PaneContext {
    /// Terminal instance for this pane
    pub terminal: Arc<FairMutex<Term<EventProxy>>>,

    /// Window context (display, PTY, etc.)
    pub window_context: Option<WindowContext>,

    /// Pane-specific title override
    pub title_override: Option<String>,

    /// Whether this pane is currently focused
    pub focused: bool,
}

/// Tab manager handles multiple tabs and centralizes ID allocation
pub struct TabManager {
    /// All tabs indexed by ID
    tabs: HashMap<TabId, TabContext>,

    /// Order of tabs for display
    tab_order: Vec<TabId>,

    /// Currently active tab
    active_tab_id: Option<TabId>,

    /// Counter for generating unique tab IDs
    next_tab_id: usize,

    /// Counter for generating unique pane IDs (centralized allocation)
    next_pane_id: usize,
}

impl TabManager {
    /// Create a new tab manager
    pub fn new() -> Self {
        Self {
            tabs: HashMap::new(),
            tab_order: Vec::new(),
            active_tab_id: None,
            next_tab_id: 0,
            next_pane_id: 0,
        }
    }

    /// Create a new tab
    pub fn create_tab(&mut self, title: String, working_dir: Option<PathBuf>) -> TabId {
        let tab_id = TabId(self.next_tab_id);
        self.next_tab_id += 1;

        let pane_id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;

        let working_directory = working_dir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));

let tab_context = TabContext {
            id: tab_id,
            title,
            working_directory,
            split_layout: SplitLayout::Single(pane_id),
            active_pane: pane_id,
            panes: HashMap::new(),
            #[cfg(feature = "ai")]
            ai_runtime: None,
            modified: false,
            shell_command: None,
            zoom_saved_layout: None,
            last_exit_nonzero: false,
            last_exit_code: None,
            panes_synced: false,
            command_running: false,
        };

        self.tabs.insert(tab_id, tab_context);
        self.tab_order.push(tab_id);

        // If this is the first tab, make it active
        if self.active_tab_id.is_none() {
            self.active_tab_id = Some(tab_id);
        }

        tab_id
    }

    /// Close a tab by ID
    pub fn close_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(_tab) = self.tabs.remove(&tab_id) {
            // Remove from tab order
            if let Some(pos) = self.tab_order.iter().position(|&id| id == tab_id) {
                self.tab_order.remove(pos);
            }

            // Update active tab if necessary
            if self.active_tab_id == Some(tab_id) {
                if self.tab_order.is_empty() {
                    self.active_tab_id = None;
                } else {
                    // Switch to the next available tab
                    self.active_tab_id = Some(self.tab_order[0]);
                }
            }

            true
        } else {
            false
        }
    }

    /// Switch to a specific tab
    pub fn switch_to_tab(&mut self, tab_id: TabId) -> bool {
        if self.tabs.contains_key(&tab_id) {
            self.active_tab_id = Some(tab_id);
            true
        } else {
            false
        }
    }

    /// Switch to the next tab
    pub fn next_tab(&mut self) -> bool {
        if let Some(current_id) = self.active_tab_id {
            if let Some(pos) = self.tab_order.iter().position(|&id| id == current_id) {
                let next_pos = (pos + 1) % self.tab_order.len();
                self.active_tab_id = Some(self.tab_order[next_pos]);
                return true;
            }
        }
        false
    }

    /// Switch to the previous tab
    pub fn previous_tab(&mut self) -> bool {
        if let Some(current_id) = self.active_tab_id {
            if let Some(pos) = self.tab_order.iter().position(|&id| id == current_id) {
                let prev_pos = if pos == 0 { self.tab_order.len() - 1 } else { pos - 1 };
                self.active_tab_id = Some(self.tab_order[prev_pos]);
                return true;
            }
        }
        false
    }

    /// Get the currently active tab
    pub fn active_tab(&self) -> Option<&TabContext> {
        self.active_tab_id.and_then(|id| self.tabs.get(&id))
    }

    /// Get the currently active tab mutably
    pub fn active_tab_mut(&mut self) -> Option<&mut TabContext> {
        self.active_tab_id.and_then(|id| self.tabs.get_mut(&id))
    }

    /// Return whether the given tab is zoomed (has a saved layout)
    pub fn is_tab_zoomed(&self, tab_id: TabId) -> bool {
        self.tabs.get(&tab_id).map(|t| t.zoom_saved_layout.is_some()).unwrap_or(false)
    }

    /// Get a tab by ID
    pub fn get_tab(&self, tab_id: TabId) -> Option<&TabContext> {
        self.tabs.get(&tab_id)
    }

    /// Get a tab by ID mutably
    pub fn get_tab_mut(&mut self, tab_id: TabId) -> Option<&mut TabContext> {
        self.tabs.get_mut(&tab_id)
    }

    /// Set last exit status flag on active tab
    pub fn set_active_tab_last_exit(&mut self, non_zero: bool) {
        if let Some(id) = self.active_tab_id {
            if let Some(tab) = self.tabs.get_mut(&id) {
                tab.last_exit_nonzero = non_zero;
            }
        }
    }

    /// Set last exit code on the active tab and update nonzero flag
    pub fn set_active_tab_last_exit_code(&mut self, code: i32) {
        if let Some(id) = self.active_tab_id {
            if let Some(tab) = self.tabs.get_mut(&id) {
                tab.last_exit_code = Some(code);
                tab.last_exit_nonzero = code != 0;
            }
        }
    }

    /// Set last exit details on the active tab from an optional exit code
    pub fn set_active_tab_last_exit_details(&mut self, code: Option<i32>) {
        if let Some(id) = self.active_tab_id {
            if let Some(tab) = self.tabs.get_mut(&id) {
                tab.last_exit_code = code;
                tab.last_exit_nonzero = code.map(|c| c != 0).unwrap_or(false);
            }
        }
    }

    /// Get the number of tabs
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Get the tab order for display
    pub fn tab_order(&self) -> &[TabId] {
        &self.tab_order
    }

    /// Get the active tab ID
    pub fn active_tab_id(&self) -> Option<TabId> {
        self.active_tab_id
    }

    /// Toggle sync flag on active tab
    pub fn toggle_active_tab_sync(&mut self) -> bool {
        if let Some(id) = self.active_tab_id {
            if let Some(tab) = self.tabs.get_mut(&id) {
                tab.panes_synced = !tab.panes_synced;
                return true;
            }
        }
        false
    }

    /// Move a tab to a new position
    pub fn move_tab(&mut self, tab_id: TabId, new_position: usize) -> bool {
        if let Some(current_pos) = self.tab_order.iter().position(|&id| id == tab_id) {
            if new_position < self.tab_order.len() {
                let tab = self.tab_order.remove(current_pos);
                self.tab_order.insert(new_position, tab);
                return true;
            }
        }
        false
    }

    /// Rename a tab
    pub fn rename_tab(&mut self, tab_id: TabId, new_title: String) -> bool {
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.title = new_title;
            true
        } else {
            false
        }
    }

    /// Mark a tab as modified
    pub fn mark_tab_modified(&mut self, tab_id: TabId, modified: bool) -> bool {
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.modified = modified;
            true
        } else {
            false
        }
    }

    /// Get all tab IDs in order
    pub fn all_tab_ids(&self) -> Vec<TabId> {
        self.tab_order.clone()
    }

    /// Check if a tab exists
    pub fn has_tab(&self, tab_id: TabId) -> bool {
        self.tabs.contains_key(&tab_id)
    }

    /// Allocate a new unique PaneId centrally
    pub fn allocate_pane_id(&mut self) -> PaneId {
        let pane_id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;
        pane_id
    }
    
    /// Create a new pane ID (legacy method - forwards to allocate_pane_id)
    pub fn create_pane_id(&mut self) -> PaneId {
        self.allocate_pane_id()
    }
    
    /// Allocate multiple PaneIds at once for batch operations
    pub fn allocate_pane_ids(&mut self, count: usize) -> Vec<PaneId> {
        let mut pane_ids = Vec::with_capacity(count);
        for _ in 0..count {
            pane_ids.push(self.allocate_pane_id());
        }
        pane_ids
    }
    
    /// Get all pane IDs for a specific tab
    pub fn get_tab_pane_ids(&self, tab_id: TabId) -> Vec<PaneId> {
        self.tabs
            .get(&tab_id)
            .map(|tab| tab.split_layout.collect_pane_ids())
            .unwrap_or_default()
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}
