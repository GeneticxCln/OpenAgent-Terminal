// Text Shaping Module - HarfBuzz integration for advanced text rendering

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::path::Path;
use harfbuzz_rs::{Face, Font, Owned, UnicodeBuffer, GlyphInfo, GlyphPosition};
use fontdb::{Database, Family, Query, Source};
use ttf_parser;
use unicode_bidi::{BidiInfo, BidiClass};
use unicode_segmentation::UnicodeSegmentation;
use lru::LruCache;
use std::num::NonZeroUsize;

pub mod emoji;
pub mod cache;
pub mod fallback;

use emoji::EmojiDatabase;
use cache::ShapingCache;
use fallback::FontFallbackChain;

/// Configuration for text shaping
#[derive(Debug, Clone)]
pub struct ShapingConfig {
    pub primary_font: String,
    pub fallback_fonts: Vec<String>,
    pub emoji_font: String,
    pub enable_ligatures: bool,
    pub enable_kerning: bool,
    pub cache_size: usize,
    pub rtl_support: bool,
    pub complex_script_support: bool,
}

impl Default for ShapingConfig {
    fn default() -> Self {
        Self {
            primary_font: "JetBrains Mono".to_string(),
            fallback_fonts: vec![
                "Noto Sans".to_string(),
                "DejaVu Sans".to_string(),
                "Liberation Sans".to_string(),
            ],
            emoji_font: "Noto Color Emoji".to_string(),
            enable_ligatures: true,
            enable_kerning: true,
            cache_size: 1000,
            rtl_support: true,
            complex_script_support: true,
        }
    }
}

/// Shaped text result
#[derive(Debug, Clone)]
pub struct ShapedText {
    pub glyphs: Vec<ShapedGlyph>,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
    pub direction: TextDirection,
}

/// Individual shaped glyph
#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    pub glyph_id: u32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub x_advance: f32,
    pub y_advance: f32,
    pub cluster: u32,
    pub font_index: usize,
    pub is_emoji: bool,
    pub color: Option<(u8, u8, u8, u8)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
    Mixed,
}

/// Script detection for complex text shaping
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Script {
    Latin,
    Arabic,
    Hebrew,
    Devanagari,
    Thai,
    Chinese,
    Japanese,
    Korean,
    Emoji,
    Unknown,
}

/// Main text shaper with HarfBuzz backend
pub struct TextShaper {
    config: ShapingConfig,
    font_db: Database,
    fonts: Vec<Arc<Font<'static>>>,
    fallback_chain: FontFallbackChain,
    emoji_db: EmojiDatabase,
    cache: Arc<RwLock<ShapingCache>>,
    hb_faces: HashMap<usize, Face<'static>>,
}

impl TextShaper {
    /// Create a new text shaper with configuration
    pub fn new(config: ShapingConfig) -> Result<Self, ShapingError> {
        let mut font_db = Database::new();

        // Load system fonts
        font_db.load_system_fonts();

        // Load custom font directories
        if let Ok(home) = std::env::var("HOME") {
            let custom_fonts = Path::new(&home).join(".fonts");
            if custom_fonts.exists() {
                font_db.load_fonts_dir(&custom_fonts);
            }
        }

        // Initialize font collection
        let mut fonts = Vec::new();
        let mut hb_faces = HashMap::new();

        // Load primary font
        let primary_id = font_db.query(&Query {
            families: &[Family::Name(&config.primary_font)],
            ..Default::default()
        }).ok_or(ShapingError::FontNotFound(config.primary_font.clone()))?;

        if let Some(face_data) = font_db.face_source(primary_id) {
            if let Source::Binary(data) = face_data {
                let face = Face::from_bytes(data.as_ref().as_ref())
                    .map_err(|_| ShapingError::InvalidFont)?;
                let font = Font::new(face.clone());
                fonts.push(Arc::new(font));
                hb_faces.insert(0, face);
            }
        }

        // Load fallback fonts
        for fallback_name in &config.fallback_fonts {
            if let Some(fallback_id) = font_db.query(&Query {
                families: &[Family::Name(fallback_name)],
                ..Default::default()
            }) {
                if let Some(face_data) = font_db.face_source(fallback_id) {
                    if let Source::Binary(data) = face_data {
                        if let Ok(face) = Face::from_bytes(data.as_ref().as_ref()) {
                            let font = Font::new(face.clone());
                            let idx = fonts.len();
                            fonts.push(Arc::new(font));
                            hb_faces.insert(idx, face);
                        }
                    }
                }
            }
        }

        // Initialize components
        let fallback_chain = FontFallbackChain::new(&fonts);
        let emoji_db = EmojiDatabase::new(&config.emoji_font)?;
        let cache = Arc::new(RwLock::new(ShapingCache::new(config.cache_size)));

        Ok(Self {
            config,
            font_db,
            fonts,
            fallback_chain,
            emoji_db,
            cache,
            hb_faces,
        })
    }

    /// Shape text with full Unicode support
    pub fn shape_text(&self, text: &str, font_size: f32) -> Result<ShapedText, ShapingError> {
        // Check cache first
        let cache_key = format!("{}-{}", text, font_size);
        if let Some(cached) = self.cache.read().unwrap().get(&cache_key) {
            return Ok(cached.clone());
        }

        // Detect script and direction
        let script = self.detect_script(text);
        let direction = self.detect_direction(text);

        // Segment text by script and direction for complex layout
        let segments = self.segment_text(text, script, direction);
        let mut all_glyphs = Vec::new();
        let mut total_width = 0.0;
        let mut max_height = 0.0;
        let mut baseline = 0.0;

        for segment in segments {
            let shaped_segment = self.shape_segment(&segment, font_size)?;

            // Adjust positions for accumulated width
            for mut glyph in shaped_segment.glyphs {
                glyph.x_offset += total_width;
                all_glyphs.push(glyph);
            }

            total_width += shaped_segment.width;
            max_height = max_height.max(shaped_segment.height);
            baseline = baseline.max(shaped_segment.baseline);
        }

        // Handle bidirectional text reordering if needed
        if direction == TextDirection::Mixed || direction == TextDirection::RightToLeft {
            all_glyphs = self.reorder_bidi_glyphs(all_glyphs, text);
        }

        let result = ShapedText {
            glyphs: all_glyphs,
            width: total_width,
            height: max_height,
            baseline,
            direction,
        };

        // Cache the result
        self.cache.write().unwrap().put(cache_key, result.clone());

        Ok(result)
    }

    /// Shape a text segment with a specific script
    fn shape_segment(&self, segment: &TextSegment, font_size: f32) -> Result<ShapedText, ShapingError> {
        let font_index = self.select_font_for_segment(segment);
        let font = &self.fonts[font_index];

        // Create HarfBuzz buffer
        let mut buffer = UnicodeBuffer::new();
        buffer.add_str(&segment.text);

        // Set script and direction
        buffer.set_script(self.script_to_hb_script(segment.script));
        buffer.set_direction(match segment.direction {
            TextDirection::LeftToRight => harfbuzz_rs::Direction::Ltr,
            TextDirection::RightToLeft => harfbuzz_rs::Direction::Rtl,
            TextDirection::Mixed => harfbuzz_rs::Direction::Ltr,
        });

        // Set language if detected
        if let Some(lang) = self.detect_language(segment.script) {
            buffer.set_language(harfbuzz_rs::Language::from_string(&lang));
        }

        // Enable features based on configuration
        let mut features = Vec::new();
        if self.config.enable_ligatures {
            features.push(harfbuzz_rs::Feature::new(b"liga", 1, 0..));
            features.push(harfbuzz_rs::Feature::new(b"clig", 1, 0..));

            // Programming ligatures
            features.push(harfbuzz_rs::Feature::new(b"calt", 1, 0..));
        }
        if self.config.enable_kerning {
            features.push(harfbuzz_rs::Feature::new(b"kern", 1, 0..));
        }

        // Shape the text
        let output = harfbuzz_rs::shape(font, buffer, &features);

        // Convert HarfBuzz output to our format
        let positions = output.get_glyph_positions();
        let infos = output.get_glyph_infos();

        let mut glyphs = Vec::new();
        let mut current_x = 0.0;

        for (info, pos) in infos.iter().zip(positions.iter()) {
            let is_emoji = segment.script == Script::Emoji ||
                           self.emoji_db.is_emoji_codepoint(info.codepoint);

            let glyph = ShapedGlyph {
                glyph_id: info.codepoint,
                x_offset: current_x + (pos.x_offset as f32 * font_size / 1000.0),
                y_offset: pos.y_offset as f32 * font_size / 1000.0,
                x_advance: pos.x_advance as f32 * font_size / 1000.0,
                y_advance: pos.y_advance as f32 * font_size / 1000.0,
                cluster: info.cluster,
                font_index,
                is_emoji,
                color: if is_emoji {
                    self.emoji_db.get_emoji_color(info.codepoint)
                } else {
                    None
                },
            };

            current_x += glyph.x_advance;
            glyphs.push(glyph);
        }

        Ok(ShapedText {
            glyphs,
            width: current_x,
            height: font_size * 1.2, // Approximate line height
            baseline: font_size * 0.8, // Approximate baseline
            direction: segment.direction,
        })
    }

    /// Detect the primary script in text
    fn detect_script(&self, text: &str) -> Script {
        for ch in text.chars() {
            let script = match ch {
                'a'..='z' | 'A'..='Z' => Script::Latin,
                '\u{0600}'..='\u{06FF}' | '\u{0750}'..='\u{077F}' => Script::Arabic,
                '\u{0590}'..='\u{05FF}' => Script::Hebrew,
                '\u{0900}'..='\u{097F}' => Script::Devanagari,
                '\u{0E00}'..='\u{0E7F}' => Script::Thai,
                '\u{4E00}'..='\u{9FFF}' => Script::Chinese,
                '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}' => Script::Japanese,
                '\u{AC00}'..='\u{D7AF}' => Script::Korean,
                _ if self.emoji_db.is_emoji_codepoint(ch as u32) => Script::Emoji,
                _ => continue,
            };

            if script != Script::Unknown {
                return script;
            }
        }

        Script::Latin // Default to Latin
    }

    /// Detect text direction
    fn detect_direction(&self, text: &str) -> TextDirection {
        let bidi_info = BidiInfo::new(text, None);

        let mut has_ltr = false;
        let mut has_rtl = false;

        for ch in text.chars() {
            let bidi_class = unicode_bidi::bidi_class(ch);
            match bidi_class {
                BidiClass::L => has_ltr = true,
                BidiClass::R | BidiClass::AL => has_rtl = true,
                _ => {}
            }
        }

        match (has_ltr, has_rtl) {
            (true, false) => TextDirection::LeftToRight,
            (false, true) => TextDirection::RightToLeft,
            (true, true) => TextDirection::Mixed,
            _ => TextDirection::LeftToRight,
        }
    }

    /// Segment text by script and direction changes
    fn segment_text(&self, text: &str, script: Script, direction: TextDirection) -> Vec<TextSegment> {
        let mut segments = Vec::new();
        let mut current_segment = String::new();
        let mut current_script = script;

        for ch in text.chars() {
            let ch_script = self.detect_script(&ch.to_string());

            if ch_script != current_script && ch_script != Script::Unknown {
                if !current_segment.is_empty() {
                    segments.push(TextSegment {
                        text: current_segment.clone(),
                        script: current_script,
                        direction,
                    });
                    current_segment.clear();
                }
                current_script = ch_script;
            }

            current_segment.push(ch);
        }

        if !current_segment.is_empty() {
            segments.push(TextSegment {
                text: current_segment,
                script: current_script,
                direction,
            });
        }

        segments
    }

    /// Select the best font for a text segment
    fn select_font_for_segment(&self, segment: &TextSegment) -> usize {
        // For emoji, use emoji font if available
        if segment.script == Script::Emoji {
            return self.fallback_chain.find_emoji_font().unwrap_or(0);
        }

        // Try to find a font that supports the script
        self.fallback_chain.find_font_for_script(segment.script).unwrap_or(0)
    }

    /// Convert our Script enum to HarfBuzz script
    fn script_to_hb_script(&self, script: Script) -> harfbuzz_rs::Script {
        match script {
            Script::Latin => harfbuzz_rs::Script::Latin,
            Script::Arabic => harfbuzz_rs::Script::Arabic,
            Script::Hebrew => harfbuzz_rs::Script::Hebrew,
            Script::Devanagari => harfbuzz_rs::Script::Devanagari,
            Script::Thai => harfbuzz_rs::Script::Thai,
            Script::Chinese => harfbuzz_rs::Script::Han,
            Script::Japanese => harfbuzz_rs::Script::Hiragana,
            Script::Korean => harfbuzz_rs::Script::Hangul,
            _ => harfbuzz_rs::Script::Common,
        }
    }

    /// Detect language from script
    fn detect_language(&self, script: Script) -> Option<String> {
        match script {
            Script::Arabic => Some("ar".to_string()),
            Script::Hebrew => Some("he".to_string()),
            Script::Devanagari => Some("hi".to_string()),
            Script::Thai => Some("th".to_string()),
            Script::Chinese => Some("zh".to_string()),
            Script::Japanese => Some("ja".to_string()),
            Script::Korean => Some("ko".to_string()),
            _ => None,
        }
    }

    /// Reorder glyphs for bidirectional text
    fn reorder_bidi_glyphs(&self, glyphs: Vec<ShapedGlyph>, text: &str) -> Vec<ShapedGlyph> {
        // Use Unicode Bidirectional Algorithm to reorder
        let bidi_info = BidiInfo::new(text, None);

        // This is a simplified implementation
        // In production, you'd use the full bidi algorithm
        glyphs
    }

    /// Shape text for terminal cells (monospace optimization)
    pub fn shape_terminal_cell(&self, ch: char, font_size: f32) -> Result<ShapedGlyph, ShapingError> {
        let text = ch.to_string();
        let shaped = self.shape_text(&text, font_size)?;

        shaped.glyphs.into_iter().next()
            .ok_or(ShapingError::ShapingFailed("No glyphs produced".to_string()))
    }
}

/// Text segment with uniform script and direction
#[derive(Debug, Clone)]
struct TextSegment {
    text: String,
    script: Script,
    direction: TextDirection,
}

/// Errors that can occur during text shaping
#[derive(Debug, thiserror::Error)]
pub enum ShapingError {
    #[error("Font not found: {0}")]
    FontNotFound(String),

    #[error("Invalid font data")]
    InvalidFont,

    #[error("Shaping failed: {0}")]
    ShapingFailed(String),

    #[error("Emoji database error: {0}")]
    EmojiError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_latin_shaping() {
        let config = ShapingConfig::default();
        let shaper = TextShaper::new(config).unwrap();

        let result = shaper.shape_text("Hello, World!", 16.0).unwrap();
        assert!(!result.glyphs.is_empty());
        assert_eq!(result.direction, TextDirection::LeftToRight);
    }

    #[test]
    fn test_arabic_shaping() {
        let config = ShapingConfig::default();
        let shaper = TextShaper::new(config).unwrap();

        let result = shaper.shape_text("مرحبا بالعالم", 16.0).unwrap();
        assert!(!result.glyphs.is_empty());
        assert_eq!(result.direction, TextDirection::RightToLeft);
    }

    #[test]
    fn test_mixed_direction() {
        let config = ShapingConfig::default();
        let shaper = TextShaper::new(config).unwrap();

        let result = shaper.shape_text("Hello مرحبا World", 16.0).unwrap();
        assert!(!result.glyphs.is_empty());
        assert_eq!(result.direction, TextDirection::Mixed);
    }

    #[test]
    fn test_emoji_detection() {
        let config = ShapingConfig::default();
        let shaper = TextShaper::new(config).unwrap();

        let result = shaper.shape_text("Hello 😀 World", 16.0).unwrap();

        // Find the emoji glyph
        let emoji_glyph = result.glyphs.iter()
            .find(|g| g.is_emoji)
            .expect("Should have emoji glyph");

        assert!(emoji_glyph.is_emoji);
    }
}
