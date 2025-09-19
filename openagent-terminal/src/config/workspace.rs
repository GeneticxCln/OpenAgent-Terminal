//! Workspace configuration for tabs and split panes

use crate::display::color::Rgb;
use log::warn;
use openagent_terminal_config::SerdeReplace;
use openagent_terminal_config_derive::{ConfigDeserialize, SerdeReplace};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

    /// Quick Actions bar configuration
    #[config(default)]
    pub quick_actions: QuickActionsConfig,

    /// Workspace keybindings (handled by main keybinding system)
    #[config(skip)]
    pub keybindings: WorkspaceKeybindings,

    /// Enable Warp-style enhanced visuals/UX
    #[config(default = true)]
    pub warp_style: bool,

    /// Enable Warp-style completions overlay
    #[config(default = true)]
    pub completions_enabled: bool,

    /// When using Warp-style UI, draw the tab bar as an overlay without reserving a grid row.
    /// This prevents the legacy/classic row reservation path from interfering with the layout.
    #[config(default = true)]
    pub warp_overlay_only: bool,

    /// File to store Warp session data (optional)
    /// Deprecated in favor of [workspace.sessions.file_path] but still supported.
    #[serde(default)]
    pub warp_session_file: Option<PathBuf>,

    /// Session persistence settings
    #[config(default)]
    pub sessions: SessionConfig,

    /// Warp-style keybindings toggle and options
    #[config(default)]
    pub warp_style_bindings: WarpStyleBindingsConfig,

    /// Drag gesture configuration (pane drag, etc.)
    #[config(default)]
    pub drag: DragConfig,

    /// Clean startup: suppress non-essential overlays (tab bar reservation, quick actions,
    /// bottom composer band) until the first terminal output renders.
    /// Set to false to show full UI immediately on startup.
    #[config(default = true)]
    pub clean_startup: bool,

    /// Focus follows mouse: when enabled, moving the mouse over a pane focuses it.
    #[config(default = false)]
    pub focus_follows_mouse: bool,

    /// Dim inactive panes to better indicate focus.
    #[config(default = false)]
    pub dim_inactive_panes: bool,

    /// Alpha for dimming inactive panes (0..1). Applied with theme overlay color.
    #[config(default = 0.15)]
    pub dim_inactive_alpha: f32,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tab_bar: TabBarConfig::default(),
            splits: SplitConfig::default(),
            quick_actions: QuickActionsConfig::default(),
            keybindings: WorkspaceKeybindings::default(),
            warp_style: true,
            completions_enabled: true,
            warp_session_file: None,
            sessions: SessionConfig::default(),
            warp_style_bindings: WarpStyleBindingsConfig::default(),
            drag: DragConfig::default(),
            clean_startup: true,
            warp_overlay_only: true,
            focus_follows_mouse: false,
            dim_inactive_panes: false,
            dim_inactive_alpha: 0.15,
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

    /// Visibility behavior
    /// - Auto: Windowed/Maximized -> Always, Fullscreen -> Hover
    /// - Always: Always visible
    /// - Hover: Only show on hover (near edge or recent hover)
    #[config(default = "TabBarVisibility::Auto")]
    pub visibility: TabBarVisibility,

    /// Show close button on tabs
    #[config(default = true)]
    pub show_close_button: bool,

    /// Only render the close button on hover (still clickable in the region)
    #[config(default = false)]
    pub close_button_on_hover: bool,

    /// Show modified indicator
    #[config(default = true)]
    pub show_modified_indicator: bool,

    /// Show a [+] button to create new tabs when there is remaining space
    #[config(default = true)]
    pub show_new_tab_button: bool,

    /// Show tab numbers/indices before the title (e.g. "1: title")
    #[config(default = false)]
    pub show_tab_numbers: bool,

    // Reserve row is deprecated in Warp-only layout and removed.
    /// Maximum tab title length
    #[config(default = 20)]
    pub max_title_length: usize,

    /// What to do when creating a new tab
    #[config(default = "NewTabAction::InheritWorkingDir")]
    pub new_tab_action: NewTabAction,
}

/// Quick Actions bar configuration
#[derive(ConfigDeserialize, Serialize, Debug, Clone, PartialEq)]
pub struct QuickActionsConfig {
    /// Show the Quick Actions bar
    #[config(default = true)]
    pub show: bool,
    /// Position of the bar: Auto chooses bottom/top intelligently, Top/Bottom force a side
    #[config(default = "QuickActionsPosition::Auto")]
    pub position: QuickActionsPosition,
    /// Show the Palette label in the Quick Actions bar
    #[config(default = true)]
    pub show_palette: bool,
}

impl Default for QuickActionsConfig {
    fn default() -> Self {
        // Turn off by default; can be enabled in config
        Self {
            show: false,
            position: QuickActionsPosition::Auto,
            show_palette: true,
        }
    }
}

/// Quick Actions bar position
#[derive(ConfigDeserialize, Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum QuickActionsPosition {
    #[default]
    Auto,
    Top,
    Bottom,
}

/// Tab bar visibility behavior
#[derive(ConfigDeserialize, Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum TabBarVisibility {
    #[default]
    Auto,
    Always,
    Hover,
}

impl Default for TabBarConfig {
    fn default() -> Self {
        Self {
            position: TabBarPosition::Top,
            show: true,
            visibility: TabBarVisibility::Auto,
            show_close_button: true,
            close_button_on_hover: false,
            show_modified_indicator: true,
            show_new_tab_button: true,
            show_tab_numbers: false,
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

/// Drag configuration for workspace interactions
#[derive(ConfigDeserialize, Serialize, Debug, Clone, PartialEq)]
pub struct DragConfig {
    /// Enable pane drag-and-drop gesture
    #[config(default = true)]
    pub enable_pane_drag: bool,
    /// Modifier required for pane drag
    #[config(default = "DragModifier::Alt")]
    pub pane_drag_modifier: DragModifier,
    /// Mouse button for pane drag
    #[config(default = "DragButton::Left")]
    pub pane_drag_button: DragButton,

    // --- Visual highlights for drag drop zones (overrides theme tokens when set) ---
    /// Optional explicit highlight color for drag drop zones (tabs, new-tab area, split targets)
    #[serde(default)]
    pub highlight_color: HighlightColorOpt,
    /// Minimum alpha floor for highlights in light themes to ensure visibility
    #[config(default = 0.08)]
    pub highlight_min_alpha: f32,
    /// Base alpha for general drop highlights (split targets)
    #[config(default = 0.15)]
    pub highlight_alpha_base: f32,
    /// Hover/active alpha for general drop highlights (split targets)
    #[config(default = 0.5)]
    pub highlight_alpha_hover: f32,
    /// Base alpha for tab highlight when hovering a tab as a drop target
    #[config(default = 0.12)]
    pub tab_highlight_alpha_base: f32,
    /// Hover/active alpha for tab highlight when hovering a tab as a drop target
    #[config(default = 0.4)]
    pub tab_highlight_alpha_hover: f32,
    /// Base alpha for the New Tab area highlight when dropping to create a new tab
    #[config(default = 0.10)]
    pub new_tab_highlight_alpha_base: f32,
    /// Hover/active alpha for the New Tab area highlight
    #[config(default = 0.45)]
    pub new_tab_highlight_alpha_hover: f32,

    // --- Snapping behavior near the tab bar ---
    /// Vertical snap margin in pixels to treat cursor as inside the tab bar band
    #[config(default = 6.0)]
    pub tab_drop_snap_px: f32,
    /// Horizontal extra margin in pixels near the right edge to make selecting "New Tab" easier
    #[config(default = 24.0)]
    pub new_tab_snap_extra_px: f32,
}

impl Default for DragConfig {
    fn default() -> Self {
        Self {
            enable_pane_drag: true,
            pane_drag_modifier: DragModifier::Alt,
            pane_drag_button: DragButton::Left,
            highlight_color: HighlightColorOpt::default(),
            highlight_min_alpha: 0.08,
            highlight_alpha_base: 0.15,
            highlight_alpha_hover: 0.5,
            tab_highlight_alpha_base: 0.12,
            tab_highlight_alpha_hover: 0.4,
            new_tab_highlight_alpha_base: 0.10,
            new_tab_highlight_alpha_hover: 0.45,
            tab_drop_snap_px: 6.0,
            new_tab_snap_extra_px: 24.0,
        }
    }
}

/// Wrapper for optional highlight color that logs key name on parse errors.
#[derive(Serialize, Debug, Clone, PartialEq, Default)]
pub struct HighlightColorOpt(#[serde(skip_serializing_if = "Option::is_none")] pub Option<Rgb>);

impl SerdeReplace for HighlightColorOpt {
    fn replace(&mut self, value: toml::Value) -> Result<(), Box<dyn std::error::Error>> {
        *self = HighlightColorOpt::deserialize(value)?;
        Ok(())
    }
}

impl HighlightColorOpt {
    pub fn get(&self) -> Option<Rgb> {
        self.0
    }
}

impl<'de> Deserialize<'de> for HighlightColorOpt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Accept hex string like "#aabbcc", 0x..., or rgb table { r=..., g=..., b=... }
        let value = toml::Value::deserialize(deserializer)?;
        // Try table form first
        if let Ok(rgb) = Rgb::deserialize(value.clone()) {
            return Ok(HighlightColorOpt(Some(rgb)));
        }
        // If value is explicitly empty string, treat as None
        if let toml::Value::String(s) = &value {
            if s.trim().is_empty() {
                return Ok(HighlightColorOpt(None));
            }
        }
        // If a string was provided but failed, log with key name and raw value
        if let toml::Value::String(s) = &value {
            log::error!(
                target: crate::config::LOG_TARGET_CONFIG,
                "Config error: workspace.drag.highlight_color: failed to parse color '{}'; expected hex like #ff00ff or table {{r,g,b}}; falling back to theme",
                s
            );
            return Ok(HighlightColorOpt(None));
        }
        // For other invalid types, log a generic diagnostic and fallback
        log::error!(
            target: crate::config::LOG_TARGET_CONFIG,
            "Config error: workspace.drag.highlight_color: invalid type {}; expected string or table; falling back to theme",
            value.type_str()
        );
        Ok(HighlightColorOpt(None))
    }
}

impl DragConfig {
    /// Clamp/validate numeric values and fix up invalid combinations.
    /// Logs warnings when values are adjusted.
    pub fn sanitize(&mut self) {
        let defaults = DragConfig::default();

        // Helper for clamping alphas to [0, 1]. If NaN/inf, reset to default.
        let clamp_alpha = |val: &mut f32, key: &str, default: f32| {
            if !val.is_finite() {
                warn!("Config workspace.drag.{key} is not a finite number; resetting to default {default}");
                *val = default;
                return;
            }
            if *val < 0.0 || *val > 1.0 {
                let old = *val;
                *val = val.clamp(0.0, 1.0);
                warn!(
                    "Config workspace.drag.{key} out of range: {old}; clamped to {new}",
                    new = *val
                );
            }
        };

        clamp_alpha(
            &mut self.highlight_min_alpha,
            "highlight_min_alpha",
            defaults.highlight_min_alpha,
        );
        clamp_alpha(
            &mut self.highlight_alpha_base,
            "highlight_alpha_base",
            defaults.highlight_alpha_base,
        );
        clamp_alpha(
            &mut self.highlight_alpha_hover,
            "highlight_alpha_hover",
            defaults.highlight_alpha_hover,
        );
        clamp_alpha(
            &mut self.tab_highlight_alpha_base,
            "tab_highlight_alpha_base",
            defaults.tab_highlight_alpha_base,
        );
        clamp_alpha(
            &mut self.tab_highlight_alpha_hover,
            "tab_highlight_alpha_hover",
            defaults.tab_highlight_alpha_hover,
        );
        clamp_alpha(
            &mut self.new_tab_highlight_alpha_base,
            "new_tab_highlight_alpha_base",
            defaults.new_tab_highlight_alpha_base,
        );
        clamp_alpha(
            &mut self.new_tab_highlight_alpha_hover,
            "new_tab_highlight_alpha_hover",
            defaults.new_tab_highlight_alpha_hover,
        );

        // Ensure hover >= base for each alpha pair.
        if self.highlight_alpha_hover < self.highlight_alpha_base {
            warn!(
                "Config workspace.drag.highlight_alpha_hover ({hover}) is less than base ({base}); fixing",
                hover = self.highlight_alpha_hover,
                base = self.highlight_alpha_base
            );
            self.highlight_alpha_hover = self.highlight_alpha_base;
        }
        if self.tab_highlight_alpha_hover < self.tab_highlight_alpha_base {
            warn!(
                "Config workspace.drag.tab_highlight_alpha_hover ({hover}) is less than base ({base}); fixing",
                hover = self.tab_highlight_alpha_hover,
                base = self.tab_highlight_alpha_base
            );
            self.tab_highlight_alpha_hover = self.tab_highlight_alpha_base;
        }
        if self.new_tab_highlight_alpha_hover < self.new_tab_highlight_alpha_base {
            warn!(
                "Config workspace.drag.new_tab_highlight_alpha_hover ({hover}) is less than base ({base}); fixing",
                hover = self.new_tab_highlight_alpha_hover,
                base = self.new_tab_highlight_alpha_base
            );
            self.new_tab_highlight_alpha_hover = self.new_tab_highlight_alpha_base;
        }

        // Non-negative snap margins; reset NaN/inf to defaults.
        let sanitize_snap = |val: &mut f32, key: &str, default: f32| {
            if !val.is_finite() {
                warn!("Config workspace.drag.{key} is not a finite number; resetting to default {default}");
                *val = default;
                return;
            }
            if *val < 0.0 {
                warn!(
                    "Config workspace.drag.{key} is negative ({old}); clamping to 0.0",
                    old = *val
                );
                *val = 0.0;
            }
        };
        sanitize_snap(
            &mut self.tab_drop_snap_px,
            "tab_drop_snap_px",
            defaults.tab_drop_snap_px,
        );
        sanitize_snap(
            &mut self.new_tab_snap_extra_px,
            "new_tab_snap_extra_px",
            defaults.new_tab_snap_extra_px,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_bar_config_parses_and_defaults() {
        // Minimal config: rely on defaults
        let ui = crate::config::ui_config::UiConfig::default();
        let tb = &ui.workspace.tab_bar;
        assert!(tb.show);
        assert_eq!(tb.position, TabBarPosition::Top);
        assert_eq!(tb.visibility, TabBarVisibility::Auto);
        assert!(tb.show_close_button);
        assert!(!tb.close_button_on_hover);
        assert!(tb.show_modified_indicator);
        assert!(tb.show_new_tab_button);
        assert!(!tb.show_tab_numbers);
        assert_eq!(tb.max_title_length, 20);
    }

    #[test]
    fn tab_bar_config_from_toml_values() {
        let toml_str = r#"
            [workspace]
            warp_style = true
            [workspace.tab_bar]
            position = "Bottom"
            show = true
            visibility = "Hover"
            show_close_button = false
            close_button_on_hover = true
            show_modified_indicator = false
            show_new_tab_button = false
            show_tab_numbers = true
            max_title_length = 12
        "#;
        let value: toml::Value = toml::from_str(toml_str).expect("toml parse");
        let mut ui: crate::config::ui_config::UiConfig =
            crate::config::ui_config::UiConfig::deserialize(value).expect("ui config deser");
        // After loading hook usually resolves theme; not needed for this unit test
        let tb = &mut ui.workspace.tab_bar;
        assert_eq!(tb.position, TabBarPosition::Bottom);
        assert!(tb.show);
        assert_eq!(tb.visibility, TabBarVisibility::Hover);
        assert!(!tb.show_close_button);
        assert!(tb.close_button_on_hover);
        assert!(!tb.show_modified_indicator);
        assert!(!tb.show_new_tab_button);
        assert!(tb.show_tab_numbers);
        assert_eq!(tb.max_title_length, 12);
    }

    #[test]
    fn drag_sanitize_clamps_and_orders_alphas_and_snaps() {
        let mut drag = DragConfig {
            enable_pane_drag: true,
            pane_drag_modifier: DragModifier::Alt,
            pane_drag_button: DragButton::Left,
            highlight_color: HighlightColorOpt(None),
            highlight_min_alpha: 0.08,
            highlight_alpha_base: 2.5,   // out of range
            highlight_alpha_hover: -0.1, // out of range and < base
            tab_highlight_alpha_base: 0.8,
            tab_highlight_alpha_hover: 0.2,     // < base
            new_tab_highlight_alpha_base: -5.0, // out of range
            new_tab_highlight_alpha_hover: 5.0, // out of range
            tab_drop_snap_px: -10.0,            // negative
            new_tab_snap_extra_px: -1.0,        // negative
        };
        drag.sanitize();

        assert!(drag.highlight_alpha_base >= 0.0 && drag.highlight_alpha_base <= 1.0);
        assert!(drag.highlight_alpha_hover >= 0.0 && drag.highlight_alpha_hover <= 1.0);
        assert!(drag.tab_highlight_alpha_base >= 0.0 && drag.tab_highlight_alpha_base <= 1.0);
        assert!(drag.tab_highlight_alpha_hover >= 0.0 && drag.tab_highlight_alpha_hover <= 1.0);
        assert!(
            drag.new_tab_highlight_alpha_base >= 0.0 && drag.new_tab_highlight_alpha_base <= 1.0
        );
        assert!(
            drag.new_tab_highlight_alpha_hover >= 0.0 && drag.new_tab_highlight_alpha_hover <= 1.0
        );

        assert!(drag.highlight_alpha_hover >= drag.highlight_alpha_base);
        assert!(drag.tab_highlight_alpha_hover >= drag.tab_highlight_alpha_base);
        assert!(drag.new_tab_highlight_alpha_hover >= drag.new_tab_highlight_alpha_base);

        assert!(drag.tab_drop_snap_px >= 0.0);
        assert!(drag.new_tab_snap_extra_px >= 0.0);
    }

    #[test]
    fn highlight_min_alpha_default_and_clamp() {
        let mut cfg = DragConfig::default();
        // default
        assert!((cfg.highlight_min_alpha - 0.08).abs() < 1e-6);
        // clamp on sanitize
        cfg.highlight_min_alpha = -1.0;
        cfg.sanitize();
        assert!(cfg.highlight_min_alpha >= 0.0 && cfg.highlight_min_alpha <= 1.0);
    }

    #[test]
    fn drag_defaults_unset_are_defaults() {
        let mut cfg = DragConfig::default();
        cfg.sanitize();
        let def = DragConfig::default();
        assert_eq!(cfg.enable_pane_drag, def.enable_pane_drag);
        assert_eq!(cfg.pane_drag_modifier as u8, def.pane_drag_modifier as u8);
        assert_eq!(cfg.pane_drag_button as u8, def.pane_drag_button as u8);
        assert_eq!(cfg.highlight_color.get(), def.highlight_color.get());
        assert_eq!(cfg.highlight_alpha_base, def.highlight_alpha_base);
        assert_eq!(cfg.highlight_alpha_hover, def.highlight_alpha_hover);
        assert_eq!(cfg.tab_highlight_alpha_base, def.tab_highlight_alpha_base);
        assert_eq!(cfg.tab_highlight_alpha_hover, def.tab_highlight_alpha_hover);
        assert_eq!(
            cfg.new_tab_highlight_alpha_base,
            def.new_tab_highlight_alpha_base
        );
        assert_eq!(
            cfg.new_tab_highlight_alpha_hover,
            def.new_tab_highlight_alpha_hover
        );
        assert_eq!(cfg.tab_drop_snap_px, def.tab_drop_snap_px);
        assert_eq!(cfg.new_tab_snap_extra_px, def.new_tab_snap_extra_px);
    }

    #[test]
    fn highlight_color_invalid_falls_back_to_none() {
        // Emulate deserialization by directly calling the deserializer
        let val = toml::Value::String("not_a_color".to_string());
        let parsed: HighlightColorOpt = HighlightColorOpt::deserialize(val).unwrap();
        assert!(parsed.get().is_none());
    }
}

#[derive(ConfigDeserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DragModifier {
    None,
    #[default]
    Alt,
    Ctrl,
    Shift,
}

#[derive(ConfigDeserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DragButton {
    #[default]
    Left,
    Middle,
    Right,
}

/// Session persistence configuration
#[derive(ConfigDeserialize, Serialize, Debug, Clone, PartialEq)]
pub struct SessionConfig {
    /// Enable session persistence features
    #[config(default = true)]
    pub enabled: bool,
    /// Restore previous session on startup
    #[config(default = true)]
    pub restore_on_startup: bool,
    /// Autosave interval in seconds (0 disables autosave)
    #[config(default = 30)]
    pub autosave_interval_secs: u64,
    /// Optional explicit session file path
    #[serde(default)]
    pub file_path: Option<PathBuf>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            restore_on_startup: true,
            autosave_interval_secs: 30,
            file_path: None,
        }
    }
}

/// Tab bar position
#[derive(ConfigDeserialize, Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum TabBarPosition {
    #[default]
    Top,
    Bottom,
    Hidden,
}

/// Action when creating a new tab
#[allow(clippy::enum_variant_names)]
#[derive(ConfigDeserialize, Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum NewTabAction {
    /// Inherit the working directory from the current tab
    #[default]
    InheritWorkingDir,
    /// Start in the home directory
    HomeDir,
    /// Use the last used directory
    LastUsedDir,
    /// Use a specific directory (configured separately)
    CustomDir,
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

/// Warp-style keybindings configuration (toggle)
#[derive(ConfigDeserialize, Serialize, Debug, Clone, PartialEq)]
pub struct WarpStyleBindingsConfig {
    /// Enable integrating Warp-like keybindings during config load
    #[config(default = true)]
    pub enable: bool,
}

impl Default for WarpStyleBindingsConfig {
    fn default() -> Self {
        Self { enable: true }
    }
}
