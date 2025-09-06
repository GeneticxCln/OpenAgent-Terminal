use crate::config::Theme;
use anyhow::{anyhow, Result};
use regex::Regex;

pub struct ThemeValidator;

impl ThemeValidator {
    pub fn validate(theme: &Theme) -> Result<()> {
        Self::validate_metadata(&theme.metadata)?;
        Self::validate_tokens(&theme.tokens)?;
        Self::validate_ui(&theme.ui)?;
        Ok(())
    }

    fn validate_metadata(metadata: &crate::config::ThemeMetadata) -> Result<()> {
        if metadata.name.is_empty() {
            return Err(anyhow!("Theme name cannot be empty"));
        }

        if !Self::is_valid_theme_name(&metadata.name) {
            return Err(anyhow!("Theme name contains invalid characters"));
        }

        if metadata.version.is_empty() {
            return Err(anyhow!("Theme version cannot be empty"));
        }

        Ok(())
    }

    fn validate_tokens(tokens: &crate::config::ThemeTokens) -> Result<()> {
        Self::validate_color(&tokens.surface, "surface")?;
        Self::validate_color(&tokens.text, "text")?;
        Self::validate_color(&tokens.accent, "accent")?;
        Self::validate_color(&tokens.success, "success")?;
        Self::validate_color(&tokens.warning, "warning")?;
        Self::validate_color(&tokens.error, "error")?;

        Ok(())
    }

    fn validate_ui(ui: &crate::config::UiConfig) -> Result<()> {
        if ui.corner_radius_px < 0.0 || ui.corner_radius_px > 50.0 {
            return Err(anyhow!("Corner radius must be between 0 and 50 pixels"));
        }

        if ui.shadow_alpha < 0.0 || ui.shadow_alpha > 1.0 {
            return Err(anyhow!("Shadow alpha must be between 0.0 and 1.0"));
        }

        Ok(())
    }

    fn validate_color(color: &str, context: &str) -> Result<()> {
        if !Self::is_valid_hex_color(color) {
            return Err(anyhow!("Invalid color '{}' for {}", color, context));
        }
        Ok(())
    }

    fn is_valid_hex_color(color: &str) -> bool {
        let hex_regex = Regex::new(r"^#([0-9A-Fa-f]{3}|[0-9A-Fa-f]{6}|[0-9A-Fa-f]{8})$").unwrap();
        hex_regex.is_match(color)
    }

    fn is_valid_theme_name(name: &str) -> bool {
        let name_regex = Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap();
        name_regex.is_match(name)
    }
}
