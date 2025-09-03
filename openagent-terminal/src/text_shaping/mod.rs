// Text shaping module
#[cfg(feature = "harfbuzz")]
pub mod harfbuzz;

#[cfg(feature = "harfbuzz")]
pub mod integration;

#[cfg(feature = "harfbuzz")]
#[allow(unused_imports)]
pub use harfbuzz::{HarfBuzzShaper, ShapedGlyph, ShapedText, ShapingConfig, TextDirection};

#[cfg(feature = "harfbuzz")]
#[allow(unused_imports)]
pub use integration::{
    IntegratedTextShaper, ShapedCell, ShapedCellGlyph, ShapedLine, ShapedTextRenderer,
    ShapingIntegrationConfig,
};
