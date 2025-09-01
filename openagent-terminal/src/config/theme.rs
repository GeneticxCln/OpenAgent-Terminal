use serde::{Deserialize, Serialize};

use openagent_terminal_config_derive::ConfigDeserialize;

use crate::display::color::Rgb;

/// UI tokens for theming (colors only). These are mapped to renderer/UI roles.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeTokens {
    pub surface: Rgb,
    pub surface_muted: Rgb,
    pub text: Rgb,
    pub text_muted: Rgb,
    pub accent: Rgb,
    pub success: Rgb,
    pub warning: Rgb,
    pub error: Rgb,
    pub selection: Rgb,
    pub border: Rgb,
    pub overlay: Rgb,
}

impl Default for ThemeTokens {
    fn default() -> Self {
        // Dark defaults
        Self {
            surface: Rgb::new(0x12, 0x12, 0x12),
            surface_muted: Rgb::new(0x1e, 0x1e, 0x1e),
            text: Rgb::new(0xe6, 0xe6, 0xe6),
            text_muted: Rgb::new(0xa0, 0xa0, 0xa0),
            accent: Rgb::new(0x7a, 0xa2, 0xf7),
            success: Rgb::new(0x98, 0xc3, 0x79),
            warning: Rgb::new(0xf4, 0xbf, 0x75),
            error: Rgb::new(0xec, 0x61, 0x61),
            selection: Rgb::new(0x2a, 0x4a, 0x77),
            border: Rgb::new(0x33, 0x33, 0x33),
            overlay: Rgb::new(0x00, 0x00, 0x00),
        }
    }
}

/// UI parameters beyond pure colors.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ThemeUi {
    pub rounded_corners: bool,
    pub corner_radius_px: f32,
    pub shadow: bool,
    pub shadow_alpha: f32,
    pub shadow_size_px: u32,
    pub reduce_motion: bool,
}

impl Default for ThemeUi {
    fn default() -> Self {
        Self {
            rounded_corners: true,
            corner_radius_px: 12.0,
            shadow: true,
            shadow_alpha: 0.35,
            shadow_size_px: 8,
            reduce_motion: false,
        }
    }
}

/// Theme file schema loaded from TOML.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct ThemeFile {
    #[serde(default)]
    pub tokens: ThemeTokens,
    #[serde(default)]
    pub ui: ThemeUi,
}

/// Runtime-resolved theme used by the UI.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ResolvedTheme {
    pub name: String,
    pub tokens: ThemeTokens,
    pub ui: ThemeUi,
}

/// Configuration surface exposed to users for theme selection and overrides.
#[derive(ConfigDeserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ThemeConfig {
    /// Built-in theme name (e.g. "dark", "light", "high-contrast-dark").
    #[serde(default)]
    pub name: Option<String>,

    /// Custom theme file path (TOML). When set, this takes precedence over `name`.
    #[serde(default)]
    pub path: Option<String>,

    /// Global reduce motion preference.
    #[serde(default)]
    pub reduce_motion: bool,

    /// UI shape/visual overrides (optional); when provided these override theme file/built-in.
    #[serde(default)]
    pub rounded_corners: bool,
    #[serde(default)]
    pub corner_radius_px: f32,
    #[serde(default)]
    pub shadow: bool,
    #[serde(default)]
    pub shadow_alpha: f32,
    #[serde(default)]
    pub shadow_size_px: u32,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: Some("dark".into()),
            path: None,
            reduce_motion: false,
            rounded_corners: true,
            corner_radius_px: 12.0,
            shadow: true,
            shadow_alpha: 0.35,
            shadow_size_px: 8,
        }
    }
}

impl ThemeConfig {
    /// Resolve a runtime theme from config. Will gracefully fallback to built-in defaults.
    pub fn resolve(&self) -> ResolvedTheme {
        // 1) Load from custom path, if provided
        if let Some(path) = &self.path {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(file) = toml::from_str::<ThemeFile>(&text) {
                    return self.apply_overrides(resolve_with_name(file, infer_name_from_path(path)));
                }
            }
        }

        // 2) Try built-in by name
        if let Some(name) = &self.name {
            if let Some(file) = builtin_theme(name) {
                return self.apply_overrides(resolve_with_name(file, name.clone()));
            }
        }

        // 3) Fallback to dark built-in
        let file = builtin_theme("dark").unwrap_or_default();
        self.apply_overrides(resolve_with_name(file, "dark".into()))
    }

    fn apply_overrides(&self, mut resolved: ResolvedTheme) -> ResolvedTheme {
        // Apply UI overrides from config
        resolved.ui.reduce_motion |= self.reduce_motion;
        resolved.ui.rounded_corners = self.rounded_corners;
        if self.corner_radius_px > 0.0 {
            resolved.ui.corner_radius_px = self.corner_radius_px;
        }
        resolved.ui.shadow = self.shadow;
        if self.shadow_alpha >= 0.0 {
            resolved.ui.shadow_alpha = self.shadow_alpha;
        }
        if self.shadow_size_px > 0 {
            resolved.ui.shadow_size_px = self.shadow_size_px;
        }
        resolved
    }
}

fn infer_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("custom")
        .to_string()
}

fn resolve_with_name(file: ThemeFile, name: String) -> ResolvedTheme {
    ResolvedTheme { name, tokens: file.tokens, ui: file.ui }
}

// Provide a no-op SerdeReplace so the config derive can skip/ignore this at runtime.
impl openagent_terminal_config::SerdeReplace for ResolvedTheme {
    fn replace(&mut self, _value: toml::Value) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

fn builtin_theme(name: &str) -> Option<ThemeFile> {
    match name.to_ascii_lowercase().as_str() {
        "dark" => Some(ThemeFile {
            tokens: ThemeTokens::default(),
            ui: ThemeUi::default(),
        }),
        "light" => Some(ThemeFile {
            tokens: ThemeTokens {
                surface: Rgb::new(0xf7, 0xf7, 0xf7),
                surface_muted: Rgb::new(0xed, 0xed, 0xed),
                text: Rgb::new(0x18, 0x18, 0x18),
                text_muted: Rgb::new(0x55, 0x55, 0x55),
                accent: Rgb::new(0x3b, 0x82, 0xf6),
                success: Rgb::new(0x16, 0xa3, 0x4a),
                warning: Rgb::new(0xd9, 0x8e, 0x1a),
                error: Rgb::new(0xdc, 0x26, 0x26),
                selection: Rgb::new(0xc7, 0xdd, 0xff),
                border: Rgb::new(0xcc, 0xcc, 0xcc),
                overlay: Rgb::new(0x00, 0x00, 0x00),
            },
            ui: ThemeUi::default(),
        }),
        "high-contrast-dark" | "high_contrast_dark" => Some(ThemeFile {
            tokens: ThemeTokens {
                surface: Rgb::new(0x00, 0x00, 0x00),
                surface_muted: Rgb::new(0x0a, 0x0a, 0x0a),
                text: Rgb::new(0xff, 0xff, 0xff),
                text_muted: Rgb::new(0xd0, 0xd0, 0xd0),
                accent: Rgb::new(0x5a, 0xa6, 0xff),
                success: Rgb::new(0x8b, 0xe1, 0x5a),
                warning: Rgb::new(0xff, 0xc1, 0x5a),
                error: Rgb::new(0xff, 0x66, 0x66),
                selection: Rgb::new(0x22, 0x44, 0x88),
                border: Rgb::new(0x66, 0x66, 0x66),
                overlay: Rgb::new(0x00, 0x00, 0x00),
            },
            ui: ThemeUi { shadow: false, shadow_alpha: 0.0, ..ThemeUi::default() },
        }),
        _ => None,
    }
}

