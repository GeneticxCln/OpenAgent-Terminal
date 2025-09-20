use std::fmt;

use crossfont::Size as FontSize;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use openagent_terminal_config_derive::{ConfigDeserialize, SerdeReplace};

use crate::config::ui_config::Delta;

/// Font config.
///
/// Defaults are provided at the level of this struct per platform, but not per
/// field in this struct. It might be nice in the future to have defaults for
/// each value independently. Alternatively, maybe erroring when the user
/// doesn't provide complete config is Ok.
#[derive(ConfigDeserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Font {
    /// Extra spacing per character.
    pub offset: Delta<i8>,

    /// Glyph offset within character cell.
    pub glyph_offset: Delta<i8>,

    #[config(removed = "set the AppleFontSmoothing user default instead")]
    pub use_thin_strokes: bool,

    /// Normal font face.
    normal: FontDescription,

    /// Bold font face.
    bold: SecondaryFontDescription,

    /// Italic font face.
    italic: SecondaryFontDescription,

    /// Bold italic font face.
    bold_italic: SecondaryFontDescription,

    /// Font size in points.
    size: Size,

    /// Whether to use the built-in font for box drawing characters.
    pub builtin_box_drawing: bool,

    /// Text shaping configuration.
    pub text_shaping: TextShapingConfig,
}

impl Font {
    /// Get a font clone with a size modification.
    pub fn with_size(self, size: FontSize) -> Font {
        Font { size: Size(size), ..self }
    }

    #[inline]
    pub fn size(&self) -> FontSize {
        self.size.0
    }

    /// Get normal font description.
    pub fn normal(&self) -> &FontDescription {
        &self.normal
    }

    /// Get bold font description.
    pub fn bold(&self) -> FontDescription {
        self.bold.desc(&self.normal)
    }

    /// Get italic font description.
    pub fn italic(&self) -> FontDescription {
        self.italic.desc(&self.normal)
    }

    /// Get bold italic font description.
    pub fn bold_italic(&self) -> FontDescription {
        self.bold_italic.desc(&self.normal)
    }
}

impl Default for Font {
    fn default() -> Font {
        Self {
            builtin_box_drawing: true,
            glyph_offset: Default::default(),
            use_thin_strokes: Default::default(),
            bold_italic: Default::default(),
            italic: Default::default(),
            offset: Default::default(),
            normal: Default::default(),
            bold: Default::default(),
            size: Default::default(),
            text_shaping: Default::default(),
        }
    }
}

/// Description of the normal font.
#[derive(ConfigDeserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct FontDescription {
    pub family: String,
    pub style: Option<String>,
}

impl Default for FontDescription {
    fn default() -> FontDescription {
        FontDescription {
            #[cfg(not(any(target_os = "macos", windows)))]
            family: "monospace".into(),
            #[cfg(target_os = "macos")]
            family: "Menlo".into(),
            #[cfg(windows)]
            family: "Consolas".into(),
            style: None,
        }
    }
}

/// Description of the italic and bold font.
#[derive(ConfigDeserialize, Serialize, Debug, Default, Clone, PartialEq, Eq)]
pub struct SecondaryFontDescription {
    family: Option<String>,
    style: Option<String>,
}

impl SecondaryFontDescription {
    pub fn desc(&self, fallback: &FontDescription) -> FontDescription {
        FontDescription {
            family: self.family.clone().unwrap_or_else(|| fallback.family.clone()),
            style: self.style.clone(),
        }
    }
}

#[derive(SerdeReplace, Debug, Clone, PartialEq, Eq)]
struct Size(FontSize);

impl Default for Size {
    fn default() -> Self {
        Self(FontSize::new(11.25))
    }
}

impl<'de> Deserialize<'de> for Size {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct NumVisitor;
        impl Visitor<'_> for NumVisitor {
            type Value = Size;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("f64 or i64")
            }

            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                Ok(Size(FontSize::new(value as f32)))
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(Size(FontSize::new(value as f32)))
            }
        }

        deserializer.deserialize_any(NumVisitor)
    }
}

impl Serialize for Size {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f32(self.0.as_pt())
    }
}

/// Text shaping configuration.
#[derive(ConfigDeserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct TextShapingConfig {
    /// Enable programming ligatures (e.g., ->, =>, !=, <=).
    pub enable_ligatures: bool,

    /// Enable kerning adjustments between characters.
    pub enable_kerning: bool,

    /// Enable contextual alternates for better typography.
    pub enable_contextual_alternates: bool,

    /// Enable complex script shaping (Arabic, Hebrew, Devanagari, etc.).
    pub enable_complex_scripts: bool,

    /// Enable bidirectional text support.
    pub enable_bidi: bool,

    /// Cache shaped text lines to improve performance.
    pub cache_shaped_lines: bool,

    /// Maximum number of cached shaped lines.
    pub max_cached_lines: usize,

    /// Stylistic sets to enable (SS01, SS02, etc.).
    pub stylistic_sets: Vec<u32>,

    /// Fallback fonts for complex scripts and emojis.
    pub fallback_fonts: Vec<String>,

    /// Emoji font to use for emoji rendering.
    pub emoji_font: Option<String>,

    /// Fall back to basic rendering if HarfBuzz fails.
    pub fallback_to_basic_rendering: bool,

    /// Default language for text shaping hints.
    pub default_language: String,
}

impl Default for TextShapingConfig {
    fn default() -> Self {
        Self {
            enable_ligatures: true,
            enable_kerning: true,
            enable_contextual_alternates: true,
            enable_complex_scripts: true,
            enable_bidi: true,
            cache_shaped_lines: true,
            max_cached_lines: 500,
            stylistic_sets: vec![],
            fallback_fonts: vec![
                "Noto Sans".to_string(),
                "DejaVu Sans".to_string(),
                "Liberation Sans".to_string(),
                "Noto Sans CJK".to_string(),
                "Noto Sans Arabic".to_string(),
                "Noto Sans Hebrew".to_string(),
            ],
            emoji_font: Some("Noto Color Emoji".to_string()),
            fallback_to_basic_rendering: true,
            default_language: "en".to_string(),
        }
    }
}
