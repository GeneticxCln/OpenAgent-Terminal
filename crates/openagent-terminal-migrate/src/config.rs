use crate::cli::TerminalType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for a migration operation
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    pub terminal_type: TerminalType,
    pub config_path: PathBuf,
    pub detected_automatically: bool,
}

/// Unified configuration structure that represents terminal settings
/// in a format-agnostic way before conversion to OpenAgent Terminal config
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnifiedConfig {
    // Window settings
    pub window: WindowConfig,
    // Font settings
    pub font: FontConfig,
    // Color settings
    pub colors: ColorConfig,
    // Terminal behavior
    pub terminal: TerminalConfig,
    // Shell configuration
    pub shell: ShellConfig,
    // Key bindings
    pub key_bindings: Vec<KeyBinding>,
    // Any additional/custom settings that don't fit standard categories
    pub custom: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowConfig {
    pub opacity: Option<f32>,
    pub padding: Option<PaddingConfig>,
    pub decorations: Option<String>,
    pub startup_mode: Option<String>,
    pub title: Option<String>,
    pub class: Option<String>,
    pub dynamic_title: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaddingConfig {
    pub x: Option<i32>,
    pub y: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FontConfig {
    pub size: Option<f32>,
    pub normal: Option<FontFaceConfig>,
    pub bold: Option<FontFaceConfig>,
    pub italic: Option<FontFaceConfig>,
    pub bold_italic: Option<FontFaceConfig>,
    pub builtin_box_drawing: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FontFaceConfig {
    pub family: Option<String>,
    pub style: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ColorConfig {
    pub primary: Option<PrimaryColors>,
    pub normal: Option<NormalColors>,
    pub bright: Option<BrightColors>,
    pub dim: Option<DimColors>,
    pub cursor: Option<CursorColors>,
    pub selection: Option<SelectionColors>,
    pub search: Option<SearchColors>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrimaryColors {
    pub background: Option<String>,
    pub foreground: Option<String>,
    pub bright_foreground: Option<String>,
    pub dim_foreground: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NormalColors {
    pub black: Option<String>,
    pub red: Option<String>,
    pub green: Option<String>,
    pub yellow: Option<String>,
    pub blue: Option<String>,
    pub magenta: Option<String>,
    pub cyan: Option<String>,
    pub white: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrightColors {
    pub black: Option<String>,
    pub red: Option<String>,
    pub green: Option<String>,
    pub yellow: Option<String>,
    pub blue: Option<String>,
    pub magenta: Option<String>,
    pub cyan: Option<String>,
    pub white: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DimColors {
    pub black: Option<String>,
    pub red: Option<String>,
    pub green: Option<String>,
    pub yellow: Option<String>,
    pub blue: Option<String>,
    pub magenta: Option<String>,
    pub cyan: Option<String>,
    pub white: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CursorColors {
    pub text: Option<String>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SelectionColors {
    pub text: Option<String>,
    pub background: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchColors {
    pub matches: Option<MatchColors>,
    pub focused_match: Option<MatchColors>,
    pub bar: Option<BarColors>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MatchColors {
    pub foreground: Option<String>,
    pub background: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BarColors {
    pub background: Option<String>,
    pub foreground: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TerminalConfig {
    pub scrolling: Option<ScrollingConfig>,
    pub cursor: Option<CursorConfig>,
    pub selection: Option<SelectionConfig>,
    pub mouse: Option<MouseConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScrollingConfig {
    pub history: Option<u32>,
    pub multiplier: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CursorConfig {
    pub style: Option<CursorStyle>,
    pub vi_mode_style: Option<CursorStyle>,
    pub blinking: Option<String>,
    pub thickness: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CursorStyle {
    pub shape: Option<String>,
    pub blinking: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SelectionConfig {
    pub semantic_escape_chars: Option<String>,
    pub save_to_clipboard: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MouseConfig {
    pub hide_when_typing: Option<bool>,
    pub bindings: Option<Vec<MouseBinding>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseBinding {
    pub mouse: String,
    pub action: String,
    pub mods: Option<Vec<String>>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShellConfig {
    pub program: Option<String>,
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: String,
    pub mods: Option<Vec<String>>,
    pub action: Option<String>,
    pub chars: Option<String>,
    pub command: Option<Command>,
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub program: String,
    pub args: Option<Vec<String>>,
}

impl UnifiedConfig {
    /// Create a new empty unified config
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge another config into this one, with the other config taking priority
    pub fn merge(&mut self, other: UnifiedConfig) {
        // Merge window config
        if other.window.opacity.is_some() {
            self.window.opacity = other.window.opacity;
        }
        if other.window.padding.is_some() {
            self.window.padding = other.window.padding;
        }
        // ... (continue for all fields)

        // Merge custom fields
        for (key, value) in other.custom {
            self.custom.insert(key, value);
        }
    }

    /// Validate the configuration for any obvious issues
    pub fn validate(&self) -> Result<(), String> {
        // Check font size range
        if let Some(size) = self.font.size {
            if size <= 0.0 || size > 72.0 {
                return Err(format!("Font size {} is out of reasonable range (0-72)", size));
            }
        }

        // Check opacity range
        if let Some(opacity) = self.window.opacity {
            if opacity < 0.0 || opacity > 1.0 {
                return Err(format!("Window opacity {} is out of range (0.0-1.0)", opacity));
            }
        }

        // Validate color format (basic check for hex colors)
        if let Some(bg) = &self.colors.primary.as_ref().and_then(|p| p.background.as_ref()) {
            if !bg.starts_with('#') || bg.len() != 7 {
                return Err(format!(
                    "Background color '{}' doesn't appear to be a valid hex color",
                    bg
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_config_new() {
        let config = UnifiedConfig::new();
        assert!(config.font.size.is_none());
        assert!(config.custom.is_empty());
    }

    #[test]
    fn test_unified_config_validation() {
        let mut config = UnifiedConfig::new();

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Invalid font size should fail
        config.font.size = Some(-5.0);
        assert!(config.validate().is_err());

        // Valid font size should pass
        config.font.size = Some(12.0);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_migration_config_creation() {
        let config = MigrationConfig {
            terminal_type: TerminalType::Alacritty,
            config_path: PathBuf::from("/home/user/.alacritty.yml"),
            detected_automatically: true,
        };

        assert_eq!(config.terminal_type, TerminalType::Alacritty);
        assert!(config.detected_automatically);
    }
}
