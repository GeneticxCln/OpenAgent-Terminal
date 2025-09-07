use crate::config::Theme;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct ThemeLoader;

impl ThemeLoader {
    pub fn load_from_file(path: &Path) -> Result<Theme> {
        let content = std::fs::read_to_string(path)?;
        Self::load_from_string(&content)
    }

    pub fn load_from_string(content: &str) -> Result<Theme> {
        toml::from_str(content).map_err(|e| anyhow!("Failed to parse theme: {}", e))
    }

    pub fn load_built_in(name: &str) -> Result<Theme> {
        match name {
            "dark" => Self::load_dark_theme(),
            "light" => Self::load_light_theme(),
            "high-contrast-dark" => Self::load_high_contrast_theme(),
            _ => Err(anyhow!("Unknown built-in theme: {}", name)),
        }
    }

    fn load_dark_theme() -> Result<Theme> {
        // Provide a minimal built-in dark theme without relying on external files
        use crate::config::*;
        let theme = Theme {
            metadata: ThemeMetadata {
                name: "dark".to_string(),
                display_name: "Dark".to_string(),
                description: "Built-in dark theme".to_string(),
                version: "1.0.0".to_string(),
                author: "OpenAgent".to_string(),
                license: None,
                homepage: None,
                repository: None,
                tags: vec!["built-in".to_string(), "dark".to_string()],
                compatibility: ThemeCompatibility {
                    min_version: "0.16.0".to_string(),
                    max_version: None,
                    features: vec![],
                },
                marketplace: MarketplaceInfo::default(),
            },
            tokens: ThemeTokens {
                surface: "#121212".to_string(),
                surface_muted: "#1e1e1e".to_string(),
                surface_elevated: None,
                text: "#e6e6e6".to_string(),
                text_muted: "#a0a0a0".to_string(),
                text_inverse: None,
                accent: "#7aa2f7".to_string(),
                success: "#98c379".to_string(),
                warning: "#f4bf75".to_string(),
                error: "#ec6161".to_string(),
                info: None,
                selection: "#2a4a77".to_string(),
                hover: None,
                active: None,
                focus: None,
                border: "#333333".to_string(),
                border_muted: None,
                overlay: "#000000".to_string(),
                terminal: None,
            },
            ui: UiConfig {
                rounded_corners: true,
                corner_radius_px: 12.0,
                shadow: true,
                shadow_alpha: 0.35,
                shadow_size_px: 8.0,
                reduce_motion: false,
                animation_duration_ms: Some(200),
                easing: Some("ease-in-out".to_string()),
                palette: PaletteConfig {
                    pill_radius_px: 9.0,
                    title_pad_px: 12.0,
                    chip_pad_px: 6.0,
                    hint_pad_px: 6.0,
                    hint_gap_px: 8.0,
                    selection_scale: 0.06,
                    hint_border_px: 1.0,
                    hint_border_alpha: 0.10,
                    icon_tint: true,
                    icon_filter_nearest: false,
                    icon_px: 16.0,
                },
                composer: ComposerConfig {
                    blink_rate_ms: 600,
                    word_boundary_style: "Alnum".to_string(),
                    open_mode: "Instant".to_string(),
                    placeholder_text: Some("Start typing...".to_string()),
                },
                notifications: NotificationConfig {
                    position: "TopRight".to_string(),
                    timeout_ms: 3500,
                    max_width_px: Some(420),
                    animation: true,
                },
            },
            terminal: TerminalConfig::default(),
            extensions: HashMap::new(),
        };
        Ok(theme)
    }

    fn load_light_theme() -> Result<Theme> {
        let mut theme = Self::load_dark_theme()?;
        theme.metadata.name = "light".to_string();
        theme.metadata.display_name = "Light".to_string();
        theme.tokens.surface = "#ffffff".to_string();
        theme.tokens.surface_muted = "#ededed".to_string();
        theme.tokens.text = "#000000".to_string();
        theme.tokens.text_muted = "#555555".to_string();
        theme.tokens.accent = "#3b82f6".to_string();
        Ok(theme)
    }

    fn load_high_contrast_theme() -> Result<Theme> {
        let mut theme = Self::load_dark_theme()?;
        theme.metadata.name = "high-contrast-dark".to_string();
        theme.metadata.display_name = "High Contrast Dark".to_string();
        theme.tokens.surface = "#000000".to_string();
        theme.tokens.surface_muted = "#0a0a0a".to_string();
        theme.tokens.text = "#ffffff".to_string();
        theme.tokens.text_muted = "#d0d0d0".to_string();
        Ok(theme)
    }
}
