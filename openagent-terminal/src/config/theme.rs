use serde::{Deserialize, Serialize};

use openagent_terminal_config_derive::ConfigDeserialize;

use crate::display::color::Rgb;
use openagent_terminal_config::SerdeReplace;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WordBoundaryStyle {
    /// Letters, numbers and underscore are treated as word characters; punctuation separates
    /// words.
    Alnum,
    /// Treat most symbols and punctuation as separate words; jumps stop at transitions of unicode
    /// categories.
    Unicode,
}

impl Default for WordBoundaryStyle {
    fn default() -> Self {
        Self::Alnum
    }
}

impl SerdeReplace for WordBoundaryStyle {
    fn replace(&mut self, value: toml::Value) -> Result<(), Box<dyn std::error::Error>> {
        *self = Self::deserialize(value)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ComposerOpenMode {
    /// Warp-like: click opens panel immediately; any keystroke seeds and opens panel
    Instant,
    /// Buffered in composer and committed on Enter
    Commit,
}

impl Default for ComposerOpenMode {
    fn default() -> Self {
        Self::Instant
    }
}

impl SerdeReplace for ComposerOpenMode {
    fn replace(&mut self, value: toml::Value) -> Result<(), Box<dyn std::error::Error>> {
        *self = Self::deserialize(value)?;
        Ok(())
    }
}

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

    // Palette UI metrics (Warp-like defaults)
    pub palette_pill_radius_px: f32,
    pub palette_title_pad_px: f32,
    pub palette_chip_pad_px: f32,
    pub palette_hint_pad_px: f32,
    pub palette_hint_gap_px: f32,
    pub palette_selection_scale: f32,
    pub palette_hint_border_px: f32,
    pub palette_hint_border_alpha: f32,

    /// Whether to tint palette icons with accent color (true) or use native sprite colors (false)
    pub palette_icon_tint: bool,
    /// Use nearest-neighbor filter for icon sprites (crisper 1x); false uses linear filtering
    pub palette_icon_filter_nearest: bool,
    /// Icon pixel size for palette entries (autofilter: NEAREST at 16px, LINEAR when scaled)
    pub palette_icon_px: f32,

    /// Caret blink rate in milliseconds for the bottom composer. Set to 0 to disable blinking.
    pub composer_blink_rate_ms: u32,
    /// Word-boundary style for word-wise navigation/deletion in the composer.
    pub composer_word_boundary_style: WordBoundaryStyle,
    /// Composer open behavior: "instant" (Warp-like) or "commit"
    pub composer_open_mode: ComposerOpenMode,
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
            palette_pill_radius_px: 10.0,
            palette_title_pad_px: 10.0,
            palette_chip_pad_px: 8.0,
            palette_hint_pad_px: 8.0,
            palette_hint_gap_px: 6.0,
            palette_selection_scale: 0.06,
            palette_hint_border_px: 1.0,
            palette_hint_border_alpha: 0.40,
            palette_icon_tint: true,
            palette_icon_filter_nearest: true,
            palette_icon_px: 16.0,
            composer_blink_rate_ms: 600,
            composer_word_boundary_style: WordBoundaryStyle::Alnum,
            composer_open_mode: ComposerOpenMode::Instant,
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

    /// Optional overrides for sprite rendering (when set, override theme file values)
    #[serde(default)]
    pub palette_icon_tint: Option<bool>,
    #[serde(default)]
    pub palette_icon_filter_nearest: Option<bool>,
    #[serde(default)]
    pub palette_icon_px: Option<f32>,

    /// Optional overrides for composer behavior
    #[serde(default)]
    pub composer_blink_rate_ms: Option<u32>,
    #[serde(default)]
    pub composer_word_boundary_style: Option<WordBoundaryStyle>,
    #[serde(default)]
    pub composer_open_mode: Option<ComposerOpenMode>,
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
            palette_icon_tint: None,
            palette_icon_filter_nearest: None,
            palette_icon_px: None,
            composer_blink_rate_ms: None,
            composer_word_boundary_style: None,
            composer_open_mode: None,
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
                    return self
                        .apply_overrides(resolve_with_name(file, infer_name_from_path(path)));
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
        // Optional sprite overrides
        if let Some(tint) = self.palette_icon_tint {
            resolved.ui.palette_icon_tint = tint;
        }
        if let Some(nearest) = self.palette_icon_filter_nearest {
            resolved.ui.palette_icon_filter_nearest = nearest;
        }
        if let Some(px) = self.palette_icon_px {
            if px > 0.0 {
                resolved.ui.palette_icon_px = px;
            }
        }
        // Composer-specific overrides
        if let Some(ms) = self.composer_blink_rate_ms {
            resolved.ui.composer_blink_rate_ms = ms;
        }
        if let Some(style) = self.composer_word_boundary_style.clone() {
            resolved.ui.composer_word_boundary_style = style;
        }
        if let Some(mode) = self.composer_open_mode.clone() {
            resolved.ui.composer_open_mode = mode;
        }
        resolved
    }
}

fn infer_name_from_path(path: &str) -> String {
    std::path::Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("custom").to_string()
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
        "dark" => Some(ThemeFile { tokens: ThemeTokens::default(), ui: ThemeUi::default() }),
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
