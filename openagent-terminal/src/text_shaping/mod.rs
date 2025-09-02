// Text shaping module
#[cfg(feature = "harfbuzz")]
pub mod harfbuzz;

#[cfg(feature = "harfbuzz")]
#[allow(unused_imports)]
pub use harfbuzz::{HarfBuzzShaper, ShapedGlyph, ShapedText, ShapingConfig, TextDirection};
