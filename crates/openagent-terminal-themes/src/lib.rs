//! Theme management and marketplace system for OpenAgent Terminal
//!
//! This crate provides comprehensive theme management including:
//! - Theme loading and parsing
//! - Theme validation and compatibility checking
//! - Community theme marketplace integration
//! - Dynamic theme switching and hot-reloading
//! - Theme customization and exports

use std::path::Path;

pub mod config;
pub mod export;
pub mod loader;
pub mod manager;
pub mod marketplace;
pub mod validator;

// Re-exports for easy access
pub use config::{Theme, ThemeMetadata, ThemeTokens, UiConfig};
pub use loader::ThemeLoader;
pub use manager::ThemeManager;
pub use validator::ThemeValidator;

#[cfg(feature = "marketplace")]
pub use marketplace::{ThemeMarketplace, ThemeRegistry};

/// Main theme system interface
#[derive(Debug)]
pub struct ThemeSystem {
    manager: ThemeManager,
    #[cfg(feature = "marketplace")]
    marketplace: Option<ThemeMarketplace>,
}

impl ThemeSystem {
    /// Create a new theme system with default configuration
    pub fn new() -> anyhow::Result<Self> {
        let manager = ThemeManager::new()?;

        #[cfg(feature = "marketplace")]
        let marketplace = Some(ThemeMarketplace::new()?);

        Ok(Self {
            manager,
            #[cfg(feature = "marketplace")]
            marketplace,
        })
    }

    /// Load a theme by name
    pub fn load_theme(&mut self, name: &str) -> anyhow::Result<Theme> {
        self.manager.load_theme(name)
    }

    /// Get all available themes (built-in and user-installed)
    pub fn list_themes(&self) -> anyhow::Result<Vec<ThemeMetadata>> {
        self.manager.list_themes()
    }

    /// Validate a theme configuration
    pub fn validate_theme(&self, theme: &Theme) -> anyhow::Result<()> {
        ThemeValidator::validate(theme)
    }

    /// Install a theme from the marketplace
    #[cfg(feature = "marketplace")]
    pub async fn install_theme(&mut self, theme_id: &str) -> anyhow::Result<()> {
        if let Some(marketplace) = &mut self.marketplace {
            let theme_package = marketplace.download_theme(theme_id).await?;
            self.manager.install_theme(theme_package)?;
        }
        Ok(())
    }

    /// Search for themes in the marketplace
    #[cfg(feature = "marketplace")]
    pub async fn search_themes(
        &self,
        query: &str,
    ) -> anyhow::Result<Vec<marketplace::ThemeSearchResult>> {
        if let Some(marketplace) = &self.marketplace {
            marketplace.search_themes(query).await
        } else {
            Ok(vec![])
        }
    }

    /// Export a theme for sharing
    pub fn export_theme(&mut self, theme_name: &str, output_path: &Path) -> anyhow::Result<()> {
        let theme = self.manager.load_theme(theme_name)?;
        export::export_theme(&theme, output_path)
    }

    /// Import a theme from a file
    pub fn import_theme(&mut self, theme_path: &Path) -> anyhow::Result<String> {
        let theme = export::import_theme(theme_path)?;
        let theme_name = theme.metadata.name.clone();
        self.manager.install_theme_from_file(theme, theme_path)?;
        Ok(theme_name)
    }

    /// Get the currently active theme
    pub fn get_active_theme(&self) -> Option<&Theme> {
        self.manager.get_active_theme()
    }

    /// Switch to a different theme
    pub fn set_active_theme(&mut self, theme_name: &str) -> anyhow::Result<()> {
        self.manager.set_active_theme(theme_name)
    }

    /// Create a custom theme based on an existing theme
    pub fn create_custom_theme(
        &mut self,
        base_theme: &str,
        name: &str,
        modifications: &config::ThemeModifications,
    ) -> anyhow::Result<Theme> {
        self.manager.create_custom_theme(base_theme, name, modifications)
    }

    /// Enable theme hot-reloading for development
    pub fn enable_hot_reload(&mut self) -> anyhow::Result<()> {
        self.manager.enable_hot_reload()
    }
}

impl Default for ThemeSystem {
    fn default() -> Self {
        Self::new().expect("Failed to create default theme system")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_system_creation() {
        let theme_system = ThemeSystem::new();
        assert!(theme_system.is_ok());
    }

    #[test]
    fn test_list_built_in_themes() {
        let theme_system = ThemeSystem::new().unwrap();
        let themes = theme_system.list_themes().unwrap();

        // Should have at least the built-in themes
        assert!(!themes.is_empty());

        // Should include dark theme
        let has_dark = themes.iter().any(|t| t.name == "dark");
        assert!(has_dark);
    }

    #[cfg(feature = "marketplace")]
    mod marketplace_tests {
        use super::*;
        #[tokio::test]
        async fn test_theme_search() {
            let theme_system = ThemeSystem::new().unwrap();
            // This would normally connect to a real marketplace
            // In tests, we'd mock this or use a test server
            let _results = theme_system.search_themes("dark").await.unwrap_or_default();
            // Just verify the search doesn't crash
            // No assertion on length; reaching here without panic is success
        }
    }
}
