// Blocks search state persistence
// Saves and restores search queries, filters, and preferences per session

#![cfg(feature = "blocks")]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::blocks_v2::{ExitCodeFilter, DurationFilter, SortField, SortOrder, ExecutionStatus, ShellType};
use crate::display::blocks_search_panel::{BlocksSearchState, FilterState, SearchMode};

/// Persistent search state data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentSearchState {
    /// Last search query
    pub last_query: String,
    /// Last search mode used
    pub last_mode: SearchMode,
    /// Last active filters
    pub last_filters: SerializableFilterState,
    /// Last sort configuration
    pub last_sort_field: SortField,
    pub last_sort_order: SortOrder,
    /// Search history (up to 50 entries)
    pub search_history: Vec<String>,
    /// Filter history for quick access
    pub filter_history: Vec<SerializableFilterState>,
    /// User preferences
    pub preferences: SearchPreferences,
    /// Session metadata
    pub session_id: String,
    pub last_updated: String,
}

/// Search preferences that persist across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPreferences {
    /// Default search mode when opening search panel
    pub default_mode: SearchMode,
    /// Number of items per page
    pub items_per_page: usize,
    /// Auto-save search state
    pub auto_save: bool,
    /// Remember filter state between sessions
    pub remember_filters: bool,
    /// Remember last search query between sessions
    pub remember_last_query: bool,
    /// Remember sort preferences between sessions
    pub remember_sort_preferences: bool,
    /// Search history size
    pub max_history_size: usize,
    /// Filter history size
    pub max_filter_history_size: usize,
    /// Default sort configuration
    pub default_sort_field: SortField,
    pub default_sort_order: SortOrder,
    /// Show search suggestions while typing
    pub show_search_suggestions: bool,
    /// Show filter history suggestions
    pub show_filter_suggestions: bool,
    /// Auto-complete from search history
    pub enable_search_autocomplete: bool,
}

/// Serializable version of FilterState (some types need conversion)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableFilterState {
    pub directory: Option<String>,
    pub shell: Option<String>,
    pub status: Option<String>,
    pub exit_code: Option<String>, // Serialized as string
    pub duration: Option<String>,  // Serialized as string
    pub starred_only: bool,
    pub tags: Vec<String>,
    pub date_from: Option<String>, // ISO 8601 string
    pub date_to: Option<String>,   // ISO 8601 string
}

impl Default for SearchPreferences {
    fn default() -> Self {
        Self {
            default_mode: SearchMode::Basic,
            items_per_page: 20,
            auto_save: true,
            remember_filters: true,
            remember_last_query: true,
            remember_sort_preferences: true,
            max_history_size: 50,
            max_filter_history_size: 20,
            default_sort_field: SortField::CreatedAt,
            default_sort_order: SortOrder::Descending,
            show_search_suggestions: true,
            show_filter_suggestions: true,
            enable_search_autocomplete: true,
        }
    }
}

impl Default for PersistentSearchState {
    fn default() -> Self {
        let session_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        
        Self {
            last_query: String::new(),
            last_mode: SearchMode::Basic,
            last_filters: SerializableFilterState::default(),
            last_sort_field: SortField::CreatedAt,
            last_sort_order: SortOrder::Descending,
            search_history: Vec::new(),
            filter_history: Vec::new(),
            preferences: SearchPreferences::default(),
            session_id,
            last_updated: now,
        }
    }
}

impl Default for SerializableFilterState {
    fn default() -> Self {
        Self {
            directory: None,
            shell: None,
            status: None,
            exit_code: None,
            duration: None,
            starred_only: false,
            tags: Vec::new(),
            date_from: None,
            date_to: None,
        }
    }
}

/// Manages persistence of blocks search state
pub struct SearchStatePersistence {
    state_file: PathBuf,
    current_state: PersistentSearchState,
    auto_save: bool,
}

impl SearchStatePersistence {
    /// Create a new search state persistence manager
    pub fn new(config_dir: &Path) -> Result<Self> {
        let state_file = config_dir.join("blocks_search_state.json");
        
        let current_state = if state_file.exists() {
            Self::load_from_file(&state_file)
                .unwrap_or_else(|_| PersistentSearchState::default())
        } else {
            PersistentSearchState::default()
        };
        
        Ok(Self {
            state_file,
            current_state,
            auto_save: current_state.preferences.auto_save,
        })
    }
    
    /// Save current search state to a BlocksSearchState
    pub fn apply_to_search_state(&self, search_state: &mut BlocksSearchState) {
        let prefs = &self.current_state.preferences;
        
        // Always apply basic preferences
        search_state.items_per_page = prefs.items_per_page;
        
        // Apply mode preference
        search_state.mode = if prefs.remember_filters {
            self.current_state.last_mode.clone()
        } else {
            prefs.default_mode.clone()
        };
        
        // Apply query preference
        if prefs.remember_last_query {
            search_state.query = self.current_state.last_query.clone();
        }
        
        // Apply filter preferences
        if prefs.remember_filters {
            search_state.filters = self.current_state.last_filters.to_filter_state();
        }
        
        // Apply sort preferences
        if prefs.remember_sort_preferences {
            search_state.sort_field = self.current_state.last_sort_field;
            search_state.sort_order = self.current_state.last_sort_order;
        } else {
            search_state.sort_field = prefs.default_sort_field;
            search_state.sort_order = prefs.default_sort_order;
        }
        
        // Apply search history if configured
        if prefs.show_search_suggestions {
            search_state.search_history = self.current_state.search_history.clone();
        }
    }
    
    /// Update persistent state from current BlocksSearchState
    pub fn update_from_search_state(&mut self, search_state: &BlocksSearchState) -> Result<()> {
        self.current_state.last_mode = search_state.mode.clone();
        self.current_state.last_sort_field = search_state.sort_field;
        self.current_state.last_sort_order = search_state.sort_order;
        
        // Update query if not empty
        if !search_state.query.is_empty() {
            self.current_state.last_query = search_state.query.clone();
            self.add_to_search_history(&search_state.query);
        }
        
        // Update filters if any are active
        if search_state.has_active_filters() {
            self.current_state.last_filters = SerializableFilterState::from_filter_state(&search_state.filters);
            self.add_to_filter_history(&self.current_state.last_filters);
        }
        
        self.current_state.last_updated = chrono::Utc::now().to_rfc3339();
        
        if self.auto_save {
            self.save()?;
        }
        
        Ok(())
    }
    
    /// Add a search query to history
    fn add_to_search_history(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        
        // Remove existing entry if present
        self.current_state.search_history.retain(|q| q != query);
        
        // Add to front
        self.current_state.search_history.insert(0, query.to_string());
        
        // Limit history size
        let max_size = self.current_state.preferences.max_history_size;
        if self.current_state.search_history.len() > max_size {
            self.current_state.search_history.truncate(max_size);
        }
    }
    
    /// Add filter state to history
    fn add_to_filter_history(&mut self, filters: &SerializableFilterState) {
        // Only add if filters are actually set
        if !filters.has_any_filter() {
            return;
        }
        
        // Remove existing identical entry
        self.current_state.filter_history.retain(|f| f != filters);
        
        // Add to front
        self.current_state.filter_history.insert(0, filters.clone());
        
        // Limit filter history size
        let max_filter_size = self.current_state.preferences.max_filter_history_size;
        if self.current_state.filter_history.len() > max_filter_size {
            self.current_state.filter_history.truncate(max_filter_size);
        }
    }
    
    /// Get search history for suggestions
    pub fn get_search_history(&self) -> &[String] {
        &self.current_state.search_history
    }
    
    /// Get filter history for quick access
    pub fn get_filter_history(&self) -> &[SerializableFilterState] {
        &self.current_state.filter_history
    }
    
    /// Get current preferences
    pub fn get_preferences(&self) -> &SearchPreferences {
        &self.current_state.preferences
    }
    
    /// Update preferences
    pub fn update_preferences(&mut self, preferences: SearchPreferences) -> Result<()> {
        self.current_state.preferences = preferences;
        self.auto_save = self.current_state.preferences.auto_save;
        
        if self.auto_save {
            self.save()?;
        }
        
        Ok(())
    }
    
    /// Save state to file
    pub fn save(&self) -> Result<()> {\n        let json = serde_json::to_string_pretty(&self.current_state)\n            .context(\"Failed to serialize search state\")?;\n        \n        // Ensure parent directory exists\n        if let Some(parent) = self.state_file.parent() {\n            fs::create_dir_all(parent)\n                .context(\"Failed to create config directory\")?;\n        }\n        \n        fs::write(&self.state_file, json)\n            .context(\"Failed to write search state file\")?;\n        \n        Ok(())\n    }\n    \n    /// Load state from file\n    fn load_from_file(file_path: &Path) -> Result<PersistentSearchState> {\n        let content = fs::read_to_string(file_path)\n            .context(\"Failed to read search state file\")?;\n        \n        let state: PersistentSearchState = serde_json::from_str(&content)\n            .context(\"Failed to parse search state file\")?;\n        \n        Ok(state)\n    }\n    \n    /// Clear all stored state\n    pub fn clear(&mut self) -> Result<()> {\n        self.current_state = PersistentSearchState::default();\n        \n        if self.state_file.exists() {\n            fs::remove_file(&self.state_file)\n                .context(\"Failed to remove search state file\")?;\n        }\n        \n        Ok(())\n    }\n}\n\nimpl SerializableFilterState {\n    /// Convert from runtime FilterState\n    pub fn from_filter_state(filter_state: &FilterState) -> Self {\n        Self {\n            directory: filter_state.directory.as_ref().map(|p| p.to_string_lossy().to_string()),\n            shell: filter_state.shell.as_ref().map(|s| s.to_str().to_string()),\n            status: filter_state.status.as_ref().map(|s| format!(\"{:?}\", s)),\n            exit_code: filter_state.exit_code.as_ref().map(|e| format!(\"{:?}\", e)),\n            duration: filter_state.duration.as_ref().map(|d| format!(\"{:?}\", d)),\n            starred_only: filter_state.starred_only,\n            tags: filter_state.tags.clone(),\n            date_from: filter_state.date_from.as_ref().map(|d| d.to_rfc3339()),\n            date_to: filter_state.date_to.as_ref().map(|d| d.to_rfc3339()),\n        }\n    }\n    \n    /// Convert to runtime FilterState\n    pub fn to_filter_state(&self) -> FilterState {\n        FilterState {\n            directory: self.directory.as_ref().map(PathBuf::from),\n            shell: self.shell.as_ref().and_then(|s| s.parse().ok()),\n            status: self.status.as_ref().and_then(|s| s.parse().ok()),\n            exit_code: self.exit_code.as_ref().and_then(|s| s.parse().ok()),\n            duration: self.duration.as_ref().and_then(|s| s.parse().ok()),\n            starred_only: self.starred_only,\n            tags: self.tags.clone(),\n            date_from: self.date_from.as_ref()\n                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())\n                .map(|dt| dt.with_timezone(&chrono::Utc)),\n            date_to: self.date_to.as_ref()\n                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())\n                .map(|dt| dt.with_timezone(&chrono::Utc)),\n        }\n    }\n    \n    /// Check if any filters are set\n    pub fn has_any_filter(&self) -> bool {\n        self.directory.is_some()\n            || self.shell.is_some()\n            || self.status.is_some()\n            || self.exit_code.is_some()\n            || self.duration.is_some()\n            || self.starred_only\n            || !self.tags.is_empty()\n            || self.date_from.is_some()\n            || self.date_to.is_some()\n    }\n}\n\n/// Extended blocks search state with persistence support\npub struct PersistentBlocksSearchState {\n    /// Current runtime state\n    pub state: BlocksSearchState,\n    /// Persistence manager\n    persistence: SearchStatePersistence,\n    /// Whether state has been modified since last save\n    dirty: bool,\n}\n\nimpl PersistentBlocksSearchState {\n    /// Create new persistent search state\n    pub fn new(config_dir: &Path) -> Result<Self> {\n        let persistence = SearchStatePersistence::new(config_dir)?;\n        let mut state = BlocksSearchState::new();\n        \n        // Apply persisted state\n        persistence.apply_to_search_state(&mut state);\n        \n        Ok(Self {\n            state,\n            persistence,\n            dirty: false,\n        })\n    }\n    \n    /// Open search with restored state\n    pub fn open(&mut self) -> Result<()> {\n        self.state.open();\n        \n        // Restore preferences\n        let prefs = self.persistence.get_preferences();\n        if !prefs.remember_filters {\n            // Clear filters if not remembering them\n            self.state.clear_all_filters();\n            self.state.query.clear();\n        }\n        \n        Ok(())\n    }\n    \n    /// Close search and save state\n    pub fn close(&mut self) -> Result<()> {\n        if self.dirty {\n            self.save_current_state()?;\n        }\n        self.state.close();\n        Ok(())\n    }\n    \n    /// Update query and mark as dirty\n    pub fn update_query(&mut self, query: String) {\n        if self.state.query != query {\n            self.state.query = query;\n            self.dirty = true;\n        }\n    }\n    \n    /// Update filters and mark as dirty\n    pub fn update_filters(&mut self, filters: FilterState) {\n        self.state.filters = filters;\n        self.dirty = true;\n    }\n    \n    /// Update sort configuration and mark as dirty\n    pub fn update_sort(&mut self, field: SortField, order: SortOrder) {\n        self.state.sort_field = field;\n        self.state.sort_order = order;\n        self.dirty = true;\n    }\n    \n    /// Get search suggestions based on history\n    pub fn get_search_suggestions(&self, partial_query: &str) -> Vec<String> {\n        if partial_query.is_empty() {\n            return self.persistence.get_search_history().iter().take(10).cloned().collect();\n        }\n        \n        self.persistence.get_search_history()\n            .iter()\n            .filter(|query| query.starts_with(partial_query))\n            .take(10)\n            .cloned()\n            .collect()\n    }\n    \n    /// Get filter suggestions based on history\n    pub fn get_filter_suggestions(&self) -> &[SerializableFilterState] {\n        &self.persistence.get_filter_history()[..self.persistence.get_filter_history().len().min(5)]\n    }\n    \n    /// Save current state to persistence\n    pub fn save_current_state(&mut self) -> Result<()> {\n        self.persistence.update_from_search_state(&self.state)?;\n        self.dirty = false;\n        Ok(())\n    }\n    \n    /// Get mutable reference to the search state\n    pub fn state_mut(&mut self) -> &mut BlocksSearchState {\n        self.dirty = true; // Mark as dirty when getting mutable access\n        &mut self.state\n    }\n    \n    /// Get immutable reference to the search state\n    pub fn state(&self) -> &BlocksSearchState {\n        &self.state\n    }\n    \n    /// Update preferences\n    pub fn update_preferences(&mut self, preferences: SearchPreferences) -> Result<()> {\n        self.persistence.update_preferences(preferences)\n    }\n    \n    /// Get preferences\n    pub fn get_preferences(&self) -> &SearchPreferences {\n        self.persistence.get_preferences()\n    }\n    \n    /// Clear all persistent state\n    pub fn clear_all(&mut self) -> Result<()> {\n        self.persistence.clear()?;\n        self.state = BlocksSearchState::new();\n        self.dirty = false;\n        Ok(())\n    }\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n    use tempfile::tempdir;\n    \n    #[test]\n    fn test_persistence_creation() {\n        let temp_dir = tempdir().unwrap();\n        let persistence = SearchStatePersistence::new(temp_dir.path()).unwrap();\n        assert_eq!(persistence.current_state.last_query, \"\");\n        assert_eq!(persistence.current_state.preferences.items_per_page, 20);\n    }\n    \n    #[test]\n    fn test_serializable_filter_conversion() {\n        let mut filter_state = FilterState::default();\n        filter_state.starred_only = true;\n        filter_state.tags = vec![\"important\".to_string(), \"bug\".to_string()];\n        \n        let serializable = SerializableFilterState::from_filter_state(&filter_state);\n        assert!(serializable.starred_only);\n        assert_eq!(serializable.tags.len(), 2);\n        \n        let converted_back = serializable.to_filter_state();\n        assert!(converted_back.starred_only);\n        assert_eq!(converted_back.tags.len(), 2);\n    }\n    \n    #[test]\n    fn test_search_history() {\n        let temp_dir = tempdir().unwrap();\n        let mut persistence = SearchStatePersistence::new(temp_dir.path()).unwrap();\n        \n        // Add some history entries\n        persistence.add_to_search_history(\"first query\");\n        persistence.add_to_search_history(\"second query\");\n        persistence.add_to_search_history(\"third query\");\n        \n        let history = persistence.get_search_history();\n        assert_eq!(history.len(), 3);\n        assert_eq!(history[0], \"third query\"); // Most recent first\n        assert_eq!(history[2], \"first query\"); // Oldest last\n    }\n    \n    #[test]\n    fn test_persistent_search_state() {\n        let temp_dir = tempdir().unwrap();\n        let mut persistent_state = PersistentBlocksSearchState::new(temp_dir.path()).unwrap();\n        \n        // Update some state\n        persistent_state.update_query(\"test query\".to_string());\n        persistent_state.state_mut().mode = SearchMode::Advanced;\n        \n        assert!(persistent_state.dirty);\n        assert_eq!(persistent_state.state().query, \"test query\");\n        assert_eq!(persistent_state.state().mode, SearchMode::Advanced);\n    }\n}"]}]
