//! Integration layer for Warp-style functionality
//!
//! This module provides the bridge between the Warp-style managers and the main
//! OpenAgent Terminal application, handling terminal creation, event dispatching,
//! and lifecycle management.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{debug, error, info, warn};
use openagent_terminal_core::sync::FairMutex;
use openagent_terminal_core::term::Term;
use winit::event_loop::EventLoopProxy;
use winit::window::WindowId;

use crate::config::{Action, UiConfig};
use crate::event::{Event, EventProxy, EventType};
use crate::window_context::WindowContext;

use super::split_manager::PaneId;
use super::tab_manager::TabId;
use super::warp_split_manager::{WarpNavDirection, WarpResizeDirection, WarpSplitManager};
use super::warp_tab_manager::WarpTabManager;

/// Errors that can occur during Warp integration
#[derive(Debug, thiserror::Error)]
pub enum WarpIntegrationError {
    #[error("Failed to create terminal: {0}")]
    TerminalCreation(String),

    #[error("Session file error: {0}")]
    SessionFile(#[from] std::io::Error),

    #[error("Session format error: {0}")]
    SessionFormat(#[from] serde_json::Error),

    #[error("Invalid pane ID: {0:?}")]
    InvalidPaneId(PaneId),

    #[error("Invalid tab ID: {0:?}")]
    InvalidTabId(TabId),

    #[error("Window context not found for ID: {0:?}")]
    WindowNotFound(WindowId),
}

/// Result type for Warp integration operations
type WarpResult<T> = Result<T, WarpIntegrationError>;

/// Integrated Warp functionality manager
pub struct WarpIntegration {
    /// Enhanced tab management
    tab_manager: WarpTabManager,

    /// Enhanced split management
    split_manager: WarpSplitManager,

    /// Active terminal instances keyed by pane ID
    terminals: HashMap<PaneId, Arc<FairMutex<Term<EventProxy>>>>,

    /// PTY managers for each terminal  
    // pty_managers: HashMap<PaneId, Arc<PtyManager>>, // TODO: Uncomment when PtyManager is available

    /// Configuration
    config: Rc<UiConfig>,

    /// Window context for creating terminals
    window_context: Option<Arc<WindowContext>>,

    /// Event proxy for sending events
    event_proxy: Option<EventLoopProxy<Event>>,

    /// Last activity timestamp for session auto-save
    last_activity: Instant,

    /// Performance monitoring
    perf_stats: WarpPerformanceStats,
}

/// Performance statistics for monitoring
#[derive(Debug, Default)]
pub struct WarpPerformanceStats {
    pub tab_creation_time_ms: u64,
    pub split_creation_time_ms: u64,
    pub navigation_time_ms: u64,
    pub session_save_time_ms: u64,
    pub active_terminals: usize,
    pub memory_usage_kb: u64,
}

impl WarpIntegration {
    /// Create new Warp integration with session file
    pub fn new(config: Rc<UiConfig>, session_file: Option<PathBuf>) -> Self {
        let tab_manager = if let Some(session_path) = session_file {
            WarpTabManager::with_session_file(session_path)
        } else {
            WarpTabManager::new()
        };

        Self {
            tab_manager,
            split_manager: WarpSplitManager::new(),
            terminals: HashMap::new(),
            // pty_managers: HashMap::new(), // TODO: Uncomment when available
            config,
            window_context: None,
            event_proxy: None,
            last_activity: Instant::now(),
            perf_stats: WarpPerformanceStats::default(),
        }
    }

    /// Initialize with window context and event proxy
    pub fn initialize(
        &mut self,
        window_context: Arc<WindowContext>,
        event_proxy: EventLoopProxy<Event>,
    ) -> WarpResult<()> {
        self.window_context = Some(window_context);
        self.event_proxy = Some(event_proxy);

        // Try to load previous session
        match self.tab_manager.load_session() {
            Ok(true) => {
                info!("Loaded Warp session successfully");
                self.restore_session_terminals()?;
            },
            Ok(false) => {
                info!("No previous Warp session found, creating default tab");
                self.create_default_tab()?;
            },
            Err(e) => {
                warn!("Failed to load Warp session: {}, creating default tab", e);
                self.create_default_tab()?;
            },
        }

        Ok(())
    }

    /// Create default tab when no session exists
    fn create_default_tab(&mut self) -> WarpResult<TabId> {
        let start = Instant::now();

        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        let tab_id = self.tab_manager.create_warp_tab(Some(current_dir));

        // Create the actual terminal for the default pane
        if let Some(tab) = self.tab_manager.active_tab() {
            let active_pane = tab.active_pane;
            let working_dir = tab.working_directory.clone();
            self.create_terminal_for_pane(active_pane, &working_dir)?;
        }

        self.perf_stats.tab_creation_time_ms = start.elapsed().as_millis() as u64;
        self.update_activity();

        Ok(tab_id)
    }

    /// Restore terminals from loaded session
    fn restore_session_terminals(&mut self) -> WarpResult<()> {
        // TODO: Implement session restoration when tab_manager methods are available
        info!("Session restoration not yet implemented");
        Ok(())
    }

    /// Create actual terminal instance for a pane
    fn create_terminal_for_pane(&mut self, pane_id: PaneId, working_dir: &Path) -> WarpResult<()> {
        let Some(window_context) = &self.window_context else {
            return Err(WarpIntegrationError::TerminalCreation(
                "Window context not initialized".to_string(),
            ));
        };

        let Some(event_proxy) = &self.event_proxy else {
            return Err(WarpIntegrationError::TerminalCreation(
                "Event proxy not initialized".to_string(),
            ));
        };

        // Create EventProxy for this terminal
        let terminal_event_proxy = EventProxy::new(event_proxy.clone(), window_context.id());

        // Create terminal configuration
        let term_config = openagent_terminal_core::term::Config::default();
        let size_info = window_context.display.size_info;

        // Create terminal instance
        let terminal =
            Arc::new(FairMutex::new(Term::new(term_config, &size_info, terminal_event_proxy)));

        // Store terminal reference
        self.terminals.insert(pane_id, terminal);

        // TODO: Create and store PTY manager
        // let pty_manager = Arc::new(PtyManager::new(pane_id, working_dir)?);
        // self.pty_managers.insert(pane_id, pty_manager);

        self.perf_stats.active_terminals = self.terminals.len();

        debug!("Created terminal for pane {:?} in {}", pane_id, working_dir.display());
        Ok(())
    }

    /// Handle Warp-style action execution
    pub fn execute_warp_action(&mut self, action: &WarpAction) -> WarpResult<bool> {
        let start = Instant::now();

        let result = match action {
            WarpAction::CreateTab => self.handle_create_tab(),
            WarpAction::CloseTab => self.handle_close_tab(),
            WarpAction::NextTab => self.handle_next_tab(),
            WarpAction::PreviousTab => self.handle_previous_tab(),
            WarpAction::SplitRight => self.handle_split_right(),
            WarpAction::SplitDown => self.handle_split_down(),
            WarpAction::NavigatePane(direction) => self.handle_navigate_pane(*direction),
            WarpAction::ResizePane(direction) => self.handle_resize_pane(*direction),
            WarpAction::ZoomPane => self.handle_zoom_pane(),
            WarpAction::CycleRecentPanes => self.handle_cycle_recent_panes(),
            WarpAction::EqualizeSplits => self.handle_equalize_splits(),
            WarpAction::ClosePane => self.handle_close_pane(),
            WarpAction::SaveSession => self.handle_save_session(),
            WarpAction::LoadSession => self.handle_load_session(),
        };

        // Update performance stats
        let elapsed = start.elapsed().as_millis() as u64;
        match action {
            WarpAction::NavigatePane(_) => self.perf_stats.navigation_time_ms = elapsed,
            WarpAction::SplitRight | WarpAction::SplitDown => {
                self.perf_stats.split_creation_time_ms = elapsed
            },
            WarpAction::SaveSession | WarpAction::LoadSession => {
                self.perf_stats.session_save_time_ms = elapsed
            },
            _ => {},
        }

        self.update_activity();
        result
    }

    /// Handle tab creation
    fn handle_create_tab(&mut self) -> WarpResult<bool> {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        let tab_id = self.tab_manager.create_warp_tab(Some(current_dir.clone()));

        // Send UI update event
        self.send_ui_update_event(WarpUiUpdateType::TabCreated(tab_id));

        // TODO: Create terminal when tab structure is available
        info!("Created Warp tab {:?}", tab_id);
        Ok(true)
    }

    /// Handle tab closing
    fn handle_close_tab(&mut self) -> WarpResult<bool> {
        if let Some(active_tab) = self.tab_manager.active_tab() {
            let tab_id = active_tab.id;
            let tab_title = active_tab.title.clone();
            let ok = self.tab_manager.close_warp_tab(tab_id);

            let msg = if ok {
                format!("Closed Warp tab '{}'", tab_title)
            } else {
                "Close Warp tab failed".into()
            };

            info!("{}", msg);

            if ok {
                // Cleanup terminal for the closed tab's panes
                // TODO: Get panes from tab before closing and cleanup

                // Send UI update event
                self.send_ui_update_event(WarpUiUpdateType::TabClosed(tab_id));
            }

            Ok(ok)
        } else {
            info!("No active tab to close");
            Ok(false)
        }
    }

    /// Handle split right operation
    fn handle_split_right(&mut self) -> WarpResult<bool> {
        // TODO: Implement when split functionality is available
        info!("Split right not yet implemented");
        Ok(false)
    }

    /// Handle split down operation  
    fn handle_split_down(&mut self) -> WarpResult<bool> {
        // TODO: Implement when split functionality is available
        info!("Split down not yet implemented");
        Ok(false)
    }

    /// Handle pane navigation
    fn handle_navigate_pane(&mut self, _direction: WarpNavDirection) -> WarpResult<bool> {
        // TODO: Implement when navigation APIs are available
        info!("Navigate pane not yet implemented");
        Ok(false)
    }

    /// Handle pane resizing
    fn handle_resize_pane(&mut self, _direction: WarpResizeDirection) -> WarpResult<bool> {
        // TODO: Implement when resize APIs are available
        info!("Resize pane not yet implemented");
        Ok(false)
    }

    /// Handle pane zoom toggle
    fn handle_zoom_pane(&mut self) -> WarpResult<bool> {
        // TODO: Implement when zoom APIs are available
        info!("Zoom pane not yet implemented");
        Ok(false)
    }

    /// Handle recent pane cycling
    fn handle_cycle_recent_panes(&mut self) -> WarpResult<bool> {
        // TODO: Implement when cycle APIs are available
        info!("Cycle recent panes not yet implemented");
        Ok(false)
    }

    /// Handle split equalization
    fn handle_equalize_splits(&mut self) -> WarpResult<bool> {
        // TODO: Implement when equalize APIs are available
        info!("Equalize splits not yet implemented");
        Ok(false)
    }

    /// Handle pane closing
    fn handle_close_pane(&mut self) -> WarpResult<bool> {
        // TODO: Implement when close APIs are available
        info!("Close pane not yet implemented");
        Ok(false)
    }

    /// Handle session saving
    fn handle_save_session(&mut self) -> WarpResult<bool> {
        let start = Instant::now();

        match self.tab_manager.save_session() {
            Ok(()) => {
                self.perf_stats.session_save_time_ms = start.elapsed().as_millis() as u64;
                info!("Warp session saved successfully");
                Ok(true)
            },
            Err(e) => {
                error!("Failed to save Warp session: {}", e);
                Err(WarpIntegrationError::SessionFile(e))
            },
        }
    }

    /// Handle session loading
    fn handle_load_session(&mut self) -> WarpResult<bool> {
        // Clean up current state first
        self.cleanup_all_terminals();

        match self.tab_manager.load_session() {
            Ok(true) => {
                self.restore_session_terminals()?;
                info!("Warp session loaded successfully");
                Ok(true)
            },
            Ok(false) => {
                self.create_default_tab()?;
                Ok(false)
            },
            Err(e) => Err(WarpIntegrationError::SessionFile(e)),
        }
    }

    /// Handle other standard actions
    fn handle_next_tab(&mut self) -> WarpResult<bool> {
        let ok = self.tab_manager.next_tab();
        let msg = if ok { "Switched to next tab" } else { "Switch to next tab failed" };
        info!("{}", msg);

        if ok {
            // Send UI update event
            if let Some(active_tab) = self.tab_manager.active_tab() {
                self.send_ui_update_event(WarpUiUpdateType::TabSwitched { tab_id: active_tab.id });
            }
        }

        Ok(ok)
    }

    fn handle_previous_tab(&mut self) -> WarpResult<bool> {
        let ok = self.tab_manager.previous_tab();
        let msg = if ok { "Switched to previous tab" } else { "Switch to previous tab failed" };
        info!("{}", msg);

        if ok {
            // Send UI update event
            if let Some(active_tab) = self.tab_manager.active_tab() {
                self.send_ui_update_event(WarpUiUpdateType::TabSwitched { tab_id: active_tab.id });
            }
        }

        Ok(ok)
    }

    /// Cleanup terminal resources for a pane
    fn cleanup_pane(&mut self, pane_id: PaneId) {
        if let Some(_terminal) = self.terminals.remove(&pane_id) {
            // Terminal will be dropped here, cleaning up resources
            debug!("Cleaned up terminal for pane {:?}", pane_id);
        }

        // if let Some(pty_manager) = self.pty_managers.remove(&pane_id) {
        //     // PTY manager cleanup would go here
        //     debug!("Cleaned up PTY manager for pane {:?}", pane_id);
        // }

        self.perf_stats.active_terminals = self.terminals.len();
    }

    /// Clean up all terminals
    fn cleanup_all_terminals(&mut self) {
        let pane_ids: Vec<PaneId> = self.terminals.keys().copied().collect();
        for pane_id in pane_ids {
            self.cleanup_pane(pane_id);
        }
    }

    /// Update activity timestamp
    fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Send UI update event through the event proxy
    fn send_ui_update_event(&self, update_type: WarpUiUpdateType) {
        if let Some(proxy) = &self.event_proxy {
            let event = Event::new(
                EventType::WarpUiUpdate(update_type),
                self.window_context.as_ref().unwrap().id(),
            );
            let _ = proxy.send_event(event);
        }
    }

    /// Get reference to active terminal for current pane
    pub fn active_terminal(&self) -> Option<&Arc<FairMutex<Term<EventProxy>>>> {
        let active_tab = self.tab_manager.active_tab()?;
        self.terminals.get(&active_tab.active_pane)
    }

    /// Update command for current tab (for smart tab naming)
    pub fn update_current_command(&mut self, command: &str) {
        if let Some(active_tab) = self.tab_manager.active_tab() {
            self.tab_manager.update_tab_for_command(active_tab.id, command);
            self.update_activity();
        }
    }

    /// Get performance statistics
    pub fn performance_stats(&self) -> &WarpPerformanceStats {
        &self.perf_stats
    }

    /// Check if auto-save is needed
    pub fn should_auto_save(&self) -> bool {
        self.last_activity.elapsed() > Duration::from_secs(30)
    }

    /// Get current tab and split layout info for debugging
    pub fn debug_info(&self) -> WarpDebugInfo {
        WarpDebugInfo {
            tab_count: self.tab_manager.tab_count(),
            active_tab_id: self.tab_manager.active_tab().map(|t| t.id),
            active_pane_count: self
                .tab_manager
                .active_tab()
                .map(|t| t.split_layout.pane_count())
                .unwrap_or(0),
            terminal_count: self.terminals.len(),
            memory_usage_estimate: self.estimate_memory_usage(),
        }
    }

    /// Estimate memory usage for performance monitoring
    fn estimate_memory_usage(&self) -> u64 {
        // Rough estimate: 50KB per terminal + 10KB per tab + base overhead
        let terminal_memory = self.terminals.len() as u64 * 50 * 1024;
        let tab_memory = self.tab_manager.tab_count() as u64 * 10 * 1024;
        let base_memory = 100 * 1024; // 100KB base

        terminal_memory + tab_memory + base_memory
    }
}

/// Warp-specific actions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarpAction {
    CreateTab,
    CloseTab,
    NextTab,
    PreviousTab,
    SplitRight,
    SplitDown,
    NavigatePane(WarpNavDirection),
    ResizePane(WarpResizeDirection),
    ZoomPane,
    CycleRecentPanes,
    EqualizeSplits,
    ClosePane,
    SaveSession,
    LoadSession,
}

/// UI update events for Warp functionality
#[derive(Debug, Clone)]
pub enum WarpUiUpdateType {
    TabCreated(TabId),
    TabClosed(TabId),
    TabSwitched { tab_id: TabId },
    PaneSplit { tab_id: TabId, new_pane_id: PaneId },
    PaneFocused { tab_id: TabId, pane_id: PaneId },
    PaneResized { tab_id: TabId, pane_id: PaneId },
    PaneZoomed { tab_id: TabId, pane_id: PaneId, zoomed: bool },
    PaneClosed { tab_id: TabId, closed_pane_id: PaneId, new_active_pane_id: PaneId },
    SplitsEqualized { tab_id: TabId },
}

/// Debug information for troubleshooting
#[derive(Debug, Clone)]
pub struct WarpDebugInfo {
    pub tab_count: usize,
    pub active_tab_id: Option<TabId>,
    pub active_pane_count: usize,
    pub terminal_count: usize,
    pub memory_usage_estimate: u64,
}

/// Extension trait for standard Action enum to include Warp actions
pub trait ActionExt {
    fn to_warp_action(&self) -> Option<WarpAction>;
}

impl ActionExt for Action {
    fn to_warp_action(&self) -> Option<WarpAction> {
        match self {
            Action::CreateTab => Some(WarpAction::CreateTab),
            Action::CloseTab => Some(WarpAction::CloseTab),
            Action::NextTab => Some(WarpAction::NextTab),
            Action::PreviousTab => Some(WarpAction::PreviousTab),
            Action::SplitHorizontal => Some(WarpAction::SplitRight),
            Action::SplitVertical => Some(WarpAction::SplitDown),
            Action::FocusNextPane => Some(WarpAction::NavigatePane(WarpNavDirection::Right)),
            Action::FocusPreviousPane => Some(WarpAction::NavigatePane(WarpNavDirection::Left)),
            Action::ResizePaneLeft => Some(WarpAction::ResizePane(WarpResizeDirection::ExpandLeft)),
            Action::ResizePaneRight => {
                Some(WarpAction::ResizePane(WarpResizeDirection::ExpandRight))
            },
            Action::ResizePaneUp => Some(WarpAction::ResizePane(WarpResizeDirection::ExpandUp)),
            Action::ResizePaneDown => Some(WarpAction::ResizePane(WarpResizeDirection::ExpandDown)),
            Action::ClosePane => Some(WarpAction::ClosePane),
            _ => None,
        }
    }
}
