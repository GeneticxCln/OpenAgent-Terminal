pub mod glyph_cache;
#[cfg(all(feature = "harfbuzz", feature = "wgpu"))]
pub mod shaped_renderer;

// Built-in box-drawing and powerline glyphs used by the glyph cache.
mod builtin_font;

// Import only what's needed locally to avoid unused import warnings.
use glyph_cache::{Glyph, LoadGlyph};

use crossfont::RasterizedGlyph;

#[derive(Debug)]
pub struct LoaderApi<'a> {
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> LoaderApi<'a> {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a> Default for LoaderApi<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadGlyph for LoaderApi<'_> {
    fn load_glyph(&mut self, _rasterized: &RasterizedGlyph) -> Glyph {
        // Return an empty glyph; WGPU path does not use this dummy loader for actual uploads.
        Glyph {
            tex_id: 0,
            multicolor: false,
            top: 0,
            left: 0,
            width: 0,
            height: 0,
            uv_bot: 0.0,
            uv_left: 0.0,
            uv_width: 0.0,
            uv_height: 0.0,
        }
    }

    fn clear(&mut self) {}
}

/// Execute a closure with a temporary LoaderApi for preloading glyphs without GL state.
#[allow(dead_code)]
pub fn with_dummy_loader<T, F: FnOnce(LoaderApi<'_>) -> T>(func: F) -> T {
    let loader = LoaderApi::new();
    // Use a copy since the trait methods take &mut self.
    let loader_mut = loader;
    func(loader_mut)
}
