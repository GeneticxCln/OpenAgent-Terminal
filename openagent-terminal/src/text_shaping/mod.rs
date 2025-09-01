// Text shaping module
#[cfg(feature = "harfbuzz")]
pub mod harfbuzz;

#[cfg(feature = "harfbuzz")]
pub use harfbuzz::{HarfBuzzShaper, ShapingConfig, ShapedText, ShapedGlyph, TextDirection};
