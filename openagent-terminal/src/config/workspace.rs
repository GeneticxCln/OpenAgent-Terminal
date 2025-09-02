//! Workspace configuration for tabs and split panes

use openagent_terminal_config_derive::{ConfigDeserialize, SerdeReplace};
use serde::{Deserialize, Serialize};

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
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tab_bar: TabBarConfig::default(),
            splits: SplitConfig::default(),
            keybindings: WorkspaceKeybindings::default(),
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
}

impl Default for SplitConfig {
    fn default() -> Self {
        Self {
            borders: true,
            border_thickness: 1.0,
            default_ratio: 0.5,
            minimum_pane_size: 10,
            resize_increment: 5,
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
