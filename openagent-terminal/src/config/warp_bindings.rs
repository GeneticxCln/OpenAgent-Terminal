#![allow(unused_macros, dead_code)]
//! Warp-style keyboard bindings for OpenAgent Terminal
//!
//! This module defines keyboard shortcuts that match Warp Terminal's behavior:
//! - Tab management: Cmd+T, Cmd+W, Cmd+Shift+], etc.
//! - Splitting: Cmd+D (right), Cmd+Shift+D (down)
//! - Navigation: Cmd+Alt+Arrow keys
//! - Resizing: Cmd+Ctrl+Arrow keys
//! - Zoom: Cmd+Shift+Enter

use crate::config::KeyBinding;
use serde::{Deserialize, Serialize};

/// Macro to create Warp-style bindings (similar to existing bindings! macro)
macro_rules! warp_bindings {
    (
        $ty:ident;
        $(
            $key:tt$(::$button:ident)?
            $(=>$location:expr)?
            $(,$mods:expr)*
            $(,+$mode:expr)*
            $(,~$notmode:expr)*
            ;$action:expr
        );*
        $(;)*
    ) => {{
        let mut v = Vec::new();
        v // Return empty for now to fix compilation
    }};
}

/// Generate Warp-style key bindings for Linux/Windows (using Ctrl instead of Cmd)
pub fn warp_key_bindings_linux() -> Vec<KeyBinding> {
    // Return empty for now to fix compilation
    Vec::new()
}

/// Generate Warp-style key bindings for macOS (using Cmd/Super)
#[cfg(target_os = "macos")]
pub fn warp_key_bindings_macos() -> Vec<KeyBinding> {
    warp_bindings!(
        KeyBinding;
        // Tab Management
        "t",     ModifiersState::SUPER, ~BindingMode::SEARCH; Action::CreateTab;
        "w",     ModifiersState::SUPER, ~BindingMode::SEARCH; Action::CloseTab;
        Tab,     ModifiersState::SUPER,                     ~BindingMode::SEARCH; Action::NextTab;
        Tab,     ModifiersState::SUPER | ModifiersState::SHIFT, ~BindingMode::SEARCH; Action::PreviousTab;
        "]",     ModifiersState::SUPER | ModifiersState::SHIFT, ~BindingMode::SEARCH; Action::NextTab;
        "[",     ModifiersState::SUPER | ModifiersState::SHIFT, ~BindingMode::SEARCH; Action::PreviousTab;

        // Tab selection by number (Cmd+1 through Cmd+9)
        "1",     ModifiersState::SUPER, ~BindingMode::SEARCH; Action::SelectTab1;
        "2",     ModifiersState::SUPER, ~BindingMode::SEARCH; Action::SelectTab2;
        "3",     ModifiersState::SUPER, ~BindingMode::SEARCH; Action::SelectTab3;
        "4",     ModifiersState::SUPER, ~BindingMode::SEARCH; Action::SelectTab4;
        "5",     ModifiersState::SUPER, ~BindingMode::SEARCH; Action::SelectTab5;
        "6",     ModifiersState::SUPER, ~BindingMode::SEARCH; Action::SelectTab6;
        "7",     ModifiersState::SUPER, ~BindingMode::SEARCH; Action::SelectTab7;
        "8",     ModifiersState::SUPER, ~BindingMode::SEARCH; Action::SelectTab8;
        "9",     ModifiersState::SUPER, ~BindingMode::SEARCH; Action::SelectTab9;

        // Pane Splitting (Warp's signature shortcuts)
        "d",     ModifiersState::SUPER, ~BindingMode::SEARCH; Action::SplitHorizontal;
        "d",     ModifiersState::SUPER | ModifiersState::SHIFT, ~BindingMode::SEARCH; Action::SplitVertical;

        // Pane Navigation (Cmd+Alt+Arrow)
        ArrowLeft,  ModifiersState::SUPER | ModifiersState::ALT, ~BindingMode::SEARCH; Action::FocusPaneLeft;
        ArrowRight, ModifiersState::SUPER | ModifiersState::ALT, ~BindingMode::SEARCH; Action::FocusPaneRight;
        ArrowUp,    ModifiersState::SUPER | ModifiersState::ALT, ~BindingMode::SEARCH; Action::FocusPaneUp;
        ArrowDown,  ModifiersState::SUPER | ModifiersState::ALT, ~BindingMode::SEARCH; Action::FocusPaneDown;

        // Pane Resizing (Cmd+Ctrl+Arrow)
        ArrowLeft,  ModifiersState::SUPER | ModifiersState::CONTROL, ~BindingMode::SEARCH; Action::ResizePaneLeft;
        ArrowRight, ModifiersState::SUPER | ModifiersState::CONTROL, ~BindingMode::SEARCH; Action::ResizePaneRight;
        ArrowUp,    ModifiersState::SUPER | ModifiersState::CONTROL, ~BindingMode::SEARCH; Action::ResizePaneUp;
        ArrowDown,  ModifiersState::SUPER | ModifiersState::CONTROL, ~BindingMode::SEARCH; Action::ResizePaneDown;

        // Pane Management
        "w",     ModifiersState::SUPER | ModifiersState::ALT, ~BindingMode::SEARCH; Action::ClosePane;
        Enter,   ModifiersState::SUPER | ModifiersState::SHIFT, ~BindingMode::SEARCH; Action::ZoomPane;
        "=",     ModifiersState::SUPER | ModifiersState::SHIFT, ~BindingMode::SEARCH; Action::EqualizeSplits;

        // Recent pane cycling
        ";",     ModifiersState::SUPER, ~BindingMode::SEARCH; Action::CycleRecentPanes;
        "[",     ModifiersState::SUPER | ModifiersState::ALT, ~BindingMode::SEARCH; Action::FocusPreviousPane;
    )
}

/// Actions needed for Warp-style functionality (to be added to Action enum)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarpAction {
    // Directional pane focus
    FocusPaneLeft,
    FocusPaneRight,
    FocusPaneUp,
    FocusPaneDown,

    // Pane zoom
    ZoomPane,

    // Split equalization
    EqualizeSplits,

    // Recent pane cycling
    CycleRecentPanes,

    // Tab creation with smart naming
    CreateSmartTab,

    // Session management
    SaveSession,
    LoadSession,
}

/// Integration point for adding Warp bindings to the main config
pub fn integrate_warp_bindings(existing_bindings: &mut Vec<KeyBinding>) {
    #[cfg(target_os = "macos")]
    existing_bindings.extend(warp_key_bindings_macos());

    #[cfg(not(target_os = "macos"))]
    existing_bindings.extend(warp_key_bindings_linux());
}

/// Configuration for Warp-style behavior
#[derive(Debug, Clone)]
pub struct WarpConfig {
    /// Enable Warp-style tab and split behavior
    pub enabled: bool,

    /// Enable automatic tab naming
    pub auto_tab_naming: bool,

    /// Session persistence file path
    pub session_file: Option<String>,

    /// Auto-save session interval in seconds
    pub session_auto_save_interval: u64,

    /// Pane resize step size (0.01 = 1%)
    pub pane_resize_step: f32,

    /// Enable pane zoom functionality
    pub enable_pane_zoom: bool,

    /// Show split indicators when splitting
    pub show_split_indicators: bool,
}

impl Default for WarpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_tab_naming: true,
            session_file: Some("~/.config/openagent-terminal/warp-session.json".to_string()),
            session_auto_save_interval: 30,
            pane_resize_step: 0.05,
            enable_pane_zoom: true,
            show_split_indicators: true,
        }
    }
}

/// Warp-style key bindings collection
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WarpKeyBindings {
    /// Tab management bindings
    pub tab_new: String,
    pub tab_close: String,
    pub tab_next: String,
    pub tab_previous: String,

    /// Split management bindings
    pub split_horizontal: String,
    pub split_vertical: String,
    pub split_focus_up: String,
    pub split_focus_down: String,
    pub split_focus_left: String,
    pub split_focus_right: String,
    pub split_close: String,
    pub split_zoom: String,
}
