use crate::config::Theme;
use anyhow::{anyhow, Result};
use std::path::Path;

pub struct ThemeLoader;

impl ThemeLoader {
    pub fn load_from_file(path: &Path) -> Result<Theme> {
        let content = std::fs::read_to_string(path)?;
        Self::load_from_string(&content)
    }
    
    pub fn load_from_string(content: &str) -> Result<Theme> {
        toml::from_str(content)
            .map_err(|e| anyhow!("Failed to parse theme: {}", e))
    }
    
    pub fn load_built_in(name: &str) -> Result<Theme> {
        match name {
            "dark" => Self::load_dark_theme(),
            "light" => Self::load_light_theme(),
            "high-contrast-dark" => Self::load_high_contrast_theme(),
            _ => Err(anyhow!("Unknown built-in theme: {}", name))
        }
    }
    
    fn load_dark_theme() -> Result<Theme> {
        // Load from embedded or default dark theme
        let content = include_str!("../../../extra/themes/dark.toml");
        Self::load_from_string(content)
    }
    
    fn load_light_theme() -> Result<Theme> {
        // Placeholder for light theme
        let mut theme = Self::load_dark_theme()?;
        theme.metadata.name = "light".to_string();
        theme.tokens.surface = "#ffffff".to_string();
        theme.tokens.text = "#000000".to_string();
        Ok(theme)
    }
    
    fn load_high_contrast_theme() -> Result<Theme> {
        // Placeholder for high contrast theme
        let mut theme = Self::load_dark_theme()?;
        theme.metadata.name = "high-contrast-dark".to_string();
        theme.tokens.surface = "#000000".to_string();
        theme.tokens.text = "#ffffff".to_string();
        Ok(theme)
    }
}
