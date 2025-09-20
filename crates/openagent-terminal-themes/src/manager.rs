use crate::config::{Theme, ThemeMetadata, ThemeModifications, ThemePackage};
use crate::loader::ThemeLoader;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug)]
pub struct ThemeManager {
    themes: HashMap<String, Theme>,
    active_theme: Option<String>,
    user_themes_dir: std::path::PathBuf,
}

impl ThemeManager {
    pub fn new() -> Result<Self> {
        let user_themes_dir = dirs::config_dir()
            .ok_or_else(|| anyhow!("Could not find config directory"))?
            .join("openagent-terminal")
            .join("themes");

        std::fs::create_dir_all(&user_themes_dir)?;

        let mut manager = Self { themes: HashMap::new(), active_theme: None, user_themes_dir };

        manager.load_built_in_themes()?;
        manager.load_user_themes()?;

        Ok(manager)
    }

    pub fn load_theme(&mut self, name: &str) -> Result<Theme> {
        if let Some(theme) = self.themes.get(name) {
            return Ok(theme.clone());
        }

        // Try loading from built-in themes
        match ThemeLoader::load_built_in(name) {
            Ok(theme) => {
                self.themes.insert(name.to_string(), theme.clone());
                Ok(theme)
            }
            Err(_) => {
                // Try loading from user themes directory
                let theme_path = self.user_themes_dir.join(format!("{}.toml", name));
                if theme_path.exists() {
                    let theme = ThemeLoader::load_from_file(&theme_path)?;
                    self.themes.insert(name.to_string(), theme.clone());
                    Ok(theme)
                } else {
                    Err(anyhow!("Theme '{}' not found", name))
                }
            }
        }
    }

    pub fn list_themes(&self) -> Result<Vec<ThemeMetadata>> {
        let mut themes: Vec<ThemeMetadata> =
            self.themes.values().map(|theme| theme.metadata.clone()).collect();

        themes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(themes)
    }

    pub fn get_active_theme(&self) -> Option<&Theme> {
        if let Some(active_name) = &self.active_theme {
            self.themes.get(active_name)
        } else {
            None
        }
    }

    pub fn set_active_theme(&mut self, theme_name: &str) -> Result<()> {
        if !self.themes.contains_key(theme_name) {
            self.load_theme(theme_name)?;
        }
        self.active_theme = Some(theme_name.to_string());
        Ok(())
    }

    pub fn install_theme(&mut self, package: ThemePackage) -> Result<()> {
        let theme_name = package.theme.metadata.name.clone();
        let theme_path = self.user_themes_dir.join(format!("{}.toml", theme_name));

        let theme_content = toml::to_string_pretty(&package.theme)?;
        std::fs::write(theme_path, theme_content)?;

        self.themes.insert(theme_name, package.theme);
        Ok(())
    }

    pub fn install_theme_from_file(&mut self, theme: Theme, _source_path: &Path) -> Result<()> {
        let theme_name = theme.metadata.name.clone();
        let theme_path = self.user_themes_dir.join(format!("{}.toml", theme_name));

        let theme_content = toml::to_string_pretty(&theme)?;
        std::fs::write(theme_path, theme_content)?;

        self.themes.insert(theme_name, theme);
        Ok(())
    }

    pub fn create_custom_theme(
        &mut self,
        base_theme: &str,
        name: &str,
        modifications: &ThemeModifications,
    ) -> Result<Theme> {
        let mut base = self.load_theme(base_theme)?;

        // Apply modifications
        if let Some(token_overrides) = &modifications.token_overrides {
            // Apply token overrides (simplified implementation)
            for (key, value) in token_overrides {
                match key.as_str() {
                    "accent" => base.tokens.accent = value.clone(),
                    "surface" => base.tokens.surface = value.clone(),
                    "text" => base.tokens.text = value.clone(),
                    _ => {} // Handle other tokens
                }
            }
        }

        // Update metadata
        base.metadata.name = name.to_string();
        base.metadata.display_name = format!("Custom {}", name);
        base.metadata.version = "1.0.0-custom".to_string();

        // Save the custom theme
        let theme_path = self.user_themes_dir.join(format!("{}.toml", name));
        let theme_content = toml::to_string_pretty(&base)?;
        std::fs::write(theme_path, theme_content)?;

        self.themes.insert(name.to_string(), base.clone());
        Ok(base)
    }

    pub fn enable_hot_reload(&mut self) -> Result<()> {
        // Placeholder for hot-reload functionality
        // In a real implementation, this would set up file watching
        Ok(())
    }

    fn load_built_in_themes(&mut self) -> Result<()> {
        let built_in_themes = ["dark", "light", "high-contrast-dark"];

        for theme_name in &built_in_themes {
            if let Ok(theme) = ThemeLoader::load_built_in(theme_name) {
                self.themes.insert(theme_name.to_string(), theme);
            }
        }

        // Set default active theme
        if self.themes.contains_key("dark") {
            self.active_theme = Some("dark".to_string());
        }

        Ok(())
    }

    fn load_user_themes(&mut self) -> Result<()> {
        if !self.user_themes_dir.exists() {
            return Ok(());
        }

        for entry in walkdir::WalkDir::new(&self.user_themes_dir)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if let Some(extension) = entry.path().extension() {
                if extension == "toml" {
                    if let Ok(theme) = ThemeLoader::load_from_file(entry.path()) {
                        self.themes.insert(theme.metadata.name.clone(), theme);
                    }
                }
            }
        }

        Ok(())
    }
}
