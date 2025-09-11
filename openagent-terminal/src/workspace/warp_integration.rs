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

use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::tty::pty_manager::{PtyAiContext, PtyManagerCollection, ShellConfig};

use log::{debug, error, info, warn};
use openagent_terminal_core::event::WindowSize;
use openagent_terminal_core::sync::FairMutex;
use openagent_terminal_core::term::Term;
use winit::event_loop::EventLoopProxy;
use winit::window::WindowId;

use crate::config::{Action, UiConfig};
use crate::display::SizeInfo;
use crate::event::{Event, EventProxy, EventType};

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

    // Session restoration specific errors
    #[error("Session restoration failed: {0}")]
    SessionRestore(String),

    #[error("Invalid session format version: expected {expected}, found {actual}")]
    SessionVersion { expected: String, actual: String },

    #[error("PTY creation failed for pane {pane_id:?}: {reason}")]
    PtyCreation { pane_id: PaneId, reason: String },

    #[error("Working directory not accessible: {path} - {reason}")]
    WorkingDirectoryError { path: String, reason: String },

    #[error("Partial session restore: {restored} of {total} panes restored successfully")]
    PartialRestore { restored: usize, total: usize },

    #[error("Session file corrupted: {0}")]
    SessionCorrupted(String),

    #[error("PTY manager error: {0}")]
    PtyManager(#[from] openagent_terminal_core::tty::pty_manager::PtyManagerError),
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
    pty_managers: PtyManagerCollection,

    /// Configuration
    config: Rc<UiConfig>,

    /// Window ID for event routing
    window_id: Option<WindowId>,

    /// Cached size info for terminal creation
    size_info: Option<SizeInfo>,

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
            pty_managers: PtyManagerCollection::new(),
            config,
            window_id: None,
            size_info: None,
            event_proxy: None,
            last_activity: Instant::now(),
            perf_stats: WarpPerformanceStats::default(),
        }
    }

    /// Initialize with window context and event proxy
    pub fn initialize(
        &mut self,
        window_id: WindowId,
        event_proxy: EventLoopProxy<Event>,
        size_info: SizeInfo,
        restore_on_startup: bool,
    ) -> WarpResult<()> {
        self.window_id = Some(window_id);
        self.size_info = Some(size_info);
        self.event_proxy = Some(event_proxy);

        // Try to load previous session based on setting
        if restore_on_startup {
            match self.tab_manager.load_session() {
                Ok(true) => {
                    info!("Loaded Warp session successfully");
                    self.restore_session_terminals()?;
                }
                Ok(false) => {
                    info!("No previous Warp session found, creating default tab");
                    self.create_default_tab()?;
                }
                Err(e) => {
                    warn!("Failed to load Warp session: {}, creating default tab", e);
                    self.create_default_tab()?;
                }
            }
        } else {
            info!("Session restore disabled; creating default tab");
            self.create_default_tab()?;
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
        let start = Instant::now();
        let mut restored_panes = 0;
        let mut total_panes = 0;
        let mut errors = Vec::new();

        info!("Starting session restoration...");

        // Snapshot tab info to avoid borrowing conflicts
        let tabs_snapshot: Vec<(TabId, PathBuf, super::split_manager::SplitLayout, String)> = self
            .tab_manager
            .all_tabs()
            .map(|tab| {
                (
                    tab.id,
                    tab.working_directory.clone(),
                    tab.split_layout.clone(),
                    tab.title.clone(),
                )
            })
            .collect();

        // Iterate through snapshot and perform mutations/restoration
        for (tab_id, working_dir, split_layout, title) in tabs_snapshot {
            // Determine working directory to use
            let mut effective_dir = working_dir.clone();
            if !working_dir.exists() {
                let error_msg = format!(
                    "Working directory no longer exists: {}",
                    working_dir.display()
                );
                warn!("{}", error_msg);
                errors.push(WarpIntegrationError::WorkingDirectoryError {
                    path: working_dir.to_string_lossy().to_string(),
                    reason: "Directory not found".to_string(),
                });

                // Try to fallback to home directory and update manager state
                let fallback_dir = std::env::var("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("/"));
                effective_dir = fallback_dir.clone();
                self.tab_manager
                    .update_tab_working_directory(tab_id, fallback_dir);
            }

            // Collect all pane IDs from the split layout
            let pane_ids = split_layout.collect_pane_ids();
            total_panes += pane_ids.len();

            debug!("Restoring {} panes for tab '{}'...", pane_ids.len(), title);

            // Restore each pane in this tab
            for pane_id in pane_ids {
                match self.restore_pane_terminal(pane_id, &effective_dir, tab_id) {
                    Ok(()) => {
                        restored_panes += 1;
                        debug!("Successfully restored pane {:?}", pane_id);
                    }
                    Err(e) => {
                        error!("Failed to restore pane {:?}: {}", pane_id, e);
                        errors.push(e);
                    }
                }
            }
        }

        let elapsed = start.elapsed();
        self.perf_stats.session_save_time_ms = elapsed.as_millis() as u64;

        // Report restoration results
        if restored_panes == total_panes {
            info!(
                "Session restoration completed successfully: {} panes restored in {}ms",
                restored_panes,
                elapsed.as_millis()
            );
            Ok(())
        } else if restored_panes > 0 {
            warn!(
                "Partial session restoration: {}/{} panes restored in {}ms",
                restored_panes,
                total_panes,
                elapsed.as_millis()
            );
            // Return partial restore error but don't fail completely
            Err(WarpIntegrationError::PartialRestore {
                restored: restored_panes,
                total: total_panes,
            })
        } else {
            error!(
                "Session restoration failed completely: 0/{} panes restored",
                total_panes
            );
            Err(WarpIntegrationError::SessionRestore(format!(
                "Failed to restore any panes. Errors: {}",
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )))
        }
    }

    /// Restore terminal for a specific pane (used during session restoration)
    fn restore_pane_terminal(
        &mut self,
        pane_id: PaneId,
        working_dir: &Path,
        tab_id: TabId,
    ) -> WarpResult<()> {
        // Check if terminal already exists for this pane (avoid duplicates)
        if self.terminals.contains_key(&pane_id) {
            debug!("Terminal already exists for pane {:?}, skipping", pane_id);
            return Ok(());
        }

        // Create the terminal and PTY
        self.create_terminal_for_pane(pane_id, working_dir)?;

        // Create a minimal pane context for the restored pane
        let pane_context = super::tab_manager::PaneContext {
            terminal: self.terminals.get(&pane_id).unwrap().clone(),
            window_context: None, // Will be set up later during window management
            title_override: None,
            focused: false, // Will be updated based on active pane
        };

        // Add the pane to the tab
        if !self
            .tab_manager
            .add_pane_to_tab(tab_id, pane_id, pane_context)
        {
            return Err(WarpIntegrationError::InvalidTabId(tab_id));
        }

        debug!(
            "Successfully restored terminal for pane {:?} in tab {:?}",
            pane_id, tab_id
        );

        Ok(())
    }

    /// Create actual terminal instance for a pane
    fn create_terminal_for_pane(&mut self, pane_id: PaneId, working_dir: &Path) -> WarpResult<()> {
        let Some(window_id) = &self.window_id else {
            return Err(WarpIntegrationError::TerminalCreation(
                "Window id not initialized".to_string(),
            ));
        };

        let Some(size_info) = &self.size_info else {
            return Err(WarpIntegrationError::TerminalCreation(
                "SizeInfo not initialized".to_string(),
            ));
        };

        let Some(event_proxy) = &self.event_proxy else {
            return Err(WarpIntegrationError::TerminalCreation(
                "Event proxy not initialized".to_string(),
            ));
        };

        // Create EventProxy for this terminal
        let terminal_event_proxy = EventProxy::new(event_proxy.clone(), *window_id);

        // Create terminal configuration
        let term_config = openagent_terminal_core::term::Config::default();
        let size_info = *size_info;

        // Create terminal instance
        let terminal = Arc::new(FairMutex::new(Term::new(
            term_config,
            &size_info,
            terminal_event_proxy,
        )));

        // Store terminal reference
        self.terminals.insert(pane_id, terminal);

        // Create and store PTY manager with better error handling
        let shell_config = ShellConfig {
            executable: self
                .config
                .terminal
                .shell
                .as_ref()
                .map(|s| s.program().to_string())
                .unwrap_or_else(|| {
                    // Platform-specific shell defaults
                    #[cfg(target_os = "windows")]
                    {
                        "powershell.exe".to_string()
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string())
                    }
                }),
            args: self
                .config
                .terminal
                .shell
                .as_ref()
                .map(|s| s.args().to_vec())
                .unwrap_or_else(|| {
                    #[cfg(target_os = "windows")]
                    {
                        vec!["-NoProfile".to_string()]
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        vec!["-l".to_string()]
                    }
                }),
            env_vars: HashMap::new(),
            prompt_pattern: None,
        };

        // Build environment variables
        let mut environment = HashMap::new();

        // Set working directory in environment
        environment.insert("PWD".to_string(), working_dir.to_string_lossy().to_string());

        // Add shell integration environment if available
        if let Some(integration_path) = self.get_shell_integration_path() {
            environment.insert(
                "OPENAGENT_TERMINAL_SHELL_INTEGRATION".to_string(),
                integration_path,
            );
        }

        let pty_id = self
            .pty_managers
            .create_pty_manager(working_dir.to_path_buf(), shell_config, environment)
            .map_err(|e| WarpIntegrationError::PtyCreation {
                pane_id,
                reason: e.to_string(),
            })?;

        // Create actual PTY process with window size conversion
        if let Some(manager) = self.pty_managers.get_manager(pty_id) {
            let mut manager_guard = manager.lock();

            // Convert SizeInfo to WindowSize
            let window_size = WindowSize {
                num_lines: size_info.screen_lines() as u16,
                num_cols: size_info.columns() as u16,
                cell_width: size_info.cell_width() as u16,
                cell_height: size_info.cell_height() as u16,
            };

            manager_guard
                .create_pty(window_size, (*window_id).into())
                .map_err(|e| WarpIntegrationError::PtyCreation {
                    pane_id,
                    reason: e.to_string(),
                })?;

            debug!(
                "Created PTY for pane {:?} with shell: {} in {}",
                pane_id,
                manager_guard.context.shell_config.executable,
                working_dir.display()
            );
        } else {
            return Err(WarpIntegrationError::PtyCreation {
                pane_id,
                reason: "PTY manager not found after creation".to_string(),
            });
        }

        self.perf_stats.active_terminals = self.terminals.len();

        debug!(
            "Created terminal and PTY manager for pane {:?} in {}",
            pane_id,
            working_dir.display()
        );
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
            }
            WarpAction::SaveSession | WarpAction::LoadSession => {
                self.perf_stats.session_save_time_ms = elapsed
            }
            _ => {}
        }

        self.update_activity();
        result
    }

    /// Handle tab creation
    fn handle_create_tab(&mut self) -> WarpResult<bool> {
        let start = Instant::now();
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

        let tab_id = self.tab_manager.create_warp_tab(Some(current_dir.clone()));

        // If runtime is initialized, create the initial terminal for the new tab's active pane
        if let Some(tab) = self.tab_manager.active_tab() {
            let active_pane = tab.active_pane;
            let working_dir = tab.working_directory.clone();

            if self.window_id.is_some() && self.size_info.is_some() && self.event_proxy.is_some() {
                // Create terminal + PTY for the initial pane
                self.create_terminal_for_pane(active_pane, &working_dir)?;

                // Register a minimal PaneContext for the tab (window context will be wired elsewhere)
                let pane_context = super::tab_manager::PaneContext {
                    terminal: self.terminals.get(&active_pane).unwrap().clone(),
                    window_context: None,
                    title_override: None,
                    focused: true,
                };
                let _ = self
                    .tab_manager
                    .add_pane_to_tab(tab_id, active_pane, pane_context);
            } else {
                info!(
                    "Warp not initialized; created tab {:?} without spawning terminal (will be restored later)",
                    tab_id
                );
            }
        }

        // Send UI update event
        self.send_ui_update_event(WarpUiUpdateType::TabCreated(tab_id));

        // Update perf stats and activity
        self.perf_stats.tab_creation_time_ms = start.elapsed().as_millis() as u64;
        self.update_activity();

        info!("Created Warp tab {:?}", tab_id);
        Ok(true)
    }

    /// Handle tab closing
    fn handle_close_tab(&mut self) -> WarpResult<bool> {
        if let Some(active_tab) = self.tab_manager.active_tab() {
            let tab_id = active_tab.id;
            let tab_title = active_tab.title.clone();

            // Snapshot pane IDs before closing the tab
            let pane_ids = active_tab.split_layout.collect_pane_ids();

            // Close the tab in the manager
            let ok = self.tab_manager.close_warp_tab(tab_id);

            let msg = if ok {
                format!("Closed Warp tab '{}'", tab_title)
            } else {
                "Close Warp tab failed".into()
            };

            info!("{}", msg);

            if ok {
                // Cleanup terminals/PTYS for the closed tab's panes
                for pane_id in pane_ids {
                    self.cleanup_pane(pane_id);
                }

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
        // Snapshot needed fields to avoid borrowing conflicts
        let (tab_id, active_pane_id, working_dir, mut layout) = {
            let Some(active_tab) = self.tab_manager.active_tab() else {
                info!("No active tab for split right");
                return Ok(false);
            };
            (
                active_tab.id,
                active_tab.active_pane,
                active_tab.working_directory.clone(),
                active_tab.split_layout.clone(),
            )
        };

        // Generate new pane ID
        let new_pane_id = self.generate_pane_id();

        // Create split in the layout
        let split_success =
            self.split_manager
                .split_right(&mut layout, active_pane_id, new_pane_id);

        if split_success {
            // Update the tab with new layout
            self.tab_manager.update_tab_split_layout(tab_id, layout);

            // Create terminal for new pane if initialized; otherwise skip gracefully
            if self.window_id.is_some() && self.size_info.is_some() && self.event_proxy.is_some() {
                self.create_terminal_for_pane(new_pane_id, &working_dir)?;
            } else {
                info!(
                    "Warp not initialized; skipping terminal creation for pane {:?}",
                    new_pane_id
                );
            }

            // Send UI update event
            self.send_ui_update_event(WarpUiUpdateType::PaneSplit {
                tab_id,
                new_pane_id,
            });

            info!("Split right created pane {:?}", new_pane_id);
            Ok(true)
        } else {
            info!("Split right failed");
            Ok(false)
        }
    }

    /// Handle split down operation
    fn handle_split_down(&mut self) -> WarpResult<bool> {
        // Snapshot needed fields to avoid borrowing conflicts
        let (tab_id, active_pane_id, working_dir, mut layout) = {
            let Some(active_tab) = self.tab_manager.active_tab() else {
                info!("No active tab for split down");
                return Ok(false);
            };
            (
                active_tab.id,
                active_tab.active_pane,
                active_tab.working_directory.clone(),
                active_tab.split_layout.clone(),
            )
        };

        // Generate new pane ID
        let new_pane_id = self.generate_pane_id();

        // Create split in the layout
        let split_success = self
            .split_manager
            .split_down(&mut layout, active_pane_id, new_pane_id);

        if split_success {
            // Update the tab with new layout
            self.tab_manager.update_tab_split_layout(tab_id, layout);

            // Create terminal for new pane if initialized; otherwise skip gracefully
            if self.window_id.is_some() && self.size_info.is_some() && self.event_proxy.is_some() {
                self.create_terminal_for_pane(new_pane_id, &working_dir)?;
            } else {
                info!(
                    "Warp not initialized; skipping terminal creation for pane {:?}",
                    new_pane_id
                );
            }

            // Send UI update event
            self.send_ui_update_event(WarpUiUpdateType::PaneSplit {
                tab_id,
                new_pane_id,
            });

            info!("Split down created pane {:?}", new_pane_id);
            Ok(true)
        } else {
            info!("Split down failed");
            Ok(false)
        }
    }

    /// Handle pane navigation
    fn handle_navigate_pane(&mut self, direction: WarpNavDirection) -> WarpResult<bool> {
        // Snapshot needed fields to avoid borrowing conflicts
        let (tab_id, mut current_pane, layout_clone) = {
            let Some(active_tab) = self.tab_manager.active_tab() else {
                info!("No active tab for navigate pane");
                return Ok(false);
            };
            (
                active_tab.id,
                active_tab.active_pane,
                active_tab.split_layout.clone(),
            )
        };

        let navigation_success =
            self.split_manager
                .navigate_pane(&layout_clone, &mut current_pane, direction);

        if navigation_success {
            // Update active pane in tab
            self.tab_manager.set_active_pane(tab_id, current_pane);

            // Send UI update event
            self.send_ui_update_event(WarpUiUpdateType::PaneFocused {
                tab_id,
                pane_id: current_pane,
            });

            info!("Navigated to pane {:?}", current_pane);
            Ok(true)
        } else {
            info!(
                "Navigate pane failed - no pane in {:?} direction",
                direction
            );
            Ok(false)
        }
    }

    /// Handle pane resizing
    fn handle_resize_pane(&mut self, direction: WarpResizeDirection) -> WarpResult<bool> {
        // Snapshot needed fields to avoid borrowing conflicts
        let (tab_id, current_pane, mut layout) = {
            let Some(active_tab) = self.tab_manager.active_tab() else {
                info!("No active tab for resize pane");
                return Ok(false);
            };
            (
                active_tab.id,
                active_tab.active_pane,
                active_tab.split_layout.clone(),
            )
        };

        let resize_success = self
            .split_manager
            .resize_pane(&mut layout, current_pane, direction);

        if resize_success {
            // Update the tab with new layout
            self.tab_manager.update_tab_split_layout(tab_id, layout);

            // Send UI update event
            self.send_ui_update_event(WarpUiUpdateType::PaneResized {
                tab_id,
                pane_id: current_pane,
            });

            info!("Resized pane {:?}", current_pane);
            Ok(true)
        } else {
            info!("Resize pane failed");
            Ok(false)
        }
    }

    /// Handle pane zoom toggle
    fn handle_zoom_pane(&mut self) -> WarpResult<bool> {
        // Snapshot needed fields to avoid borrowing conflicts
        let (tab_id, current_pane, mut layout) = {
            let Some(active_tab) = self.tab_manager.active_tab() else {
                info!("No active tab for zoom pane");
                return Ok(false);
            };
            (
                active_tab.id,
                active_tab.active_pane,
                active_tab.split_layout.clone(),
            )
        };

        let zoom_success = self
            .split_manager
            .toggle_pane_zoom(&mut layout, current_pane);

        if zoom_success {
            // Update the tab with new layout
            self.tab_manager.update_tab_split_layout(tab_id, layout);

            let is_zoomed = self.split_manager.is_pane_zoomed(current_pane);

            // Send UI update event
            self.send_ui_update_event(WarpUiUpdateType::PaneZoomed {
                tab_id,
                pane_id: current_pane,
                zoomed: is_zoomed,
            });

            info!(
                "Toggled zoom for pane {:?} (zoomed: {})",
                current_pane, is_zoomed
            );
            Ok(true)
        } else {
            info!("Zoom pane failed");
            Ok(false)
        }
    }

    /// Handle recent pane cycling
    fn handle_cycle_recent_panes(&mut self) -> WarpResult<bool> {
        // Snapshot needed fields to avoid borrowing conflicts
        let (tab_id, mut current_pane) = {
            let Some(active_tab) = self.tab_manager.active_tab() else {
                info!("No active tab for cycle recent panes");
                return Ok(false);
            };
            (active_tab.id, active_tab.active_pane)
        };

        let cycle_success = self.split_manager.cycle_recent_panes(&mut current_pane);

        if cycle_success {
            // Update active pane in tab
            self.tab_manager.set_active_pane(tab_id, current_pane);

            // Send UI update event
            self.send_ui_update_event(WarpUiUpdateType::PaneFocused {
                tab_id,
                pane_id: current_pane,
            });

            info!("Cycled to recent pane {:?}", current_pane);
            Ok(true)
        } else {
            info!("Cycle recent panes failed - not enough recent panes");
            Ok(false)
        }
    }

    /// Handle split equalization
    fn handle_equalize_splits(&mut self) -> WarpResult<bool> {
        // Snapshot needed fields to avoid borrowing conflicts
        let (tab_id, mut layout) = {
            let Some(active_tab) = self.tab_manager.active_tab() else {
                info!("No active tab for equalize splits");
                return Ok(false);
            };
            (active_tab.id, active_tab.split_layout.clone())
        };

        self.split_manager.equalize_splits(&mut layout);

        // Update the tab with new layout
        self.tab_manager.update_tab_split_layout(tab_id, layout);

        // Send UI update event
        self.send_ui_update_event(WarpUiUpdateType::SplitsEqualized { tab_id });

        info!("Equalized splits for tab {:?}", tab_id);
        Ok(true)
    }

    /// Handle pane closing
    fn handle_close_pane(&mut self) -> WarpResult<bool> {
        // Snapshot needed fields to avoid borrowing conflicts
        let (tab_id, current_pane, mut layout) = {
            let Some(active_tab) = self.tab_manager.active_tab() else {
                info!("No active tab for close pane");
                return Ok(false);
            };
            (
                active_tab.id,
                active_tab.active_pane,
                active_tab.split_layout.clone(),
            )
        };

        let mut active_pane_id = current_pane;

        let close_success =
            self.split_manager
                .close_pane_smart(&mut layout, current_pane, &mut active_pane_id);

        if close_success {
            // Clean up terminal resources for the closed pane
            self.cleanup_pane(current_pane);

            // Update the tab with new layout and active pane
            self.tab_manager.update_tab_split_layout(tab_id, layout);
            if active_pane_id != current_pane {
                self.tab_manager.set_active_pane(tab_id, active_pane_id);
            }

            // Check if this was the last pane in the tab
            let remaining_panes = self
                .tab_manager
                .active_tab()
                .map(|t| t.split_layout.collect_pane_ids().len())
                .unwrap_or(0);

            if remaining_panes == 0 {
                // Close the entire tab if no panes remain
                return self.handle_close_tab();
            }

            // Send UI update event
            self.send_ui_update_event(WarpUiUpdateType::PaneClosed {
                tab_id,
                closed_pane_id: current_pane,
                new_active_pane_id: active_pane_id,
            });

            info!(
                "Closed pane {:?}, new active: {:?}",
                current_pane, active_pane_id
            );
            Ok(true)
        } else {
            info!("Close pane failed");
            Ok(false)
        }
    }

    /// Handle session saving
    fn handle_save_session(&mut self) -> WarpResult<bool> {
        let start = Instant::now();

        match self.tab_manager.save_session() {
            Ok(()) => {
                self.perf_stats.session_save_time_ms = start.elapsed().as_millis() as u64;
                info!("Warp session saved successfully");
                Ok(true)
            }
            Err(e) => {
                error!("Failed to save Warp session: {}", e);
                Err(WarpIntegrationError::SessionFile(e))
            }
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
            }
            Ok(false) => {
                self.create_default_tab()?;
                Ok(false)
            }
            Err(e) => Err(WarpIntegrationError::SessionFile(e)),
        }
    }

    /// Handle other standard actions
    fn handle_next_tab(&mut self) -> WarpResult<bool> {
        let ok = self.tab_manager.next_tab();
        let msg = if ok {
            "Switched to next tab"
        } else {
            "Switch to next tab failed"
        };
        info!("{}", msg);

        if ok {
            // Send UI update event
            if let Some(active_tab) = self.tab_manager.active_tab() {
                self.send_ui_update_event(WarpUiUpdateType::TabSwitched {
                    tab_id: active_tab.id,
                });
            }
        }

        Ok(ok)
    }

    fn handle_previous_tab(&mut self) -> WarpResult<bool> {
        let ok = self.tab_manager.previous_tab();
        let msg = if ok {
            "Switched to previous tab"
        } else {
            "Switch to previous tab failed"
        };
        info!("{}", msg);

        if ok {
            // Send UI update event
            if let Some(active_tab) = self.tab_manager.active_tab() {
                self.send_ui_update_event(WarpUiUpdateType::TabSwitched {
                    tab_id: active_tab.id,
                });
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

        // Clean up PTY manager - we need to find the PTY ID that corresponds to this pane
        // For now, we'll do periodic cleanup of inactive PTY managers
        self.pty_managers.cleanup_inactive();
        debug!("Cleaned up inactive PTY managers");

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

    /// Get shell integration path if available
    fn get_shell_integration_path(&self) -> Option<String> {
        // Look for shell integration scripts in common locations
        let integration_paths = [
            "shell-integration/bash/openagent_integration.bash",
            "shell-integration/zsh/openagent_integration.zsh",
            "shell-integration/fish/openagent_integration.fish",
        ];

        for path in &integration_paths {
            let full_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
            if full_path.exists() {
                return Some(full_path.to_string_lossy().to_string());
            }
        }

        None
    }

    /// Send UI update event through the event proxy
    fn send_ui_update_event(&self, update_type: WarpUiUpdateType) {
        if let Some(proxy) = &self.event_proxy {
            if let Some(wid) = self.window_id {
                let event = Event::new(EventType::WarpUiUpdate(update_type), wid);
                let _ = proxy.send_event(event);
            }
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
            self.tab_manager
                .update_tab_for_command(active_tab.id, command);
            self.update_activity();
        }
    }

    /// Get current context for AI integration
    pub fn get_current_ai_context(&self) -> Option<PtyAiContext> {
        let active_tab = self.tab_manager.active_tab()?;

        // For now, we'll map pane_id to pty_id by searching through our PTY managers
        // In a real implementation, we'd maintain a pane_id -> pty_id mapping
        for pty_id in self.pty_managers.active_pty_ids() {
            if let Some(context) = self.pty_managers.get_ai_context(pty_id) {
                // For now, return the first active context
                // TODO: Implement proper pane_id -> pty_id mapping
                return Some(context);
            }
        }

        // Fallback to basic context from active tab
        Some(PtyAiContext {
            working_directory: active_tab.working_directory.clone(),
            shell_kind: openagent_terminal_core::tty::pty_manager::ShellKind::Unknown,
            last_command: None,
            shell_executable: "bash".to_string(),
        })
    }

    /// Update command context for the active pane
    pub fn update_command_context(&mut self, command: &str) {
        // Update tab for smart naming
        if let Some(active_tab) = self.tab_manager.active_tab() {
            self.tab_manager
                .update_tab_for_command(active_tab.id, command);
        }

        // Update PTY manager context
        for pty_id in self.pty_managers.active_pty_ids() {
            if let Some(manager) = self.pty_managers.get_manager(pty_id) {
                manager.lock().update_last_command(command.to_string());
                break; // For now, update first active PTY
            }
        }

        self.update_activity();
    }

    /// Get performance statistics
    pub fn performance_stats(&self) -> &WarpPerformanceStats {
        &self.perf_stats
    }

    /// Check if auto-save is needed
    pub fn should_auto_save(&self) -> bool {
        self.last_activity.elapsed() > Duration::from_secs(30)
    }

    /// Generate unique pane ID
    fn generate_pane_id(&self) -> PaneId {
        use std::time::{SystemTime, UNIX_EPOCH};
        PaneId(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as usize,
        )
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
    TabSwitched {
        tab_id: TabId,
    },
    PaneSplit {
        tab_id: TabId,
        new_pane_id: PaneId,
    },
    PaneFocused {
        tab_id: TabId,
        pane_id: PaneId,
    },
    PaneResized {
        tab_id: TabId,
        pane_id: PaneId,
    },
    PaneZoomed {
        tab_id: TabId,
        pane_id: PaneId,
        zoomed: bool,
    },
    PaneClosed {
        tab_id: TabId,
        closed_pane_id: PaneId,
        new_active_pane_id: PaneId,
    },
    SplitsEqualized {
        tab_id: TabId,
    },
    /// Internal UI update to trigger session autosave
    SessionAutoSave,
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
            }
            Action::ResizePaneUp => Some(WarpAction::ResizePane(WarpResizeDirection::ExpandUp)),
            Action::ResizePaneDown => Some(WarpAction::ResizePane(WarpResizeDirection::ExpandDown)),
            Action::ClosePane => Some(WarpAction::ClosePane),
            _ => None,
        }
    }
}
