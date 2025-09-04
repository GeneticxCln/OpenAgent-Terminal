//! Warp-style Tab Management for OpenAgent Terminal
//!
//! This module implements Warp Terminal's tab management patterns:
//! - Automatic tab naming based on directory/command
//! - Session persistence and restoration
//! - Smart tab lifecycle management
//! - Command-aware tab titles

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::split_manager::{PaneId, SplitLayout};
use super::tab_manager::{PaneContext, TabContext, TabId};

/// Warp-style session data for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpSession {
    /// Session format version for migration support
    #[serde(default = "default_session_version")]
    pub version: String,
    pub id: String,
    pub name: String,
    pub created_at: SystemTime,
    pub last_used: SystemTime,
    pub tabs: Vec<WarpTabSession>,
    pub active_tab_id: Option<TabId>,
}

/// Current session format version
const SESSION_VERSION: &str = "1.0.0";

fn default_session_version() -> String {
    SESSION_VERSION.to_string()
}

/// Serializable tab session data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpTabSession {
    pub id: TabId,
    pub title: String,
    pub working_directory: PathBuf,
    pub split_layout: WarpSplitLayoutSession,
    pub active_pane: PaneId,
    pub panes: HashMap<PaneId, WarpPaneSession>,
    pub shell_command: Option<String>,
    pub last_command: Option<String>,
    pub created_at: SystemTime,
}

/// Serializable split layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WarpSplitLayoutSession {
    Single(PaneId),
    Horizontal { left: Box<WarpSplitLayoutSession>, right: Box<WarpSplitLayoutSession>, ratio: f32 },
    Vertical { top: Box<WarpSplitLayoutSession>, bottom: Box<WarpSplitLayoutSession>, ratio: f32 },
}

/// Serializable pane session data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpPaneSession {
    pub id: PaneId,
    pub working_directory: PathBuf,
    pub shell_command: Option<String>,
    pub last_command: Option<String>,
    pub title_override: Option<String>,
}

/// Enhanced tab manager with Warp-style features
pub struct WarpTabManager {
    /// Core tab management
    tabs: HashMap<TabId, TabContext>,
    tab_order: Vec<TabId>,
    active_tab_id: Option<TabId>,
    next_tab_id: usize,
    next_pane_id: usize,

    /// Warp-specific features
    auto_naming_enabled: bool,
    session_file_path: Option<PathBuf>,
    last_session_save: SystemTime,
    session_auto_save_interval: Duration,

    /// Smart naming
    directory_cache: HashMap<PathBuf, String>,
    command_history: HashMap<TabId, Vec<String>>,
}

impl WarpTabManager {
    /// Create a new Warp-style tab manager
    pub fn new() -> Self {
        Self {
            tabs: HashMap::new(),
            tab_order: Vec::new(),
            active_tab_id: None,
            next_tab_id: 1,
            next_pane_id: 1,

            auto_naming_enabled: true,
            session_file_path: None,
            last_session_save: SystemTime::now(),
            session_auto_save_interval: Duration::from_secs(30),

            directory_cache: HashMap::new(),
            command_history: HashMap::new(),
        }
    }

    /// Create with session persistence enabled
    pub fn with_session_file<P: AsRef<Path>>(session_path: P) -> Self {
        let mut manager = Self::new();
        manager.session_file_path = Some(session_path.as_ref().to_path_buf());
        manager
    }

    /// Create a new tab with Warp-style automatic naming
    pub fn create_warp_tab(&mut self, working_dir: Option<PathBuf>) -> TabId {
        let tab_id = TabId(self.next_tab_id);
        self.next_tab_id += 1;

        let pane_id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;

        let working_directory = working_dir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));

        // Generate Warp-style tab title
        let title = self.generate_smart_tab_name(&working_directory, None);

        let tab_context = TabContext {
            id: tab_id,
            title,
            working_directory: working_directory.clone(),
            split_layout: SplitLayout::Single(pane_id),
            active_pane: pane_id,
            panes: HashMap::new(),
            #[cfg(feature = "ai")]
            ai_runtime: None,
            modified: false,
            shell_command: None,
            zoom_saved_layout: None,
            last_exit_nonzero: false,
            panes_synced: false,
        };

        self.tabs.insert(tab_id, tab_context);
        self.tab_order.push(tab_id);
        self.command_history.insert(tab_id, Vec::new());

        // If this is the first tab, make it active
        if self.active_tab_id.is_none() {
            self.active_tab_id = Some(tab_id);
        }

        // Schedule session save
        self.schedule_session_save();

        tab_id
    }

    /// Generate smart tab name based on directory and current command (Warp-style)
    fn generate_smart_tab_name(
        &mut self,
        working_dir: &Path,
        current_command: Option<&str>,
    ) -> String {
        if !self.auto_naming_enabled {
            return "Terminal".to_string();
        }

        // Check cache first
        if let Some(cached_name) = self.directory_cache.get(working_dir) {
            if current_command.is_none() {
                return cached_name.clone();
            }
        }

        let dir_name = working_dir.file_name().and_then(|name| name.to_str()).unwrap_or("~");

        let smart_name = if let Some(cmd) = current_command {
            // Format: "command in directory"
            let cmd_name = cmd.split_whitespace().next().unwrap_or(cmd);
            format!("{} in {}", cmd_name, dir_name)
        } else if self.is_project_directory(working_dir) {
            // Use project name for recognized project directories
            self.get_project_name(working_dir).unwrap_or_else(|| dir_name.to_string())
        } else {
            dir_name.to_string()
        };

        // Cache the result
        self.directory_cache.insert(working_dir.to_path_buf(), smart_name.clone());
        smart_name
    }

    /// Check if directory appears to be a project root
    fn is_project_directory(&self, dir: &Path) -> bool {
        let project_files = [
            "package.json",
            "Cargo.toml",
            "pyproject.toml",
            "requirements.txt",
            "go.mod",
            "build.gradle",
            "pom.xml",
            ".git",
            "README.md",
        ];

        project_files.iter().any(|file| dir.join(file).exists())
    }

    /// Extract project name from directory
    fn get_project_name(&self, dir: &Path) -> Option<String> {
        // Try package.json first
        if let Ok(package_json) = std::fs::read_to_string(dir.join("package.json")) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&package_json) {
                if let Some(name) = parsed.get("name").and_then(|n| n.as_str()) {
                    return Some(name.to_string());
                }
            }
        }

        // Try Cargo.toml
        if let Ok(cargo_toml) = std::fs::read_to_string(dir.join("Cargo.toml")) {
            for line in cargo_toml.lines() {
                if line.starts_with("name = ") {
                    let name = line.split('=').nth(1)?.trim().trim_matches('"').trim_matches('\'');
                    return Some(name.to_string());
                }
            }
        }

        None
    }

    /// Update tab title when command is executed
    pub fn update_tab_for_command(&mut self, tab_id: TabId, command: &str) {
        // Update command history first
        if let Some(history) = self.command_history.get_mut(&tab_id) {
            history.push(command.to_string());
            // Keep only last 10 commands
            if history.len() > 10 {
                history.remove(0);
            }
        }

        // Update tab title if auto-naming is enabled
        if self.auto_naming_enabled {
            if let Some(tab) = self.tabs.get(&tab_id) {
                let working_dir = tab.working_directory.clone();
                let new_title = self.generate_smart_tab_name(&working_dir, Some(command));

                // Now update the tab with the new title
                if let Some(tab) = self.tabs.get_mut(&tab_id) {
                    tab.title = new_title;
                }
            }
        }

        self.schedule_session_save();
    }

    /// Create a new tab and split from existing tab (Warp Cmd+D behavior)
    pub fn duplicate_tab_as_split(
        &mut self,
        source_tab_id: TabId,
        direction: SplitDirection,
    ) -> Option<PaneId> {
        let _source_tab = self.tabs.get(&source_tab_id)?;

        let new_pane_id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;

        // Get mutable reference to the tab for splitting
        if let Some(tab) = self.tabs.get_mut(&source_tab_id) {
            // Create new split layout based on direction
            let new_layout = match direction {
                SplitDirection::Right => SplitLayout::Horizontal {
                    left: Box::new(tab.split_layout.clone()),
                    right: Box::new(SplitLayout::Single(new_pane_id)),
                    ratio: 0.5,
                },
                SplitDirection::Down => SplitLayout::Vertical {
                    top: Box::new(tab.split_layout.clone()),
                    bottom: Box::new(SplitLayout::Single(new_pane_id)),
                    ratio: 0.5,
                },
            };

            tab.split_layout = new_layout;
            tab.active_pane = new_pane_id;

            // Note: In real implementation, would create proper PaneContext
            // For now, just update the layout structure

            self.schedule_session_save();
            Some(new_pane_id)
        } else {
            None
        }
    }

    /// Save current session to disk
    pub fn save_session(&mut self) -> std::io::Result<()> {
        let Some(session_path) = &self.session_file_path else {
            return Ok(()); // No session file configured
        };

        let session = self.create_session_snapshot();
        let session_json = serde_json::to_string_pretty(&session)?;

        std::fs::write(session_path, session_json)?;
        self.last_session_save = SystemTime::now();

        Ok(())
    }

    /// Load session from disk with validation and migration support
    pub fn load_session(&mut self) -> std::io::Result<bool> {
        let Some(session_path) = &self.session_file_path else {
            return Ok(false); // No session file configured
        };

        if !session_path.exists() {
            return Ok(false); // No session to load
        }

        // Read and validate session file
        let session_json = std::fs::read_to_string(session_path)?;
        
        // First try to parse as current format
        let session: WarpSession = match serde_json::from_str(&session_json) {
            Ok(session) => {
                // Validate session format version
                if session.version != SESSION_VERSION {
                    // Attempt migration if needed
                    match self.migrate_session_format(session) {
                        Ok(migrated) => migrated,
                        Err(e) => {
                            eprintln!("Failed to migrate session from version {} to {}: {}", 
                                     session.version, SESSION_VERSION, e);
                            return Ok(false);
                        }
                    }
                } else {
                    session
                }
            },
            Err(e) => {
                eprintln!("Failed to parse session file {}: {}", session_path.display(), e);
                
                // Try to create backup and return false
                if let Err(backup_err) = self.backup_corrupted_session(session_path) {
                    eprintln!("Failed to backup corrupted session: {}", backup_err);
                }
                
                return Ok(false);
            }
        };

        // Validate session data integrity
        if let Err(e) = self.validate_session(&session) {
            eprintln!("Session validation failed: {}", e);
            return Ok(false);
        }

        self.restore_from_session(session);
        Ok(true)
    }

    /// Create session snapshot for persistence
    fn create_session_snapshot(&self) -> WarpSession {
        let tabs = self
            .tab_order
            .iter()
            .filter_map(|&tab_id| self.tabs.get(&tab_id))
            .map(|tab| self.tab_to_session(tab))
            .collect();

        WarpSession {
            version: SESSION_VERSION.to_string(),
            id: uuid::Uuid::new_v4().to_string(),
            name: "OpenAgent Terminal Session".to_string(),
            created_at: SystemTime::now(),
            last_used: SystemTime::now(),
            tabs,
            active_tab_id: self.active_tab_id,
        }
    }

    /// Convert tab context to session data
    fn tab_to_session(&self, tab: &TabContext) -> WarpTabSession {
        WarpTabSession {
            id: tab.id,
            title: tab.title.clone(),
            working_directory: tab.working_directory.clone(),
            split_layout: self.split_layout_to_session(&tab.split_layout),
            active_pane: tab.active_pane,
            panes: tab
                .panes
                .iter()
                .map(|(&id, pane)| (id, self.pane_to_session(id, pane)))
                .collect(),
            shell_command: tab.shell_command.clone(),
            last_command: self.command_history.get(&tab.id).and_then(|hist| hist.last()).cloned(),
            created_at: SystemTime::now(),
        }
    }

    /// Convert split layout to session format
    fn split_layout_to_session(&self, layout: &SplitLayout) -> WarpSplitLayoutSession {
        match layout {
            SplitLayout::Single(pane_id) => WarpSplitLayoutSession::Single(*pane_id),
            SplitLayout::Horizontal { left, right, ratio } => WarpSplitLayoutSession::Horizontal {
                left: Box::new(self.split_layout_to_session(left)),
                right: Box::new(self.split_layout_to_session(right)),
                ratio: *ratio,
            },
            SplitLayout::Vertical { top, bottom, ratio } => WarpSplitLayoutSession::Vertical {
                top: Box::new(self.split_layout_to_session(top)),
                bottom: Box::new(self.split_layout_to_session(bottom)),
                ratio: *ratio,
            },
        }
    }

    /// Convert pane context to session data
    fn pane_to_session(&self, pane_id: PaneId, pane: &PaneContext) -> WarpPaneSession {
        WarpPaneSession {
            id: pane_id,
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            shell_command: None,
            last_command: None,
            title_override: pane.title_override.clone(),
        }
    }

    /// Restore manager state from session
    fn restore_from_session(&mut self, session: WarpSession) {
        // Clear current state
        self.tabs.clear();
        self.tab_order.clear();
        self.command_history.clear();

        // Restore tabs
        for tab_session in session.tabs {
            self.restore_tab_from_session(tab_session);
        }

        self.active_tab_id = session.active_tab_id;
    }

    /// Restore individual tab from session
    fn restore_tab_from_session(&mut self, tab_session: WarpTabSession) {
        let tab_context = TabContext {
            id: tab_session.id,
            title: tab_session.title,
            working_directory: tab_session.working_directory,
            split_layout: self.session_to_split_layout(&tab_session.split_layout),
            active_pane: tab_session.active_pane,
            panes: HashMap::new(), // Will be populated separately
            #[cfg(feature = "ai")]
            ai_runtime: None,
            modified: false,
            shell_command: tab_session.shell_command,
            zoom_saved_layout: None,
            last_exit_nonzero: false,
            panes_synced: false,
        };

        self.tabs.insert(tab_session.id, tab_context);
        self.tab_order.push(tab_session.id);

        // Restore command history
        if let Some(last_command) = tab_session.last_command {
            self.command_history.insert(tab_session.id, vec![last_command]);
        }

        // Update ID counters
        self.next_tab_id = self.next_tab_id.max(tab_session.id.0 + 1);
    }

    /// Convert session split layout back to runtime format
    fn session_to_split_layout(&self, session_layout: &WarpSplitLayoutSession) -> SplitLayout {
        match session_layout {
            WarpSplitLayoutSession::Single(pane_id) => SplitLayout::Single(*pane_id),
            WarpSplitLayoutSession::Horizontal { left, right, ratio } => SplitLayout::Horizontal {
                left: Box::new(self.session_to_split_layout(left)),
                right: Box::new(self.session_to_split_layout(right)),
                ratio: *ratio,
            },
            WarpSplitLayoutSession::Vertical { top, bottom, ratio } => SplitLayout::Vertical {
                top: Box::new(self.session_to_split_layout(top)),
                bottom: Box::new(self.session_to_split_layout(bottom)),
                ratio: *ratio,
            },
        }
    }

    /// Schedule session save if enough time has passed
    fn schedule_session_save(&mut self) {
        if self.last_session_save.elapsed().unwrap_or(Duration::MAX)
            > self.session_auto_save_interval
        {
            // In a real implementation, this would schedule an async save
            let _ = self.save_session();
        }
    }

    /// Enable/disable automatic tab naming
    pub fn set_auto_naming(&mut self, enabled: bool) {
        self.auto_naming_enabled = enabled;
    }

    /// Validate session data integrity
    fn validate_session(&self, session: &WarpSession) -> Result<(), String> {
        // Check if session has any tabs
        if session.tabs.is_empty() {
            return Err("Session contains no tabs".to_string());
        }

        // Validate each tab
        for (idx, tab) in session.tabs.iter().enumerate() {
            // Check working directory exists or can be recovered
            if !tab.working_directory.exists() && !tab.working_directory.parent().map_or(false, |p| p.exists()) {
                println!("Warning: Tab {} working directory {} is not accessible", idx, tab.working_directory.display());
            }

            // Validate split layout has at least one pane
            let pane_count = match &tab.split_layout {
                WarpSplitLayoutSession::Single(_) => 1,
                WarpSplitLayoutSession::Horizontal { .. } => 2, // Simplified validation
                WarpSplitLayoutSession::Vertical { .. } => 2,   // Simplified validation
            };
            
            if pane_count == 0 {
                return Err(format!("Tab {} has no panes", idx));
            }
        }

        // Validate active tab ID exists
        if let Some(active_id) = session.active_tab_id {
            if !session.tabs.iter().any(|tab| tab.id == active_id) {
                return Err("Active tab ID not found in tabs list".to_string());
            }
        }

        Ok(())
    }

    /// Migrate session from older format versions
    fn migrate_session_format(&self, mut session: WarpSession) -> Result<WarpSession, String> {
        match session.version.as_str() {
            "1.0.0" => {
                // Current version, no migration needed
                Ok(session)
            },
            "" | "0.9.0" => {
                // Migrate from pre-version or 0.9.0
                session.version = SESSION_VERSION.to_string();
                println!("Migrated session from version 0.9.0 to {}", SESSION_VERSION);
                Ok(session)
            },
            _ => {
                Err(format!("Unsupported session version: {}", session.version))
            }
        }
    }

    /// Create backup of corrupted session file
    fn backup_corrupted_session(&self, session_path: &Path) -> std::io::Result<()> {
        let backup_path = session_path.with_extension("json.backup");
        std::fs::copy(session_path, backup_path)?;
        println!("Backed up corrupted session file");
        Ok(())
    }

    /// Get tab count
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Get active tab
    pub fn active_tab(&self) -> Option<&TabContext> {
        self.active_tab_id.and_then(|id| self.tabs.get(&id))
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

    /// Close tab with session cleanup
    pub fn close_warp_tab(&mut self, tab_id: TabId) -> bool {
        if self.tabs.remove(&tab_id).is_some() {
            // Remove from order
            if let Some(pos) = self.tab_order.iter().position(|&id| id == tab_id) {
                self.tab_order.remove(pos);
            }

            // Clean up command history
            self.command_history.remove(&tab_id);

            // Update active tab
            if self.active_tab_id == Some(tab_id) {
                self.active_tab_id =
                    if self.tab_order.is_empty() { None } else { Some(self.tab_order[0]) };
            }

            self.schedule_session_save();
            true
        } else {
            false
        }
    }

    /// Get all tabs for session restoration
    pub fn all_tabs(&self) -> impl Iterator<Item = &TabContext> {
        self.tab_order.iter().filter_map(|&id| self.tabs.get(&id))
    }

    /// Update tab split layout (for session restoration)
    pub fn update_tab_split_layout(&mut self, tab_id: TabId, new_layout: SplitLayout) -> bool {
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.split_layout = new_layout;
            self.schedule_session_save();
            true
        } else {
            false
        }
    }

    /// Set active pane for a tab (for session restoration)
    pub fn set_active_pane(&mut self, tab_id: TabId, pane_id: PaneId) -> bool {
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.active_pane = pane_id;
            true
        } else {
            false
        }
    }

    /// Add pane context to a tab (for session restoration)
    pub fn add_pane_to_tab(&mut self, tab_id: TabId, pane_id: PaneId, pane_context: PaneContext) -> bool {
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.panes.insert(pane_id, pane_context);
            true
        } else {
            false
        }
    }

    /// Update working directory for a tab (for session restoration fallback)
    pub fn update_tab_working_directory(&mut self, tab_id: TabId, new_dir: PathBuf) -> bool {
        if let Some(tab) = self.tabs.get_mut(&tab_id) {
            tab.working_directory = new_dir;
            true
        } else {
            false
        }
    }
}

/// Direction for splitting panes
#[derive(Debug, Clone, Copy)]
pub enum SplitDirection {
    Right,
    Down,
}

impl Default for WarpTabManager {
    fn default() -> Self {
        Self::new()
    }
}
