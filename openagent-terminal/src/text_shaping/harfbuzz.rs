// HarfBuzz text shaping module for OpenAgent Terminal
// Provides advanced text shaping for complex scripts, ligatures, and bidirectional text

use harfbuzz_rs::{
    Blob, Direction, Face, Font as HbFont, Language, Script, 
    UnicodeBuffer, GlyphBuffer, GlyphInfo, GlyphPosition,
    Feature, Tag
};
use ttf_parser::Face as TtfFace;
use fontdb::{Database, Family, Query, Source};
use unicode_bidi::{BidiInfo, BidiClass, Level};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use anyhow::{Context, Result};
use tracing::{debug, info, warn};

/// Text shaping configuration
#[derive(Debug, Clone)]
pub struct ShapingConfig {
    /// Enable ligatures (e.g., fi, fl, programming ligatures)
    pub enable_ligatures: bool,
    /// Enable kerning adjustments
    pub enable_kerning: bool,
    /// Enable contextual alternates
    pub enable_contextual_alternates: bool,
    /// Enable stylistic sets
    pub stylistic_sets: Vec<u32>,
    /// Default language
    pub default_language: String,
    /// Fallback fonts
    pub fallback_fonts: Vec<String>,
    /// Emoji font
    pub emoji_font: Option<String>,
}

impl Default for ShapingConfig {
    fn default() -> Self {
        Self {
            enable_ligatures: true,
            enable_kerning: true,
            enable_contextual_alternates: true,
            stylistic_sets: vec![],
            default_language: "en".to_string(),
            fallback_fonts: vec![
                "Noto Sans".to_string(),
                "DejaVu Sans".to_string(),
            ],
            emoji_font: Some("Noto Color Emoji".to_string()),
        }
    }
}

/// Shaped glyph with position information
#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    pub glyph_id: u32,
    pub cluster: u32,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub font_index: usize,
}

/// Result of text shaping operation
#[derive(Debug, Clone)]
pub struct ShapedText {
    pub glyphs: Vec<ShapedGlyph>,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
    pub direction: TextDirection,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
    Mixed,
}

/// Cache for shaped text segments
struct ShapingCache {
    entries: HashMap<String, Arc<ShapedText>>,
    lru_order: VecDeque<String>,
    max_entries: usize,
}

impl ShapingCache {
    fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            lru_order: VecDeque::new(),
            max_entries,
        }
    }
    
    fn get(&mut self, key: &str) -> Option<Arc<ShapedText>> {
        if let Some(shaped) = self.entries.get(key) {
            // Move to front of LRU
            if let Some(pos) = self.lru_order.iter().position(|k| k == key) {
                self.lru_order.remove(pos);
            }
            self.lru_order.push_front(key.to_string());
            Some(shaped.clone())
        } else {
            None
        }
    }
    
    fn insert(&mut self, key: String, shaped: Arc<ShapedText>) {
        // Evict if at capacity
        while self.entries.len() >= self.max_entries {
            if let Some(old_key) = self.lru_order.pop_back() {
                self.entries.remove(&old_key);
            }
        }
        
        self.entries.insert(key.clone(), shaped);
        self.lru_order.push_front(key);
    }
    
    fn clear(&mut self) {
        self.entries.clear();
        self.lru_order.clear();
    }
}

/// HarfBuzz text shaper
pub struct HarfBuzzShaper {
    config: ShapingConfig,
    font_database: Arc<Database>,
    font_cache: HashMap<String, Arc<HbFont>>,
    shaping_cache: Arc<RwLock<ShapingCache>>,
}

impl HarfBuzzShaper {
    /// Create a new HarfBuzz shaper
    pub fn new(config: ShapingConfig) -> Result<Self> {
        let mut font_database = Database::new();
        
        // Load system fonts
        font_database.load_system_fonts();
        
        // Log available fonts
        let families: Vec<_> = font_database.families().collect();
        info!("Loaded {} font families", families.len());
        
        Ok(Self {
            config,
            font_database: Arc::new(font_database),
            font_cache: HashMap::new(),
            shaping_cache: Arc::new(RwLock::new(ShapingCache::new(1000))),
        })
    }
    
    /// Shape a text string
    pub fn shape_text(
        &mut self,
        text: &str,
        font_name: &str,
        font_size: f32,
    ) -> Result<ShapedText> {
        // Check cache first
        let cache_key = format!("{}:{}:{}", text, font_name, font_size);
        if let Ok(mut cache) = self.shaping_cache.write() {
            if let Some(shaped) = cache.get(&cache_key) {
                return Ok((*shaped).clone());
            }
        }
        
        // Analyze text direction
        let bidi_info = BidiInfo::new(text, None);
        let direction = self.detect_direction(text);
        
        // Get or load font
        let hb_font = self.get_or_load_font(font_name, font_size)?;
        
        // Create buffer and add text
        let mut buffer = UnicodeBuffer::new();
        buffer.add_str(text);
        
        // Set buffer properties
        buffer.set_direction(match direction {
            TextDirection::RightToLeft => Direction::RTL,
            _ => Direction::LTR,
        });
        
        buffer.set_script(self.detect_script(text));
        buffer.set_language(Language::from_str(&self.config.default_language));
        
        // Apply features
        let features = self.build_features();
        
        // Shape the text
        let glyph_buffer = harfbuzz_rs::shape(&hb_font, buffer, features.as_slice());
        
        // Convert to our format
        let shaped = self.convert_shaped_buffer(glyph_buffer, font_size, direction);
        
        // Cache the result
        let shaped_arc = Arc::new(shaped.clone());
        if let Ok(mut cache) = self.shaping_cache.write() {
            cache.insert(cache_key, shaped_arc);
        }
        
        Ok(shaped)
    }
    
    /// Shape text with fallback for missing glyphs
    pub fn shape_text_with_fallback(
        &mut self,
        text: &str,
        primary_font: &str,
        font_size: f32,
    ) -> Result<ShapedText> {
        let mut all_glyphs = Vec::new();
        let mut current_pos = 0.0;
        
        // Split text into runs that can be handled by single fonts
        let runs = self.split_into_font_runs(text, primary_font);
        
        for run in runs {
            // Choose font name without borrowing self during the call to shape_text.
            let font_name_owned: String = if run.needs_emoji && self.config.emoji_font.is_some() {
                self.config
                    .emoji_font
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| primary_font.to_string())
            } else if run.needs_fallback {
                self
                    .find_fallback_font(&run.text, primary_font)
                    .unwrap_or(primary_font)
                    .to_string()
            } else {
                primary_font.to_string()
            };
            
            let shaped = self.shape_text(&run.text, &font_name_owned, font_size)?;
            
            // Adjust positions and add to result
            for mut glyph in shaped.glyphs {
                glyph.x_offset += current_pos;
                all_glyphs.push(glyph);
            }
            
            current_pos += shaped.width;
        }
        
        Ok(ShapedText {
            glyphs: all_glyphs,
            width: current_pos,
            height: font_size,
            baseline: font_size * 0.8,
            direction: self.detect_direction(text),
        })
    }
    
    /// Detect text direction
    fn detect_direction(&self, text: &str) -> TextDirection {
        let mut has_ltr = false;
        let mut has_rtl = false;
        
        for ch in text.chars() {
            match unicode_bidi::bidi_class(ch) {
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
    
    /// Detect script for text
    fn detect_script(&self, text: &str) -> Script {
        // Simple script detection - could be enhanced
        for ch in text.chars() {
            if ch.is_ascii() {
                continue;
            }
            
            // Check for common scripts
            let code = ch as u32;
            if (0x0600..=0x06FF).contains(&code) {
                return Script::Arabic;
            } else if (0x0900..=0x097F).contains(&code) {
                return Script::Devanagari;
            } else if (0x0E00..=0x0E7F).contains(&code) {
                return Script::Thai;
            } else if (0x4E00..=0x9FFF).contains(&code) {
                return Script::Han;
            } else if (0x3040..=0x309F).contains(&code) {
                return Script::Hiragana;
            } else if (0x30A0..=0x30FF).contains(&code) {
                return Script::Katakana;
            }
        }
        
        Script::Latin
    }
    
    /// Build HarfBuzz features from config
    fn build_features(&self) -> Vec<Feature> {
        let mut features = Vec::new();
        
        if self.config.enable_ligatures {
            features.push(Feature::new(Tag::from_bytes(b"liga"), 1, 0..));
            features.push(Feature::new(Tag::from_bytes(b"clig"), 1, 0..));
        }
        
        if self.config.enable_kerning {
            features.push(Feature::new(Tag::from_bytes(b"kern"), 1, 0..));
        }
        
        if self.config.enable_contextual_alternates {
            features.push(Feature::new(Tag::from_bytes(b"calt"), 1, 0..));
        }
        
        // Add stylistic sets
        for &set_num in &self.config.stylistic_sets {
            if set_num <= 20 {
                let tag = format!("ss{:02}", set_num);
                features.push(Feature::new(Tag::from_bytes(tag.as_bytes()), 1, 0..));
            }
        }
        
        features
    }
    
    /// Get or load a font
    fn get_or_load_font(&mut self, font_name: &str, size: f32) -> Result<Arc<HbFont>> {
        let key = format!("{}:{}", font_name, size);
        
        if let Some(font) = self.font_cache.get(&key) {
            return Ok(font.clone());
        }
        
        // Query font from database
        let query = Query {
            families: &[Family::Name(font_name)],
            ..Default::default()
        };
        
        let font_id = self.font_database
            .query(&query)
            .context("Font not found in database")?;
        
        let (font_data, _) = self.font_database
            .face_source(font_id)
            .context("Failed to get font data")?
            .load(&self.font_database)
            .context("Failed to load font data")?;
        
        // Create HarfBuzz font
        let blob = Blob::from_bytes(&font_data)?;
        let face = Face::new(blob)?;
        let mut hb_font = HbFont::new(face);
        
        // Set scale for the font size
        let units_per_em = hb_font.face().upem() as f32;
        let scale = (size * 64.0 * 96.0 / 72.0) / units_per_em;
        hb_font.set_scale(scale as i32, scale as i32);
        
        let font_arc = Arc::new(hb_font);
        self.font_cache.insert(key, font_arc.clone());
        
        Ok(font_arc)
    }
    
    /// Convert HarfBuzz buffer to our format
    fn convert_shaped_buffer(
        &self,
        buffer: GlyphBuffer,
        font_size: f32,
        direction: TextDirection,
    ) -> ShapedText {
        let glyph_infos = buffer.glyph_infos();
        let glyph_positions = buffer.glyph_positions();
        
        let mut glyphs = Vec::new();
        let mut total_width = 0.0;
        
        for (info, pos) in glyph_infos.iter().zip(glyph_positions.iter()) {
            let glyph = ShapedGlyph {
                glyph_id: info.codepoint,
                cluster: info.cluster,
                x_advance: pos.x_advance as f32 / 64.0,
                y_advance: pos.y_advance as f32 / 64.0,
                x_offset: pos.x_offset as f32 / 64.0,
                y_offset: pos.y_offset as f32 / 64.0,
                font_index: 0,
            };
            
            total_width += glyph.x_advance;
            glyphs.push(glyph);
        }
        
        ShapedText {
            glyphs,
            width: total_width,
            height: font_size,
            baseline: font_size * 0.8,
            direction,
        }
    }
    
    /// Split text into runs for different fonts
    fn split_into_font_runs(&self, text: &str, primary_font: &str) -> Vec<FontRun> {
        let mut runs = Vec::new();
        let mut current_run = FontRun::new();
        
        for ch in text.chars() {
            let needs_emoji = self.is_emoji(ch);
            let needs_fallback = !self.can_render_char(primary_font, ch) && !needs_emoji;
            
            if current_run.is_compatible(needs_emoji, needs_fallback) {
                current_run.text.push(ch);
            } else {
                if !current_run.text.is_empty() {
                    runs.push(current_run);
                }
                current_run = FontRun {
                    text: ch.to_string(),
                    needs_emoji,
                    needs_fallback,
                };
            }
        }
        
        if !current_run.text.is_empty() {
            runs.push(current_run);
        }
        
        runs
    }
    
    /// Check if a character is an emoji
    fn is_emoji(&self, ch: char) -> bool {
        let code = ch as u32;
        // Simplified emoji detection
        (0x1F300..=0x1F9FF).contains(&code) ||
        (0x2600..=0x26FF).contains(&code) ||
        (0x2700..=0x27BF).contains(&code)
    }
    
    /// Check if a font can render a character
    fn can_render_char(&self, font_name: &str, ch: char) -> bool {
        // This is a simplified check - actual implementation would
        // query the font's character map
        true
    }
    
    /// Find a fallback font for text
    fn find_fallback_font(&self, text: &str, primary: &str) -> Option<&str> {
        for fallback in &self.config.fallback_fonts {
            if fallback != primary {
                // Check if this font can handle the text
                // Simplified - actual implementation would check coverage
                return Some(fallback);
            }
        }
        None
    }
    
    /// Clear all caches
    pub fn clear_caches(&mut self) {
        self.font_cache.clear();
        if let Ok(mut cache) = self.shaping_cache.write() {
            cache.clear();
        }
    }
}

/// Font run for splitting text
struct FontRun {
    text: String,
    needs_emoji: bool,
    needs_fallback: bool,
}

impl FontRun {
    fn new() -> Self {
        Self {
            text: String::new(),
            needs_emoji: false,
            needs_fallback: false,
        }
    }
    
    fn is_compatible(&self, needs_emoji: bool, needs_fallback: bool) -> bool {
        self.text.is_empty() ||
        (self.needs_emoji == needs_emoji && self.needs_fallback == needs_fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_shaping() {
        let config = ShapingConfig::default();
        let mut shaper = HarfBuzzShaper::new(config).unwrap();
        
        let shaped = shaper.shape_text("Hello, World!", "Arial", 16.0);
        assert!(shaped.is_ok());
        
        let result = shaped.unwrap();
        assert!(!result.glyphs.is_empty());
        assert_eq!(result.direction, TextDirection::LeftToRight);
    }
    
    #[test]
    fn test_ligature_shaping() {
        let config = ShapingConfig {
            enable_ligatures: true,
            ..Default::default()
        };
        let mut shaper = HarfBuzzShaper::new(config).unwrap();
        
        // Test programming ligatures
        let shaped = shaper.shape_text("=> != <=", "JetBrains Mono", 14.0);
        assert!(shaped.is_ok());
    }
    
    #[test]
    fn test_rtl_text() {
        let config = ShapingConfig::default();
        let mut shaper = HarfBuzzShaper::new(config).unwrap();
        
        let shaped = shaper.shape_text("مرحبا", "Noto Sans Arabic", 16.0);
        assert!(shaped.is_ok());
        
        let result = shaped.unwrap();
        assert_eq!(result.direction, TextDirection::RightToLeft);
    }
}
