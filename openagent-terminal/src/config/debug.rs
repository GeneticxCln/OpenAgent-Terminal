use log::LevelFilter;
use serde::Serialize;

use openagent_terminal_config_derive::ConfigDeserialize;

/// Eviction policy for WGPU multipage glyph atlas.
#[derive(ConfigDeserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AtlasEvictionPolicy {
    /// Rotate through pages regardless of usage.
    RoundRobin,
    /// Choose least-recently-used page; break ties by smallest occupancy.
    LruMinOccupancy,
}

impl Default for AtlasEvictionPolicy {
    fn default() -> Self {
        Self::LruMinOccupancy
    }
}

/// Preference for enabling subpixel text rendering.
#[derive(ConfigDeserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SubpixelPreference {
    Auto,
    Enabled,
    Disabled,
}

impl Default for SubpixelPreference {
    fn default() -> Self {
        Self::Auto
    }
}

/// Preference for using an sRGB swapchain/surface when available.
#[derive(ConfigDeserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SrgbPreference {
    Auto,
    Enabled,
    Disabled,
}

impl Default for SrgbPreference {
    fn default() -> Self {
        Self::Auto
    }
}

/// Render timer color style.
#[derive(ConfigDeserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RenderTimerStyle {
    /// Subtle, unobtrusive background using surface_muted with text color.
    LowContrast,
    /// Attention-grabbing highlight using warning background.
    Warning,
}

impl Default for RenderTimerStyle {
    fn default() -> Self {
        Self::LowContrast
    }
}

/// Debugging options.
#[derive(ConfigDeserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
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

    /// [EXPERIMENTAL] Prefer the WGPU backend when available (falls back to OpenGL if
    /// unavailable). WARNING: WGPU backend is experimental and may have visual
    /// inconsistencies. OpenGL is the stable, recommended renderer.
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

    /// sRGB swapchain preference (Auto/Enabled/Disabled).
    pub srgb_swapchain: SrgbPreference,

    /// Optional: Zero the evicted GPU atlas layer before reuse (cosmetic).
    pub zero_evicted_atlas_layer: bool,

    /// WGPU atlas eviction policy.
    pub atlas_eviction_policy: AtlasEvictionPolicy,

    /// Periodic atlas stats reporting interval in frames (0 disables reporting).
    pub atlas_report_interval_frames: u32,

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
            prefer_wgpu: Default::default(),
            plugins_preview: false,
            prefer_egl: Default::default(),
            blocks: true,
            subpixel_text: Default::default(),
            srgb_swapchain: Default::default(),
            zero_evicted_atlas_layer: false,
            atlas_eviction_policy: Default::default(),
            atlas_report_interval_frames: 0,
            theme_block_cursor: false,
            theme_text_cursors: false,
            theme_selection: false,
            #[cfg(feature = "completions")]
            completions: false,
        }
    }
}

/// The renderer configuration options.
/// OpenGL variants are stable and recommended for production use.
/// WGPU backend (if enabled via feature flag) is experimental.
#[derive(ConfigDeserialize, Serialize, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RendererPreference {
    /// OpenGL 3.3 renderer (RECOMMENDED - stable, well-tested).
    Glsl3,

    /// GLES 2 renderer, with optional extensions like dual source blending (stable).
    Gles2,

    /// Pure GLES 2 renderer (stable, maximum compatibility).
    Gles2Pure,
}
