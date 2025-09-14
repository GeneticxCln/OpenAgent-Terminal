use log::LevelFilter;
use serde::Serialize;

use openagent_terminal_config_derive::ConfigDeserialize;

/// Eviction policy for WGPU multipage glyph atlas.
#[derive(
    ConfigDeserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default,
)]
pub enum AtlasEvictionPolicy {
    /// Rotate through pages regardless of usage.
    RoundRobin,
    /// Choose least-recently-used page; break ties by smallest occupancy.
    #[default]
    LruMinOccupancy,
}

/// Preference for enabling subpixel text rendering.
#[derive(
    ConfigDeserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default,
)]
pub enum SubpixelPreference {
    #[default]
    Auto,
    Enabled,
    Disabled,
}

/// Orientation for LCD subpixel rendering.
#[derive(
    ConfigDeserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default,
)]
#[allow(clippy::upper_case_acronyms)]
pub enum SubpixelOrientation {
    #[default]
    RGB,
    BGR,
}

/// Preference for using an sRGB swapchain/surface when available.
#[derive(
    ConfigDeserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default,
)]
pub enum SrgbPreference {
    #[default]
    Auto,
    Enabled,
    Disabled,
}

/// Render timer color style.
#[derive(
    ConfigDeserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default,
)]
pub enum RenderTimerStyle {
    /// Subtle, unobtrusive background using surface_muted with text color.
    #[default]
    LowContrast,
    /// Attention-grabbing highlight using warning background.
    Warning,
}

/// Debugging options.
#[derive(ConfigDeserialize, Serialize, Copy, Clone, Debug, PartialEq)]
pub struct Debug {
    pub log_level: LevelFilter,

    pub print_events: bool,

    /// Keep the log file after quitting.
    pub persistent_logging: bool,

    /// Should show render timer.
    pub render_timer: bool,

    /// Render timer color style.
    pub render_timer_style: RenderTimerStyle,

    /// Highlight damage information produced by OpenAgent Terminal.
    pub highlight_damage: bool,

    /// The renderer OpenAgent Terminal should be using.
    pub renderer: Option<RendererPreference>,

    /// Always use WGPU backend. Set to false to force an error if WGPU cannot initialize.
    /// This is the default and recommended configuration for simplified rendering.
    pub prefer_wgpu: bool,

    /// Preview: Enable plugin system (WASI sandbox) integration.
    /// This is gated behind this preview flag even when the `plugins` cargo feature is enabled.
    /// Default is false; set to true to turn on plugin loading.
    pub plugins_preview: bool,

    /// Use EGL as display API if the current platform allows it.
    pub prefer_egl: bool,

    /// Enable command block overlays/folding UI.
    pub blocks: bool,

    /// Subpixel text rendering toggle (Auto/Enabled/Disabled).
    pub subpixel_text: SubpixelPreference,

    /// LCD subpixel orientation used when subpixel text is enabled.
    pub subpixel_orientation: SubpixelOrientation,

    /// Subpixel gamma exponent (typical ~2.2).
    pub subpixel_gamma: f32,

    /// Enable HarfBuzz text shaping in WGPU renderer (experimental).
    pub text_shaping: bool,

    /// sRGB swapchain preference (Auto/Enabled/Disabled).
    pub srgb_swapchain: SrgbPreference,

    /// Optional: Zero the evicted GPU atlas layer before reuse (cosmetic).
    pub zero_evicted_atlas_layer: bool,

    /// WGPU atlas eviction policy.
    pub atlas_eviction_policy: AtlasEvictionPolicy,

    /// Periodic atlas stats reporting interval in frames (0 disables reporting).
    pub atlas_report_interval_frames: u32,

    /// Periodic renderer performance reporting interval in frames (0 disables reporting).
    pub renderer_report_interval_frames: u32,

    /// Toggle on-screen performance HUD overlay (frame time, draw calls).
    pub renderer_perf_hud: bool,

    /// Record ref test.
    #[config(skip)]
    #[serde(skip_serializing)]
    pub ref_test: bool,

    /// Use theme tokens to style the block cursor instead of terminal colors.
    pub theme_block_cursor: bool,

    /// Use theme tokens to style beam/underline cursors instead of terminal colors.
    pub theme_text_cursors: bool,

    /// Use theme tokens to style selection highlight instead of terminal colors.
    pub theme_selection: bool,

    /// Enable always-on completions overlay (experimental; gated by cargo feature `completions`).
    #[cfg(feature = "completions")]
    pub completions: bool,
}

impl Default for Debug {
    fn default() -> Self {
        Self {
            log_level: LevelFilter::Warn,
            print_events: Default::default(),
            persistent_logging: Default::default(),
            render_timer: Default::default(),
            render_timer_style: Default::default(),
            highlight_damage: Default::default(),
            ref_test: Default::default(),
            renderer: Default::default(),
            // Default to WGPU preferred (the app will attempt WGPU first when compiled with
            // `wgpu`).
            prefer_wgpu: true,
            plugins_preview: false,
            prefer_egl: Default::default(),
            blocks: true,
            subpixel_text: Default::default(),
            subpixel_orientation: Default::default(),
            subpixel_gamma: 2.2,
            text_shaping: false,
            srgb_swapchain: Default::default(),
            zero_evicted_atlas_layer: false,
            atlas_eviction_policy: Default::default(),
            atlas_report_interval_frames: 0,
            renderer_report_interval_frames: 0,
            renderer_perf_hud: false,
            theme_block_cursor: false,
            theme_text_cursors: false,
            theme_selection: false,
            #[cfg(feature = "completions")]
            completions: false,
        }
    }
}

/// The renderer configuration options.
/// WGPU is the only supported backend for simplified architecture.
#[derive(ConfigDeserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RendererPreference {
    /// WGPU-only; legacy options removed.
    WgpuOnly,
}
