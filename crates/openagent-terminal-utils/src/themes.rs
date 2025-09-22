//! Themes functionality for OpenAgent Terminal
//! 
//! This module provides theme management and loading capabilities.

use crate::{UtilsError, UtilsResult};
use std::path::Path;
use std::collections::HashMap;

/// Theme color definition
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThemeColors {
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub selection_foreground: Option<String>,
    pub selection_background: Option<String>,
    pub normal: Vec<String>,
    pub bright: Vec<String>,
}

/// Theme definition
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Theme {
    pub name: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub colors: ThemeColors,
}

/// Theme manager
#[derive(Debug, Default)]
pub struct ThemesManager {
    themes: HashMap<String, Theme>,
    current_theme: Option<String>,
}

impl ThemesManager {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn initialize(&mut self) -> UtilsResult<()> {
        tracing::info!("Initializing themes manager");
        self.load_builtin_themes()?;
        Ok(())
    }
    
    pub fn load_from_directory(&mut self, path: &Path) -> UtilsResult<()> {
        tracing::info!("Loading themes from directory: {:?}", path);
        // TODO: Scan directory for .toml theme files and load them
        Ok(())
    }
    
    pub fn get_theme(&self, name: &str) -> Option<&Theme> {
        self.themes.get(name)
    }
    
    pub fn list_themes(&self) -> Vec<&str> {
        self.themes.keys().map(|s| s.as_str()).collect()
    }
    
    pub fn set_current_theme(&mut self, name: String) -> UtilsResult<()> {
        if self.themes.contains_key(&name) {
            self.current_theme = Some(name);
            Ok(())
        } else {
            Err(UtilsError::Theme(format!("Theme '{}' not found", name)))
        }
    }
    
    pub fn get_current_theme(&self) -> Option<&Theme> {
        self.current_theme.as_ref().and_then(|name| self.themes.get(name))
    }
    
    fn load_builtin_themes(&mut self) -> UtilsResult<()> {
        // Add a default theme
        let default_theme = Theme {
            name: "default".to_string(),
            description: Some("Default OpenAgent Terminal theme".to_string()),
            author: Some("OpenAgent Terminal".to_string()),
            colors: ThemeColors {
                foreground: "#FFFFFF".to_string(),
                background: "#000000".to_string(),
                cursor: "#FFFFFF".to_string(),
                selection_foreground: Some("#000000".to_string()),
                selection_background: Some("#FFFFFF".to_string()),
                normal: vec![
                    "#000000".to_string(), // black
                    "#CD3131".to_string(), // red
                    "#0DBC79".to_string(), // green
                    "#E5E510".to_string(), // yellow
                    "#2472C8".to_string(), // blue
                    "#BC3FBC".to_string(), // magenta
                    "#11A8CD".to_string(), // cyan
                    "#E5E5E5".to_string(), // white
                ],
                bright: vec![
                    "#666666".to_string(), // bright black
                    "#F14C4C".to_string(), // bright red
                    "#23D18B".to_string(), // bright green
                    "#F5F543".to_string(), // bright yellow
                    "#3B8EEA".to_string(), // bright blue
                    "#D670D6".to_string(), // bright magenta
                    "#29B8DB".to_string(), // bright cyan
                    "#E5E5E5".to_string(), // bright white
                ],
            },
        };
        
        self.themes.insert("default".to_string(), default_theme);
        self.current_theme = Some("default".to_string());
        Ok(())
    }
}