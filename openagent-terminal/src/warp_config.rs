/// Warp Terminal compatible configuration system
/// This matches Warp's settings structure and naming conventions exactly

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main Warp configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpConfig {
    /// Features configuration
    pub features: FeaturesConfig,
    
    /// Editor configuration  
    pub editor: EditorConfig,
    
    /// General configuration
    pub general: GeneralConfig,
    
    /// Appearance configuration
    pub appearance: AppearanceConfig,
    
    /// Keybindings
    pub keybindings: HashMap<String, String>,
    
    /// Custom themes
    pub custom_themes: HashMap<String, Theme>,
}

/// Features configuration section (matches Warp's Settings > Features)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturesConfig {
    /// General features
    pub general: GeneralFeatures,
    
    /// Editor features
    pub editor: EditorFeatures,
}

/// General features (Settings > Features > General)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralFeatures {
    /// Copy on select - automatically copy selected text to clipboard
    pub copy_on_select: bool,
    
    /// Honor user shell configuration
    pub honor_user_shell: bool,
    
    /// Restore tabs and windows on startup
    pub restore_session: bool,
    
    /// Show working directory in tab titles
    pub show_working_directory: bool,
    
    /// Enable command palette
    pub command_palette: bool,
}

/// Editor features (Settings > Features > Editor)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorFeatures {
    /// Autocomplete quotes, brackets, and parentheses
    pub autocomplete_quotes: bool,
    
    /// Soft wrapping in input editor
    pub soft_wrapping: bool,
    
    /// Show line numbers in multiline input
    pub show_line_numbers: bool,
    
    /// Syntax highlighting in input
    pub syntax_highlighting: bool,
}

/// Editor configuration (input editor behavior)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    /// Font family for the input editor
    pub font_family: String,
    
    /// Font size for the input editor
    pub font_size: f32,
    
    /// Line height multiplier
    pub line_height: f32,
    
    /// Cursor style
    pub cursor_style: CursorStyle,
    
    /// Cursor blinking
    pub cursor_blinking: bool,
}

/// General configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Default shell to use
    pub default_shell: String,
    
    /// Working directory behavior
    pub working_directory: WorkingDirectoryConfig,
    
    /// Session management
    pub session: SessionConfig,
    
    /// Privacy settings
    pub privacy: PrivacyConfig,
}

/// Working directory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingDirectoryConfig {
    /// How to handle working directory for new tabs
    pub new_tab_behavior: NewTabBehavior,
    
    /// Show directory in window title
    pub show_in_title: bool,
    
    /// Compact home directory display (~/)
    pub compact_home: bool,
}

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Restore windows on startup
    pub restore_windows: bool,
    
    /// Restore tabs on startup  
    pub restore_tabs: bool,
    
    /// Ask before closing windows
    pub confirm_close: bool,
}

/// Privacy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    /// Analytics and telemetry (always false for Warp compatibility)
    pub analytics_enabled: bool,
    
    /// Crash reporting
    pub crash_reporting: bool,
    
    /// Usage statistics
    pub usage_statistics: bool,
}

/// Appearance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// Current theme name
    pub theme: String,
    
    /// Terminal font configuration
    pub font: FontConfig,
    
    /// Window configuration
    pub window: WindowConfig,
    
    /// Tab configuration
    pub tabs: TabConfig,
    
    /// Color overrides
    pub colors: Option<ColorOverrides>,
}

/// Font configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    /// Font family
    pub family: String,
    
    /// Font size
    pub size: f32,
    
    /// Font weight
    pub weight: FontWeight,
    
    /// Enable font ligatures
    pub ligatures: bool,
}

/// Window configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    /// Window opacity (0.0 - 1.0)
    pub opacity: f32,
    
    /// Blur behind window
    pub blur: bool,
    
    /// Window decorations
    pub decorations: bool,
    
    /// Window padding
    pub padding: WindowPadding,
}

/// Window padding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowPadding {
    pub top: u16,
    pub bottom: u16,
    pub left: u16,
    pub right: u16,
}

/// Tab configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabConfig {
    /// Show tab bar
    pub show_tab_bar: bool,
    
    /// Tab position
    pub position: TabPosition,
    
    /// Show close button on tabs
    pub show_close_button: bool,
    
    /// Show new tab button
    pub show_new_tab_button: bool,
}

/// Theme definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Theme name
    pub name: String,
    
    /// Is dark theme
    pub is_dark: bool,
    
    /// Terminal colors
    pub terminal_colors: TerminalColors,
    
    /// UI colors
    pub ui_colors: UiColors,
}

/// Terminal color scheme
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalColors {
    /// Background color
    pub background: String,
    
    /// Foreground color
    pub foreground: String,
    
    /// Cursor color
    pub cursor: String,
    
    /// Selection background
    pub selection_background: String,
    
    /// Selection foreground
    pub selection_foreground: Option<String>,
    
    /// ANSI colors (0-15)
    pub ansi: [String; 16],
}

/// UI color overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiColors {
    /// Tab bar background
    pub tab_bar_background: Option<String>,
    
    /// Active tab background
    pub active_tab_background: Option<String>,
    
    /// Inactive tab background
    pub inactive_tab_background: Option<String>,
    
    /// Tab text color
    pub tab_text: Option<String>,
    
    /// Window border color
    pub window_border: Option<String>,
}

/// Color overrides for specific elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorOverrides {
    /// Override specific terminal colors
    pub terminal: Option<TerminalColorOverrides>,
    
    /// Override specific UI colors
    pub ui: Option<UiColorOverrides>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalColorOverrides {
    pub background: Option<String>,
    pub foreground: Option<String>,
    pub cursor: Option<String>,
    pub selection_background: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiColorOverrides {
    pub tab_bar_background: Option<String>,
    pub active_tab_background: Option<String>,
}

/// Cursor style options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CursorStyle {
    Block,
    Underline,
    Beam,
}

/// Font weight options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FontWeight {
    Thin,
    ExtraLight,
    Light,
    Regular,
    Medium,
    SemiBold,
    Bold,
    ExtraBold,
    Black,
}

/// Tab position options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TabPosition {
    Top,
    Bottom,
}

/// New tab working directory behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NewTabBehavior {
    /// Use home directory
    Home,
    /// Use current tab's directory
    CurrentDirectory,
    /// Use last used directory
    LastDirectory,
    /// Use a specific directory
    CustomDirectory(PathBuf),
}

impl Default for WarpConfig {
    fn default() -> Self {
        Self {
            features: FeaturesConfig::default(),
            editor: EditorConfig::default(),
            general: GeneralConfig::default(),
            appearance: AppearanceConfig::default(),
            keybindings: default_keybindings(),
            custom_themes: HashMap::new(),
        }
    }
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            general: GeneralFeatures::default(),
            editor: EditorFeatures::default(),
        }
    }
}

impl Default for GeneralFeatures {
    fn default() -> Self {
        Self {
            copy_on_select: false, // Disabled by default in Warp
            honor_user_shell: true,
            restore_session: true,
            show_working_directory: true,
            command_palette: true,
        }
    }
}

impl Default for EditorFeatures {
    fn default() -> Self {
        Self {
            autocomplete_quotes: true,
            soft_wrapping: true,
            show_line_numbers: false,
            syntax_highlighting: true,
        }
    }
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            font_family: "JetBrains Mono".to_string(),
            font_size: 14.0,
            line_height: 1.2,
            cursor_style: CursorStyle::Block,
            cursor_blinking: true,
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            default_shell: if cfg!(target_os = "windows") {
                "powershell".to_string()
            } else {
                "zsh".to_string()
            },
            working_directory: WorkingDirectoryConfig::default(),
            session: SessionConfig::default(),
            privacy: PrivacyConfig::default(),
        }
    }
}

impl Default for WorkingDirectoryConfig {
    fn default() -> Self {
        Self {
            new_tab_behavior: NewTabBehavior::CurrentDirectory,
            show_in_title: true,
            compact_home: true,
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            restore_windows: true,
            restore_tabs: true,
            confirm_close: true,
        }
    }
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            analytics_enabled: false, // Always false for privacy
            crash_reporting: false,
            usage_statistics: false,
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: "Warp Dark".to_string(),
            font: FontConfig::default(),
            window: WindowConfig::default(),
            tabs: TabConfig::default(),
            colors: None,
        }
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: "JetBrains Mono".to_string(),
            size: 14.0,
            weight: FontWeight::Regular,
            ligatures: true,
        }
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            blur: false,
            decorations: true,
            padding: WindowPadding {
                top: 8,
                bottom: 8,
                left: 8,
                right: 8,
            },
        }
    }
}

impl Default for TabConfig {
    fn default() -> Self {
        Self {
            show_tab_bar: true,
            position: TabPosition::Top,
            show_close_button: true,
            show_new_tab_button: true,
        }
    }
}

/// Default Warp keybindings (Linux)
fn default_keybindings() -> HashMap<String, String> {
    let mut bindings = HashMap::new();
    
    // Terminal actions
    bindings.insert("Escape".to_string(), "CloseInputSuggestionsOrHistory".to_string());
    bindings.insert("Ctrl+L".to_string(), "ClearTerminal".to_string());
    bindings.insert("Ctrl+C".to_string(), "ClearEditorBuffer".to_string());
    bindings.insert("Ctrl+U".to_string(), "CopyAndClearCurrentLine".to_string());
    
    // Navigation
    bindings.insert("Alt+Left".to_string(), "MoveToBeginningOfPreviousWord".to_string());
    bindings.insert("Alt+Right".to_string(), "MoveToBeginningOfNextWord".to_string());
    bindings.insert("Ctrl+A".to_string(), "MoveToStartOfLine".to_string());
    bindings.insert("Ctrl+E".to_string(), "MoveToEndOfLine".to_string());
    
    // Selection
    bindings.insert("Shift+Left".to_string(), "SelectCharacterLeft".to_string());
    bindings.insert("Shift+Right".to_string(), "SelectCharacterRight".to_string());
    bindings.insert("Shift+Up".to_string(), "SelectUp".to_string());
    bindings.insert("Shift+Down".to_string(), "SelectDown".to_string());
    
    // Special actions
    bindings.insert("Ctrl+R".to_string(), "CommandSearch".to_string());
    bindings.insert("Ctrl+Shift+D".to_string(), "SplitPane".to_string());
    bindings.insert("Shift+Enter".to_string(), "InsertNewline".to_string());
    
    bindings
}

/// Warp's default theme definitions
pub fn default_themes() -> HashMap<String, Theme> {
    let mut themes = HashMap::new();
    
    // Warp Dark Theme
    themes.insert("Warp Dark".to_string(), Theme {
        name: "Warp Dark".to_string(),
        is_dark: true,
        terminal_colors: TerminalColors {
            background: "#1d1f21".to_string(),
            foreground: "#c5c8c6".to_string(),
            cursor: "#c5c8c6".to_string(),
            selection_background: "#373b41".to_string(),
            selection_foreground: None,
            ansi: [
                "#1d1f21".to_string(), // black
                "#cc6666".to_string(), // red
                "#b5bd68".to_string(), // green
                "#f0c674".to_string(), // yellow
                "#81a2be".to_string(), // blue
                "#b294bb".to_string(), // magenta
                "#8abeb7".to_string(), // cyan
                "#c5c8c6".to_string(), // white
                "#969896".to_string(), // bright black
                "#cc6666".to_string(), // bright red
                "#b5bd68".to_string(), // bright green
                "#f0c674".to_string(), // bright yellow
                "#81a2be".to_string(), // bright blue
                "#b294bb".to_string(), // bright magenta
                "#8abeb7".to_string(), // bright cyan
                "#ffffff".to_string(), // bright white
            ],
        },
        ui_colors: UiColors {
            tab_bar_background: Some("#1a1c1e".to_string()),
            active_tab_background: Some("#1d1f21".to_string()),
            inactive_tab_background: Some("#2a2c2e".to_string()),
            tab_text: Some("#c5c8c6".to_string()),
            window_border: Some("#373b41".to_string()),
        },
    });
    
    // Warp Light Theme
    themes.insert("Warp Light".to_string(), Theme {
        name: "Warp Light".to_string(),
        is_dark: false,
        terminal_colors: TerminalColors {
            background: "#ffffff".to_string(),
            foreground: "#4d4d4c".to_string(),
            cursor: "#4d4d4c".to_string(),
            selection_background: "#d6d6d6".to_string(),
            selection_foreground: None,
            ansi: [
                "#000000".to_string(), // black
                "#c82829".to_string(), // red
                "#718c00".to_string(), // green
                "#eab700".to_string(), // yellow
                "#4271ae".to_string(), // blue
                "#8959a8".to_string(), // magenta
                "#3e999f".to_string(), // cyan
                "#ffffff".to_string(), // white
                "#969896".to_string(), // bright black
                "#c82829".to_string(), // bright red
                "#718c00".to_string(), // bright green
                "#eab700".to_string(), // bright yellow
                "#4271ae".to_string(), // bright blue
                "#8959a8".to_string(), // bright magenta
                "#3e999f".to_string(), // bright cyan
                "#ffffff".to_string(), // bright white
            ],
        },
        ui_colors: UiColors {
            tab_bar_background: Some("#f0f0f0".to_string()),
            active_tab_background: Some("#ffffff".to_string()),
            inactive_tab_background: Some("#e0e0e0".to_string()),
            tab_text: Some("#4d4d4c".to_string()),
            window_border: Some("#d6d6d6".to_string()),
        },
    });
    
    themes
}

/// Configuration loader and manager
pub struct WarpConfigManager {
    config: WarpConfig,
    config_path: PathBuf,
}

impl WarpConfigManager {
    /// Load configuration from file or create default
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Self::config_file_path()?;
        
        let config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            toml::from_str(&content)?
        } else {
            WarpConfig::default()
        };
        
        Ok(Self { config, config_path })
    }
    
    /// Save configuration to file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(&self.config)?;
        std::fs::write(&self.config_path, content)?;
        
        Ok(())
    }
    
    /// Get current configuration
    pub fn config(&self) -> &WarpConfig {
        &self.config
    }
    
    /// Get mutable configuration
    pub fn config_mut(&mut self) -> &mut WarpConfig {
        &mut self.config
    }
    
    /// Get config file path
    fn config_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Could not find config directory")?;
        
        Ok(config_dir.join("warp-terminal").join("warp.toml"))
    }
    
    /// Reset to default configuration
    pub fn reset_to_defaults(&mut self) {
        self.config = WarpConfig::default();
    }
    
    /// Update specific feature setting
    pub fn update_feature(&mut self, feature: &str, enabled: bool) {
        match feature {
            "copy_on_select" => self.config.features.general.copy_on_select = enabled,
            "autocomplete_quotes" => self.config.features.editor.autocomplete_quotes = enabled,
            "soft_wrapping" => self.config.features.editor.soft_wrapping = enabled,
            "syntax_highlighting" => self.config.features.editor.syntax_highlighting = enabled,
            _ => {}
        }
    }
    
    /// Get available themes
    pub fn available_themes(&self) -> Vec<String> {
        let mut themes: Vec<String> = default_themes().keys().cloned().collect();
        themes.extend(self.config.custom_themes.keys().cloned());
        themes.sort();
        themes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_creation() {
        let config = WarpConfig::default();
        
        assert!(!config.features.general.copy_on_select); // Disabled by default in Warp
        assert!(config.features.editor.autocomplete_quotes);
        assert!(config.features.editor.soft_wrapping);
        assert_eq!(config.appearance.theme, "Warp Dark");
        assert_eq!(config.appearance.font.family, "JetBrains Mono");
    }

    #[test]
    fn test_config_serialization() {
        let config = WarpConfig::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: WarpConfig = toml::from_str(&serialized).unwrap();
        
        assert_eq!(config.features.general.copy_on_select, deserialized.features.general.copy_on_select);
        assert_eq!(config.appearance.theme, deserialized.appearance.theme);
    }

    #[test]
    fn test_keybindings() {
        let bindings = default_keybindings();
        
        assert!(bindings.contains_key("Ctrl+L"));
        assert_eq!(bindings.get("Ctrl+L"), Some(&"ClearTerminal".to_string()));
        assert!(bindings.contains_key("Ctrl+R"));
        assert_eq!(bindings.get("Ctrl+R"), Some(&"CommandSearch".to_string()));
    }

    #[test]
    fn test_themes() {
        let themes = default_themes();
        
        assert!(themes.contains_key("Warp Dark"));
        assert!(themes.contains_key("Warp Light"));
        
        let dark_theme = themes.get("Warp Dark").unwrap();
        assert!(dark_theme.is_dark);
        assert_eq!(dark_theme.terminal_colors.background, "#1d1f21");
    }

    #[test]
    fn test_config_manager() {
        // This would need actual file system in real testing environment
        // For now, just test that the structure compiles and basic operations work
        let config = WarpConfig::default();
        assert!(!config.features.general.copy_on_select);
    }
}