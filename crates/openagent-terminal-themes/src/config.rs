use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete theme definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub metadata: ThemeMetadata,
    pub tokens: ThemeTokens,
    pub ui: UiConfig,
    #[serde(default)]
    pub terminal: TerminalConfig,
    #[serde(default)]
    pub extensions: HashMap<String, serde_json::Value>,
}

/// Theme metadata for identification and marketplace integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeMetadata {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub tags: Vec<String>,
    pub compatibility: ThemeCompatibility,
    #[serde(default)]
    pub marketplace: MarketplaceInfo,
}

/// Theme compatibility information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeCompatibility {
    pub min_version: String,
    pub max_version: Option<String>,
    pub features: Vec<String>,
}

/// Marketplace-specific information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MarketplaceInfo {
    pub id: Option<String>,
    pub download_count: u64,
    pub rating: f32,
    pub last_updated: Option<String>,
    pub screenshots: Vec<String>,
    pub checksum: Option<String>,
}

/// Color tokens and semantic colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeTokens {
    // Surface colors
    pub surface: String,
    pub surface_muted: String,
    pub surface_elevated: Option<String>,

    // Text colors
    pub text: String,
    pub text_muted: String,
    pub text_inverse: Option<String>,

    // Semantic colors
    pub accent: String,
    pub success: String,
    pub warning: String,
    pub error: String,
    pub info: Option<String>,

    // Interactive colors
    pub selection: String,
    pub hover: Option<String>,
    pub active: Option<String>,
    pub focus: Option<String>,

    // UI element colors
    pub border: String,
    pub border_muted: Option<String>,
    pub overlay: String,

    // Terminal-specific colors
    pub terminal: Option<TerminalColorScheme>,
}

/// Terminal color scheme
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalColorScheme {
    pub foreground: String,
    pub background: String,
    pub cursor: Option<String>,
    pub selection_background: Option<String>,
    pub selection_foreground: Option<String>,

    // ANSI colors
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,

    // Bright ANSI colors
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    pub bright_magenta: String,
    pub bright_cyan: String,
    pub bright_white: String,
}

/// UI configuration and styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    // Visual styling
    pub rounded_corners: bool,
    pub corner_radius_px: f32,
    pub shadow: bool,
    pub shadow_alpha: f32,
    pub shadow_size_px: f32,

    // Animation preferences
    pub reduce_motion: bool,
    pub animation_duration_ms: Option<u32>,
    pub easing: Option<String>,

    // Component-specific settings
    pub palette: PaletteConfig,
    pub composer: ComposerConfig,
    pub notifications: NotificationConfig,
}

/// Command palette configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaletteConfig {
    pub pill_radius_px: f32,
    pub title_pad_px: f32,
    pub chip_pad_px: f32,
    pub hint_pad_px: f32,
    pub hint_gap_px: f32,
    pub selection_scale: f32,
    pub hint_border_px: f32,
    pub hint_border_alpha: f32,

    // Icon settings
    pub icon_tint: bool,
    pub icon_filter_nearest: bool,
    pub icon_px: f32,
}

/// Command composer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposerConfig {
    pub blink_rate_ms: u32,
    pub word_boundary_style: String,
    pub open_mode: String,
    pub placeholder_text: Option<String>,
}

/// Notification styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub position: String,
    pub timeout_ms: u32,
    pub max_width_px: Option<u32>,
    pub animation: bool,
}

/// Terminal-specific configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TerminalConfig {
    pub cursor_style: Option<CursorStyle>,
    pub selection_style: Option<SelectionStyle>,
    pub scrollbar_style: Option<ScrollbarStyle>,
}

/// Cursor styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorStyle {
    pub color: Option<String>,
    pub thickness: Option<f32>,
    pub blink_rate_ms: Option<u32>,
}

/// Selection styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionStyle {
    pub background_color: Option<String>,
    pub foreground_color: Option<String>,
    pub border_color: Option<String>,
}

/// Scrollbar styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollbarStyle {
    pub width_px: f32,
    pub background_color: String,
    pub thumb_color: String,
    pub hover_color: Option<String>,
}

/// Theme modifications for customization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeModifications {
    pub token_overrides: Option<HashMap<String, String>>,
    pub ui_overrides: Option<serde_json::Value>,
    pub terminal_overrides: Option<TerminalColorScheme>,
    pub metadata_changes: Option<HashMap<String, String>>,
}

/// Theme export format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemePackage {
    pub theme: Theme,
    pub assets: Option<Vec<ThemeAsset>>,
    pub manifest: PackageManifest,
}

/// Theme asset (e.g., screenshots, icons)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeAsset {
    pub name: String,
    pub path: String,
    pub asset_type: AssetType,
    pub size_bytes: u64,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssetType {
    Screenshot,
    Icon,
    Preview,
    Documentation,
    Other(String),
}

/// Package manifest for distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    pub format_version: String,
    pub created_at: String,
    pub created_by: String,
    pub checksum: String,
    pub signature: Option<String>,
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self {
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
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            rounded_corners: true,
            corner_radius_px: 12.0,
            shadow: true,
            shadow_alpha: 0.35,
            shadow_size_px: 8.0,
            reduce_motion: false,
            animation_duration_ms: None,
            easing: None,
            palette: PaletteConfig::default(),
            composer: ComposerConfig::default(),
            notifications: NotificationConfig::default(),
        }
    }
}

impl Default for PaletteConfig {
    fn default() -> Self {
        Self {
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
        }
    }
}

impl Default for ComposerConfig {
    fn default() -> Self {
        Self {
            blink_rate_ms: 600,
            word_boundary_style: "Alnum".to_string(),
            open_mode: "Instant".to_string(),
            placeholder_text: None,
        }
    }
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            position: "TopRight".to_string(),
            timeout_ms: 5000,
            max_width_px: Some(400),
            animation: true,
        }
    }
}

impl ThemeCompatibility {
    pub fn is_compatible(&self, current_version: &str) -> bool {
        // Simple semver compatibility check
        let Ok(current) = semver::Version::parse(current_version) else {
            return false;
        };
        let Ok(min) = semver::Version::parse(&self.min_version) else {
            return false;
        };

        if current < min {
            return false;
        }

        if let Some(max_version) = &self.max_version {
            let Ok(max) = semver::Version::parse(max_version) else {
                return false;
            };
            if current > max {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_compatibility() {
        let compat = ThemeCompatibility {
            min_version: "1.0.0".to_string(),
            max_version: Some("2.0.0".to_string()),
            features: vec![],
        };

        assert!(compat.is_compatible("1.5.0"));
        assert!(!compat.is_compatible("0.9.0"));
        assert!(!compat.is_compatible("2.1.0"));
    }

    #[test]
    fn test_theme_serialization() {
        let theme = Theme {
            metadata: ThemeMetadata {
                name: "test".to_string(),
                display_name: "Test Theme".to_string(),
                description: "A test theme".to_string(),
                version: "1.0.0".to_string(),
                author: "Test Author".to_string(),
                license: None,
                homepage: None,
                repository: None,
                tags: vec!["test".to_string()],
                compatibility: ThemeCompatibility {
                    min_version: "1.0.0".to_string(),
                    max_version: None,
                    features: vec![],
                },
                marketplace: MarketplaceInfo::default(),
            },
            tokens: ThemeTokens::default(),
            ui: UiConfig::default(),
            terminal: TerminalConfig::default(),
            extensions: HashMap::new(),
        };

        let serialized = toml::to_string(&theme).unwrap();
        let deserialized: Theme = toml::from_str(&serialized).unwrap();

        assert_eq!(theme.metadata.name, deserialized.metadata.name);
    }
}
