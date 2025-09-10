// WGPU-only renderer module
use std::fmt;

pub mod rects;
pub mod text;
pub mod ui;
#[cfg(feature = "wgpu")]
pub mod wgpu;

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
