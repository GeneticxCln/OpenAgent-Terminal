// Unified renderer module: WGPU backend plus GL stubs for build compatibility
use std::fmt;

pub mod rects;
pub mod text;
pub mod ui;
#[cfg(feature = "wgpu")]
pub mod wgpu;

#[cfg(feature = "gl-backend")]
pub mod platform;

pub use text::glyph_cache::{GlyphCache, LoadGlyph};
pub use text::LoaderApi;

#[derive(Debug)]
pub enum Error {
    Other(String),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Other(err) => write!(f, "{err}"),
        }
    }
}

/// Minimal stub for the legacy GL renderer API so non-GL builds still compile.
/// These methods are no-ops and exist only to satisfy references from the GL code path,
/// which is not used when running with the WGPU backend.
#[allow(dead_code)]
pub struct Renderer;

#[allow(dead_code)]
impl Renderer {
    pub fn new<T>(
        _context: &T,
        _preference: crate::config::debug::RendererPreference,
    ) -> Result<Self, Error> {
        Ok(Self)
    }

    pub fn with_loader<F, T>(&mut self, func: F) -> T
    where
        F: FnOnce(LoaderApi<'_>) -> T,
    {
        func(LoaderApi::new())
    }

    pub fn draw_cells<I>(
        &mut self,
        _size_info: &crate::display::SizeInfo,
        _glyph_cache: &mut GlyphCache,
        _cells: I,
    ) where
        I: Iterator<Item = crate::display::content::RenderableCell>,
    {
    }

    pub fn draw_string<I>(
        &mut self,
        _point: openagent_terminal_core::index::Point<usize>,
        _fg: crate::display::color::Rgb,
        _bg: crate::display::color::Rgb,
        _string_chars: I,
        _size_info: &crate::display::SizeInfo,
        _glyph_cache: &mut GlyphCache,
    ) where
        I: Iterator<Item = char>,
    {
    }

    pub fn draw_rects(
        &mut self,
        _size_info: &crate::display::SizeInfo,
        _metrics: &crossfont::Metrics,
        _rects: Vec<rects::RenderRect>,
    ) {
    }

    pub fn stage_ui_rounded_rect(&mut self, _rect: ui::UiRoundedRect) {}
    pub fn stage_ui_sprite(&mut self, _sprite: ui::UiSprite) {}

    pub fn set_sprite_filter_nearest(&mut self, _nearest: bool) {}

    pub fn clear(&self, _color: crate::display::color::Rgb, _alpha: f32) {}

    pub fn was_context_reset(&self) -> bool {
        false
    }
    pub fn finish(&self) {}

    pub fn set_viewport(&self, _size: &crate::display::SizeInfo) {}

    pub fn resize(&self, _size_info: &crate::display::SizeInfo) {}
}
