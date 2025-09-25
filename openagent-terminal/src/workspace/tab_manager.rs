//! Tab management for OpenAgent Terminal
//!
//! This module handles the creation, switching, and lifecycle of tabs within a workspace.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
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

type TabEventCallback = Box<dyn Fn(&TabEvent) + Send + Sync>;

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
    pub ai_runtime: Option<crate::ai_runtime::AiRuntime>,

    /// Whether this tab has unsaved changes
    pub modified: bool,

    /// Shell command used to create this tab
    pub shell_command: Option<String>,

    /// Saved split layout when zoom is active; None when not zoomed
    pub zoom_saved_layout: Option<SplitLayout>,

    /// True when the last completed command in this tab exited non-zero (for error badge)
    pub last_exit_nonzero: bool,

    /// True when panes/tabs are synchronized (placeholder for sync indicator)
    pub panes_synced: bool,
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

/// Native tab manager handles multiple tabs with immediate operations and no lazy fallbacks
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

    /// Native event callbacks for real-time tab updates
    event_callbacks: Vec<TabEventCallback>,

    /// Tab animation states for immediate rendering
    animation_states: HashMap<TabId, TabAnimation>,

    /// Tab history for native navigation
    tab_history: TabHistory,

    /// Native persistence state
    persistence_enabled: bool,

    /// Tab state cache for immediate access
    cached_state: TabManagerState,
}

/// Native tab events for real-time processing
#[derive(Debug, Clone)]
pub enum TabEvent {
    Created(TabId),
    Closed(TabId),
    Activated(TabId),
    Renamed(TabId, String),
    Modified(TabId, bool),
    Moved(TabId, usize),
    SplitChanged(TabId),
    PaneCreated(TabId, super::split_manager::PaneId),
    PaneClosed(TabId, super::split_manager::PaneId),
}

/// Tab animation for native rendering
#[derive(Debug, Clone)]
pub struct TabAnimation {
    pub animation_type: TabAnimationType,
    pub start_time: std::time::Instant,
    pub duration: std::time::Duration,
    pub progress: f32,
    pub from_position: Option<usize>,
    pub to_position: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabAnimationType {
    Create,
    Close,
    Switch,
    Move,
    Highlight,
    Resize,
}

/// Tab history for native navigation
#[derive(Debug, Default)]
pub struct TabHistory {
    pub visited_tabs: Vec<TabId>,
    pub current_index: usize,
    pub max_history: usize,
}

/// Cached tab manager state for immediate access
#[derive(Debug, Clone, serde::Serialize)]
pub struct TabManagerState {
    pub tab_count: usize,
    pub active_tab: Option<TabId>,
    pub tab_titles: HashMap<TabId, String>,
    pub modified_tabs: HashSet<TabId>,
    #[serde(skip_serializing, skip_deserializing)]
    pub last_update: std::time::Instant,
}

impl Default for TabManagerState {
    fn default() -> Self {
        Self {
            tab_count: 0,
            active_tab: None,
            tab_titles: HashMap::new(),
            modified_tabs: HashSet::new(),
            last_update: std::time::Instant::now(),
        }
    }
}

impl TabHistory {
    pub fn new(max_history: usize) -> Self {
        Self { visited_tabs: Vec::new(), current_index: 0, max_history }
    }

    pub fn visit(&mut self, tab_id: TabId) {
        // Remove tab if it already exists
        self.visited_tabs.retain(|&id| id != tab_id);

        // Add to front
        self.visited_tabs.insert(0, tab_id);

        // Limit size
        if self.visited_tabs.len() > self.max_history {
            self.visited_tabs.truncate(self.max_history);
        }

        self.current_index = 0;
    }

    pub fn get_previous(&self) -> Option<TabId> {
        if self.visited_tabs.len() > 1 {
            self.visited_tabs.get(1).copied()
        } else {
            None
        }
    }

    pub fn get_recent(&self, count: usize) -> Vec<TabId> {
        self.visited_tabs.iter().take(count).copied().collect()
    }
}

impl TabManager {
    /// Create a new native tab manager with immediate operations
    pub fn new() -> Self {
        Self {
            tabs: HashMap::new(),
            tab_order: Vec::new(),
            active_tab_id: None,
            next_tab_id: 0,
            next_pane_id: 0,
            event_callbacks: Vec::new(),
            animation_states: HashMap::new(),
            tab_history: TabHistory::new(20), // Keep last 20 tabs in history
            persistence_enabled: true,
            cached_state: TabManagerState::default(),
        }
    }

    /// Register native event callback for real-time updates
    pub fn register_event_callback<F>(&mut self, callback: F)
    where
        F: Fn(&TabEvent) + Send + Sync + 'static,
    {
        self.event_callbacks.push(Box::new(callback));
    }

    /// Emit tab event immediately to all registered callbacks
    fn emit_event(&self, event: TabEvent) {
        for callback in &self.event_callbacks {
            callback(&event);
        }
    }

    /// Update cached state immediately
    fn update_cached_state(&mut self) {
        self.cached_state = TabManagerState {
            tab_count: self.tabs.len(),
            active_tab: self.active_tab_id,
            tab_titles: self.tabs.iter().map(|(&id, ctx)| (id, ctx.title.clone())).collect(),
            modified_tabs: self
                .tabs
                .iter()
                .filter_map(|(&id, ctx)| if ctx.modified { Some(id) } else { None })
                .collect(),
            last_update: std::time::Instant::now(),
        };
    }

    /// Get cached state for immediate access
    pub fn get_cached_state(&self) -> &TabManagerState {
        &self.cached_state
    }

    /// Start tab animation immediately
    fn start_tab_animation(&mut self, tab_id: TabId, animation_type: TabAnimationType) {
        let animation = TabAnimation {
            animation_type,
            start_time: std::time::Instant::now(),
            duration: match animation_type {
                TabAnimationType::Create => std::time::Duration::from_millis(200),
                TabAnimationType::Close => std::time::Duration::from_millis(150),
                TabAnimationType::Switch => std::time::Duration::from_millis(100),
                TabAnimationType::Move => std::time::Duration::from_millis(250),
                TabAnimationType::Highlight => std::time::Duration::from_millis(80),
                TabAnimationType::Resize => std::time::Duration::from_millis(120),
            },
            progress: 0.0,
            from_position: None,
            to_position: None,
        };

        self.animation_states.insert(tab_id, animation);
    }

    /// Update tab animations and return tabs that need rerendering
    pub fn update_animations(&mut self) -> Vec<TabId> {
        let mut changed_tabs = Vec::new();
        let now = std::time::Instant::now();

        let keys: Vec<TabId> = self.animation_states.keys().cloned().collect();
        for tab_id in keys {
            if let Some(anim) = self.animation_states.get(&tab_id).cloned() {
                let elapsed = now.duration_since(anim.start_time);
                let progress =
                    (elapsed.as_secs_f32() / anim.duration.as_secs_f32()).clamp(0.0, 1.0);
                if progress >= 1.0 {
                    self.animation_states.remove(&tab_id);
                    changed_tabs.push(tab_id);
                } else if let Some(anim_mut) = self.animation_states.get_mut(&tab_id) {
                    anim_mut.progress = progress;
                    changed_tabs.push(tab_id);
                }
            }
        }

        changed_tabs
    }

    /// Create a new tab with immediate native operations
    pub fn create_tab(&mut self, title: String, working_dir: Option<PathBuf>) -> TabId {
        let tab_id = TabId(self.next_tab_id);
        self.next_tab_id += 1;

        let pane_id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;

        let working_directory = working_dir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));

        let tab_context = TabContext {
            id: tab_id,
            title: title.clone(),
            working_directory,
            split_layout: SplitLayout::Single(pane_id),
            active_pane: pane_id,
            panes: HashMap::new(),
            ai_runtime: None,
            modified: false,
            shell_command: None,
            zoom_saved_layout: None,
            last_exit_nonzero: false,
            panes_synced: false,
        };

        self.tabs.insert(tab_id, tab_context);
        self.tab_order.push(tab_id);

        // If this is the first tab, make it active
        if self.active_tab_id.is_none() {
            self.active_tab_id = Some(tab_id);
            self.tab_history.visit(tab_id);
        }

        // Start creation animation immediately
        self.start_tab_animation(tab_id, TabAnimationType::Create);

        // Update cached state immediately
        self.update_cached_state();

        // Emit immediate creation event
        self.emit_event(TabEvent::Created(tab_id));

        tab_id
    }

    /// Close a tab by ID with immediate native operations
    pub fn close_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(_tab) = self.tabs.remove(&tab_id) {
            // Start close animation immediately
            self.start_tab_animation(tab_id, TabAnimationType::Close);

            // Remove from tab order
            if let Some(pos) = self.tab_order.iter().position(|&id| id == tab_id) {
                self.tab_order.remove(pos);
            }

            // Remove from history
            self.tab_history.visited_tabs.retain(|&id| id != tab_id);

            // Update active tab if necessary
            if self.active_tab_id == Some(tab_id) {
                if self.tab_order.is_empty() {
                    self.active_tab_id = None;
                } else {
                    // Prefer the most recently visited remaining tab, falling back to first available
                    let next_tab = self
                        .tab_history
                        .visited_tabs
                        .first()
                        .copied()
                        .filter(|id| self.tabs.contains_key(id))
                        .unwrap_or(self.tab_order[0]);
                    self.active_tab_id = Some(next_tab);
                    self.tab_history.visit(next_tab);
                }
            }

            // Update cached state immediately
            self.update_cached_state();

            // Emit immediate close event
            self.emit_event(TabEvent::Closed(tab_id));

            true
        } else {
            false
        }
    }

    /// Switch to a specific tab with immediate native operations
    pub fn switch_to_tab(&mut self, tab_id: TabId) -> bool {
        if self.tabs.contains_key(&tab_id) {
            let _old_tab = self.active_tab_id;
            self.active_tab_id = Some(tab_id);

            // Update tab history immediately
            self.tab_history.visit(tab_id);

            // Start switch animation immediately
            self.start_tab_animation(tab_id, TabAnimationType::Switch);

            // Update cached state immediately
            self.update_cached_state();

            // Emit immediate activation event
            self.emit_event(TabEvent::Activated(tab_id));

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

    /// Move active tab one position to the left
    pub fn move_active_tab_left(&mut self) -> bool {
        if let Some(active) = self.active_tab_id {
            if let Some(pos) = self.tab_order.iter().position(|&id| id == active) {
                let new_pos = if pos == 0 { 0 } else { pos - 1 };
                return self.move_tab(active, new_pos);
            }
        }
        false
    }

    /// Move active tab one position to the right
    pub fn move_active_tab_right(&mut self) -> bool {
        if let Some(active) = self.active_tab_id {
            if let Some(pos) = self.tab_order.iter().position(|&id| id == active) {
                let new_pos = usize::min(self.tab_order.len() - 1, pos + 1);
                return self.move_tab(active, new_pos);
            }
        }
        false
    }

    /// Move a tab to a new position with immediate native operations
    pub fn move_tab(&mut self, tab_id: TabId, new_position: usize) -> bool {
        if let Some(current_pos) = self.tab_order.iter().position(|&id| id == tab_id) {
            if new_position < self.tab_order.len() && current_pos != new_position {
                let tab = self.tab_order.remove(current_pos);
                self.tab_order.insert(new_position, tab);

                // Start move animation immediately
                if let Some(animation) = self.animation_states.get_mut(&tab_id) {
                    animation.from_position = Some(current_pos);
                    animation.to_position = Some(new_position);
                } else {
                    let animation = TabAnimation {
                        animation_type: TabAnimationType::Move,
                        start_time: std::time::Instant::now(),
                        duration: std::time::Duration::from_millis(250),
                        progress: 0.0,
                        from_position: Some(current_pos),
                        to_position: Some(new_position),
                    };
                    self.animation_states.insert(tab_id, animation);
                }

                // Update cached state immediately
                self.update_cached_state();

                // Emit immediate move event
                self.emit_event(TabEvent::Moved(tab_id, new_position));

                return true;
            }
        }
        false
    }

    /// Rename a tab with immediate native operations
    pub fn rename_tab(&mut self, tab_id: TabId, new_title: String) -> bool {
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.title = new_title.clone();

            // Update cached state immediately
            self.update_cached_state();

            // Emit immediate rename event
            self.emit_event(TabEvent::Renamed(tab_id, new_title));

            true
        } else {
            false
        }
    }

    /// Mark a tab as modified with immediate native operations
    pub fn mark_tab_modified(&mut self, tab_id: TabId, modified: bool) -> bool {
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            if tab.modified != modified {
                tab.modified = modified;

                // Update cached state immediately
                self.update_cached_state();

                // Emit immediate modification event
                self.emit_event(TabEvent::Modified(tab_id, modified));
            }

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
        self.tabs.get(&tab_id).map(|tab| tab.split_layout.collect_pane_ids()).unwrap_or_default()
    }

    /// Move a PaneContext from one tab to another.
    ///
    /// Returns true if a context was moved. If the source tab did not have a PaneContext
    /// for the given pane_id, this is a no-op and returns false.
    pub fn move_pane_context(&mut self, src_tab: TabId, dest_tab: TabId, pane_id: PaneId) -> bool {
        if src_tab == dest_tab {
            // Nothing to do
            return false;
        }
        // Take the context from source
        let moved = if let Some(src) = self.tabs.get_mut(&src_tab) {
            src.panes.remove(&pane_id)
        } else {
            None
        };
        if let Some(ctx) = moved {
            if let Some(dest) = self.tabs.get_mut(&dest_tab) {
                dest.panes.insert(pane_id, ctx);
                return true;
            } else {
                // Destination missing; put it back to source to avoid dropping context
                if let Some(src) = self.tabs.get_mut(&src_tab) {
                    src.panes.insert(pane_id, ctx);
                }
                return false;
            }
        }
        false
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}
