//! Workspace configuration for tabs and split panes

use openagent_terminal_config_derive::{ConfigDeserialize, SerdeReplace};
use serde::{Deserialize, Serialize};
use crate::display::color::Rgb;

/// Workspace configuration
#[derive(ConfigDeserialize, Serialize, Debug, Clone, PartialEq)]
pub struct WorkspaceConfig {
    /// Whether workspace features are enabled
    #[config(default = true)]
    pub enabled: bool,

    /// Tab bar configuration
    #[config(default)]
    pub tab_bar: TabBarConfig,

    /// Split pane configuration
    #[config(default)]
    pub splits: SplitConfig,

    /// Workspace keybindings (handled by main keybinding system)
    #[config(skip)]
    pub keybindings: WorkspaceKeybindings,

    /// Enable Warp-style enhanced tab and split functionality
    #[config(default = true)]
    pub warp_style: bool,

    /// File to store Warp session data (optional)
    #[config(skip)]
    pub warp_session_file: Option<std::path::PathBuf>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tab_bar: TabBarConfig::default(),
            splits: SplitConfig::default(),
            keybindings: WorkspaceKeybindings::default(),
            warp_style: true,
            warp_session_file: None,
        }
    }
}

/// Tab bar configuration
#[derive(ConfigDeserialize, Serialize, Debug, Clone, PartialEq)]
pub struct TabBarConfig {
    /// Position of the tab bar
    #[config(default = "TabBarPosition::Top")]
    pub position: TabBarPosition,

    /// Whether to show the tab bar
    #[config(default = true)]
    pub show: bool,

    /// Show close button on tabs
    #[config(default = true)]
    pub show_close_button: bool,

    /// Show modified indicator
    #[config(default = true)]
    pub show_modified_indicator: bool,

    /// Reserve a terminal row for the tab bar (avoids overlaying content)
    /// Note: Top reservation hides the top line of grid content; Bottom reservation hides the
    /// bottom line. Future versions may shift the grid instead of hiding.
    #[config(default = true)]
    pub reserve_row: bool,

    /// Maximum tab title length
    #[config(default = 20)]
    pub max_title_length: usize,

    /// What to do when creating a new tab
    #[config(default = "NewTabAction::InheritWorkingDir")]
    pub new_tab_action: NewTabAction,
}

impl Default for TabBarConfig {
    fn default() -> Self {
        Self {
            position: TabBarPosition::Top,
            show: true,
            show_close_button: true,
            show_modified_indicator: true,
            max_title_length: 20,
            new_tab_action: NewTabAction::InheritWorkingDir,
            reserve_row: true,
        }
    }
}

/// Split pane configuration
#[derive(ConfigDeserialize, Serialize, Debug, Clone, PartialEq)]
pub struct SplitConfig {
    /// Whether to show borders between splits
    #[config(default = true)]
    pub borders: bool,

    /// Border thickness in pixels
    #[config(default = 1.0)]
    pub border_thickness: f32,

    /// Default split ratio for new splits
    #[config(default = 0.5)]
    pub default_ratio: f32,

    /// Minimum pane size in lines/columns
    #[config(default = 10)]
    pub minimum_pane_size: usize,

    /// Resize increment in cells
    #[config(default = 5)]
    pub resize_increment: usize,

    // --- Visual indicator configuration for Warp-style splits ---
    /// Enable split preview/indicators overlay
    #[config(default = true)]
    pub preview_enabled: bool,
    /// Base line width for split indicators (px)
    #[config(default = 2.5)]
    pub indicator_line_width: f32,
    /// Base line alpha for split indicators (0..1)
    #[config(default = 0.5)]
    pub indicator_line_alpha: f32,
    /// Multiplier applied to line width when a divider is hovered/dragged
    #[config(default = 2.0)]
    pub indicator_hover_scale: f32,
    /// Line alpha when hovered/dragged (0..1)
    #[config(default = 0.95)]
    pub indicator_hover_alpha: f32,
    /// Handle size baseline (px). Actual size also adapts to divider length.
    #[config(default = 8.0)]
    pub handle_size: f32,
    /// Handle alpha (0..1)
    #[config(default = 0.95)]
    pub handle_alpha: f32,
    /// Whether to show the resize handle on hover
    #[config(default = true)]
    pub show_resize_handles: bool,
    /// Optional explicit colors for split indicators (overrides theme tokens when set)
    #[serde(default)]
    pub indicator_line_color: Option<Rgb>,
    #[serde(default)]
    pub handle_color: Option<Rgb>,
    #[serde(default)]
    pub overlay_color: Option<Rgb>,
    /// Alpha for the zoom overlay when a pane is zoomed
    #[config(default = 0.06)]
    pub zoom_overlay_alpha: f32,
}

impl Default for SplitConfig {
    fn default() -> Self {
        Self {
            borders: true,
            border_thickness: 1.0,
            default_ratio: 0.5,
            minimum_pane_size: 10,
            resize_increment: 5,
            preview_enabled: true,
            indicator_line_width: 2.5,
            indicator_line_alpha: 0.5,
            indicator_hover_scale: 2.0,
            indicator_hover_alpha: 0.95,
            handle_size: 8.0,
            handle_alpha: 0.95,
            show_resize_handles: true,
            indicator_line_color: None,
            handle_color: None,
            overlay_color: None,
            zoom_overlay_alpha: 0.06,
        }
    }
}

/// Tab bar position
#[derive(ConfigDeserialize, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TabBarPosition {
    Top,
    Bottom,
    Hidden,
}

impl Default for TabBarPosition {
    fn default() -> Self {
        Self::Top
    }
}

/// Action when creating a new tab
#[allow(clippy::enum_variant_names)]
#[derive(ConfigDeserialize, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum NewTabAction {
    /// Inherit the working directory from the current tab
    InheritWorkingDir,
    /// Start in the home directory
    HomeDir,
    /// Use the last used directory
    LastUsedDir,
    /// Use a specific directory (configured separately)
    CustomDir,
}

impl Default for NewTabAction {
    fn default() -> Self {
        Self::InheritWorkingDir
    }
}

/// Workspace keybindings (for documentation, actual bindings in main keybinding system)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerdeReplace)]
pub struct WorkspaceKeybindings {
    pub new_tab: String,
    pub close_tab: String,
    pub next_tab: String,
    pub previous_tab: String,
    pub split_horizontal: String,
    pub split_vertical: String,
    pub focus_next_pane: String,
    pub focus_previous_pane: String,
    pub close_pane: String,
}

impl Default for WorkspaceKeybindings {
    fn default() -> Self {
        Self {
            new_tab: "Ctrl+Shift+T".to_string(),
            close_tab: "Ctrl+Shift+W".to_string(),
            next_tab: "Ctrl+Tab".to_string(),
            previous_tab: "Ctrl+Shift+Tab".to_string(),
            split_horizontal: "Ctrl+Shift+H".to_string(),
            split_vertical: "Ctrl+Shift+V".to_string(),
            focus_next_pane: "Ctrl+Shift+Right".to_string(),
            focus_previous_pane: "Ctrl+Shift+Left".to_string(),
            close_pane: "Ctrl+Shift+Q".to_string(),
        }
    }
}
