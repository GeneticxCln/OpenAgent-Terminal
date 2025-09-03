// HarfBuzz integration layer for OpenAgent Terminal
// Bridges the HarfBuzz text shaping system with the existing rendering pipeline

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crossfont::{FontKey, GlyphKey};
use openagent_terminal_core::index::{Column, Point};
use openagent_terminal_core::term::cell::Flags;

use crate::display::content::RenderableCell;
use crate::display::SizeInfo;
use crate::renderer::text::glyph_cache::{Glyph, GlyphCache, LoadGlyph};
use crate::text_shaping::harfbuzz::{HarfBuzzShaper, ShapedGlyph, ShapedText, ShapingConfig, TextDirection};
use crate::renderer::text::builtin_font;
use crossfont::{FontDesc, FontKey, RasterizedGlyph};
use crate::config::ui_config::Delta;
use crate::config::font::Font as FontConfig;
use crate::config::Config;

/// Integrated text shaper that combines HarfBuzz with the existing glyph system
pub struct IntegratedTextShaper {
    harfbuzz_shaper: HarfBuzzShaper,
    shaped_line_cache: Arc<RwLock<HashMap<String, ShapedLineInfo>>>,
    config: ShapingIntegrationConfig,
}

/// Configuration for text shaping integration
#[derive(Debug, Clone)]
pub struct ShapingIntegrationConfig {
    pub enable_ligatures: bool,
    pub enable_kerning: bool,
    pub enable_complex_scripts: bool,
    pub cache_shaped_lines: bool,
    pub max_cached_lines: usize,
    pub fallback_to_basic_rendering: bool,
}

impl Default for ShapingIntegrationConfig {
    fn default() -> Self {
        Self {
            enable_ligatures: true,
            enable_kerning: true,
            enable_complex_scripts: true,
            cache_shaped_lines: true,
            max_cached_lines: 500,
            fallback_to_basic_rendering: true,
        }
    }
}

/// Information about a shaped line of text
#[derive(Debug, Clone)]
struct ShapedLineInfo {
    shaped_text: ShapedText,
    font_name: String,
    font_size: f32,
    cell_count: usize,
}

/// Result of shaping a terminal line
#[derive(Debug)]
pub struct ShapedLine {
    pub cells: Vec<ShapedCell>,
    pub direction: TextDirection,
    pub total_width: f32,
}

/// A shaped cell with positioning information
#[derive(Debug, Clone)]
pub struct ShapedCell {
    pub cell_index: usize,
    pub shaped_glyphs: Vec<ShapedCellGlyph>,
    pub cell_width: f32,
}

/// Glyph within a shaped cell
#[derive(Debug, Clone)]
pub struct ShapedCellGlyph {
    pub glyph_id: u32,
    pub glyph: Glyph,
    pub x_offset: f32,
    pub y_offset: f32,
    pub x_advance: f32,
    pub y_advance: f32,
    pub cluster: u32,
    pub font_index: usize,
}

impl IntegratedTextShaper {
    /// Create a new integrated text shaper
    pub fn new(
        font_config: &FontConfig,
        integration_config: ShapingIntegrationConfig,
    ) -> Result<Self> {
        // Create HarfBuzz configuration from terminal font config
        let shaping_config = ShapingConfig {
            enable_ligatures: integration_config.enable_ligatures,
            enable_kerning: integration_config.enable_kerning,
            enable_contextual_alternates: true,
            stylistic_sets: vec![], // Could be configurable
            default_language: "en".to_string(),
            fallback_fonts: vec![
                "Noto Sans".to_string(),
                "DejaVu Sans".to_string(),
                "Liberation Sans".to_string(),
            ],
            emoji_font: Some("Noto Color Emoji".to_string()),
        };

        let harfbuzz_shaper = HarfBuzzShaper::new(shaping_config)
            .context("Failed to create HarfBuzz shaper")?;

        Ok(Self {
            harfbuzz_shaper,
            shaped_line_cache: Arc::new(RwLock::new(HashMap::new())),
            config: integration_config,
        })
    }

    /// Shape a line of terminal cells
    pub fn shape_line<I>(
        &mut self,
        cells: I,
        glyph_cache: &mut GlyphCache,
        size_info: &SizeInfo,
    ) -> Result<ShapedLine>
    where
        I: Iterator<Item = RenderableCell>,
    {
        let cells_vec: Vec<RenderableCell> = cells.collect();
        
        // Extract text content from cells
        let text = cells_vec.iter()
            .map(|cell| cell.character)
            .collect::<String>();

        // Check cache if enabled
        let cache_key = if self.config.cache_shaped_lines {
            Some(format!("{}:{}:{}", text, glyph_cache.font_size.to_bits(), 
                       self.get_font_name(glyph_cache)))
        } else {
            None
        };

        if let Some(ref key) = cache_key {
            if let Ok(cache) = self.shaped_line_cache.read() {
                if let Some(cached_info) = cache.get(key) {
                    return self.convert_cached_to_shaped_line(
                        &cached_info.shaped_text,
                        &cells_vec,
                        glyph_cache,
                        size_info,
                    );
                }
            }
        }

        // Shape the text using HarfBuzz
        let font_name = self.get_font_name(glyph_cache);
        let font_size = glyph_cache.font_size.get();

        let shaped_text = if self.should_use_harfbuzz_shaping(&text) {
            match self.harfbuzz_shaper.shape_text_with_fallback(&text, &font_name, font_size) {
                Ok(shaped) => shaped,
                Err(_) if self.config.fallback_to_basic_rendering => {
                    // Fall back to basic shaping
                    return self.shape_line_basic(cells_vec.into_iter(), glyph_cache, size_info);
                }
                Err(e) => return Err(e),
            }
        } else {
            // Use basic shaping for simple text
            return self.shape_line_basic(cells_vec.into_iter(), glyph_cache, size_info);
        };

        // Cache the result
        if let Some(key) = cache_key {
            if let Ok(mut cache) = self.shaped_line_cache.write() {
                // Implement LRU eviction
                if cache.len() >= self.config.max_cached_lines {
                    // Remove oldest entries (simplified LRU)
                    let keys_to_remove: Vec<String> = cache.keys()
                        .take(cache.len() - self.config.max_cached_lines + 1)
                        .cloned()
                        .collect();
                    for key_to_remove in keys_to_remove {
                        cache.remove(&key_to_remove);
                    }
                }
                
                cache.insert(key, ShapedLineInfo {
                    shaped_text: shaped_text.clone(),
                    font_name,
                    font_size,
                    cell_count: cells_vec.len(),
                });
            }
        }

        // Convert HarfBuzz output to terminal format
        self.convert_shaped_text_to_line(shaped_text, &cells_vec, glyph_cache, size_info)
    }

    /// Convert shaped text to terminal line format
    fn convert_shaped_text_to_line(
        &self,
        shaped_text: ShapedText,
        original_cells: &[RenderableCell],
        glyph_cache: &mut GlyphCache,
        size_info: &SizeInfo,
    ) -> Result<ShapedLine> {
        let mut shaped_cells = Vec::new();
        let cell_width = size_info.cell_width();
        
        // Group shaped glyphs by cluster (character position)
        let mut cluster_groups: HashMap<u32, Vec<&ShapedGlyph>> = HashMap::new();
        for glyph in &shaped_text.glyphs {
            cluster_groups.entry(glyph.cluster).or_default().push(glyph);
        }

        // Convert each original cell
        for (cell_index, cell) in original_cells.iter().enumerate() {
            let cluster = cell_index as u32;
            let shaped_glyphs = if let Some(hb_glyphs) = cluster_groups.get(&cluster) {
                self.convert_harfbuzz_glyphs_to_cell_glyphs(
                    hb_glyphs,
                    cell,
                    glyph_cache,
                )?
            } else {
                // No shaped glyph for this cell, create a basic one
                vec![self.create_basic_cell_glyph(cell, glyph_cache)?]
            };

            shaped_cells.push(ShapedCell {
                cell_index,
                shaped_glyphs,
                cell_width,
            });
        }

        Ok(ShapedLine {
            cells: shaped_cells,
            direction: shaped_text.direction,
            total_width: shaped_text.width,
        })
    }

    /// Convert cached shaped text to shaped line
    fn convert_cached_to_shaped_line(
        &self,
        shaped_text: &ShapedText,
        original_cells: &[RenderableCell],
        glyph_cache: &mut GlyphCache,
        size_info: &SizeInfo,
    ) -> Result<ShapedLine> {
        self.convert_shaped_text_to_line(shaped_text.clone(), original_cells, glyph_cache, size_info)
    }

    /// Convert HarfBuzz glyphs to cell glyphs
    fn convert_harfbuzz_glyphs_to_cell_glyphs(
        &self,
        hb_glyphs: &[&ShapedGlyph],
        cell: &RenderableCell,
        glyph_cache: &mut GlyphCache,
    ) -> Result<Vec<ShapedCellGlyph>> {
        let mut cell_glyphs = Vec::new();

        for hb_glyph in hb_glyphs {
            // Get the corresponding glyph from the cache
            let glyph_key = GlyphKey {
                font_key: self.get_font_key_for_cell(cell, glyph_cache),
                size: glyph_cache.font_size,
                character: cell.character, // This might need adjustment for complex shaping
            };

            // Load the glyph (this should be refactored to use a loader)
            let glyph = self.load_glyph_for_shaped(glyph_key, hb_glyph, glyph_cache)?;

            cell_glyphs.push(ShapedCellGlyph {
                glyph_id: hb_glyph.glyph_id,
                glyph,
                x_offset: hb_glyph.x_offset,
                y_offset: hb_glyph.y_offset,
                x_advance: hb_glyph.x_advance,
                y_advance: hb_glyph.y_advance,
                cluster: hb_glyph.cluster,
                font_index: hb_glyph.font_index,
            });
        }

        Ok(cell_glyphs)
    }

    /// Create a basic cell glyph for unshaped characters
    fn create_basic_cell_glyph(
        &self,
        cell: &RenderableCell,
        glyph_cache: &mut GlyphCache,
    ) -> Result<ShapedCellGlyph> {
        let glyph_key = GlyphKey {
            font_key: self.get_font_key_for_cell(cell, glyph_cache),
            size: glyph_cache.font_size,
            character: cell.character,
        };

        // Load the glyph through the existing glyph cache system
        let glyph = glyph_cache.get(&glyph_key, &mut LoadGlyphImpl)
            .map_err(|e| anyhow::anyhow!("Failed to load basic glyph: {:?}", e))?;

        Ok(ShapedCellGlyph {
            glyph_id: cell.character as u32,
            glyph,
            x_offset: 0.0,
            y_offset: 0.0,
            x_advance: glyph_cache.font_size.get() * 0.6, // This could be more accurate
            y_advance: 0.0,
            cluster: 0,
            font_index: 0,
        })
    }

    /// Fall back to basic line shaping
    fn shape_line_basic<I>(
        &self,
        cells: I,
        glyph_cache: &mut GlyphCache,
        size_info: &SizeInfo,
    ) -> Result<ShapedLine>
    where
        I: Iterator<Item = RenderableCell>,
    {
        let mut shaped_cells = Vec::new();
        let cell_width = size_info.cell_width();

        for (index, cell) in cells.enumerate() {
            let shaped_glyph = self.create_basic_cell_glyph(&cell, glyph_cache)?;
            
            shaped_cells.push(ShapedCell {
                cell_index: index,
                shaped_glyphs: vec![shaped_glyph],
                cell_width,
            });
        }

        Ok(ShapedLine {
            cells: shaped_cells,
            direction: TextDirection::LeftToRight,
            total_width: shaped_cells.len() as f32 * cell_width,
        })
    }

    /// Determine if text should use HarfBuzz shaping
    fn should_use_harfbuzz_shaping(&self, text: &str) -> bool {
        if !self.config.enable_complex_scripts {
            return false;
        }

        // Check for complex scripts, ligatures, or RTL text
        for ch in text.chars() {
            let code = ch as u32;
            
            // Check for non-Latin scripts
            if (0x0600..=0x06FF).contains(&code) ||  // Arabic
               (0x0590..=0x05FF).contains(&code) ||  // Hebrew
               (0x0900..=0x097F).contains(&code) ||  // Devanagari
               (0x0E00..=0x0E7F).contains(&code) ||  // Thai
               (0x4E00..=0x9FFF).contains(&code) ||  // CJK
               (0x3040..=0x309F).contains(&code) ||  // Hiragana
               (0x30A0..=0x30FF).contains(&code) ||  // Katakana
               (0xAC00..=0xD7AF).contains(&code)     // Hangul
            {
                return true;
            }
        }

        // Check for ligature candidates if ligatures are enabled
        if self.config.enable_ligatures {
            if text.contains("->") || text.contains("=>") || text.contains("!=") ||
               text.contains("<=") || text.contains(">=") || text.contains("==") ||
               text.contains("fi") || text.contains("fl") || text.contains("ffi") ||
               text.contains("ffl")
            {
                return true;
            }
        }

        false
    }

    /// Get font name from glyph cache
    fn get_font_name(&self, glyph_cache: &GlyphCache) -> String {
        // This is a placeholder - we'd need to extract the actual font name
        // from the glyph cache's font system
        "JetBrains Mono".to_string()
    }

    /// Get font key for a cell based on its flags
    fn get_font_key_for_cell(&self, cell: &RenderableCell, glyph_cache: &GlyphCache) -> FontKey {
        match cell.flags & Flags::BOLD_ITALIC {
            Flags::BOLD_ITALIC => glyph_cache.bold_italic_key,
            Flags::ITALIC => glyph_cache.italic_key,
            Flags::BOLD => glyph_cache.bold_key,
            _ => glyph_cache.font_key,
        }
    }

    /// Load glyph for shaped text through the glyph cache system
    fn load_glyph_for_shaped(
        &self,
        glyph_key: GlyphKey,
        shaped_glyph: &ShapedGlyph,
        glyph_cache: &mut GlyphCache,
    ) -> Result<Glyph> {
        // Use the existing glyph cache system to load glyphs
        // The glyph_key already contains font, size, and character information
        glyph_cache.get(&glyph_key, &mut LoadGlyphImpl)
            .map_err(|e| anyhow::anyhow!("Failed to load glyph: {:?}", e))
    }

    /// Clear caches
    pub fn clear_caches(&mut self) {
        self.harfbuzz_shaper.clear_caches();
        if let Ok(mut cache) = self.shaped_line_cache.write() {
            cache.clear();
        }
    }
}

/// Glyph loader implementation for shaped text integration
struct LoadGlyphImpl;

impl LoadGlyph for LoadGlyphImpl {
    fn load_glyph(
        &mut self,
        rasterizer: &mut dyn crossfont::Rasterize,
        glyph_key: &GlyphKey,
        metrics: &crossfont::Metrics,
        offset: &Delta<i8>,
        glyph_offset: &Delta<i8>,
    ) -> Result<RasterizedGlyph, crossfont::Error> {
        // Check for builtin glyphs first (box drawing, powerline symbols, etc.)
        if let Some(glyph) = builtin_font::builtin_glyph(
            glyph_key.character,
            metrics,
            offset,
            glyph_offset,
        ) {
            return Ok(glyph);
        }

        // Use the standard rasterizer for regular glyphs
        rasterizer.get_glyph(glyph_key)
    }
}

/// Trait for renderers that support shaped text
pub trait ShapedTextRenderer {
    /// Render a shaped line of text
    fn render_shaped_line(
        &mut self,
        shaped_line: &ShapedLine,
        size_info: &SizeInfo,
    ) -> Result<()>;

    /// Check if the renderer supports shaped text rendering
    fn supports_shaped_text(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::font::Font as FontConfig;
    use crossfont::Size;

    #[test]
    fn test_integration_config_default() {
        let config = ShapingIntegrationConfig::default();
        assert!(config.enable_ligatures);
        assert!(config.enable_kerning);
        assert!(config.cache_shaped_lines);
    }

    #[test]
    fn test_should_use_harfbuzz_shaping() {
        let config = ShapingIntegrationConfig::default();
        let font_config = FontConfig::default();
        let shaper = IntegratedTextShaper::new(&font_config, config).unwrap();

        // Simple ASCII should not require HarfBuzz
        assert!(!shaper.should_use_harfbuzz_shaping("hello"));

        // Text with ligature candidates should use HarfBuzz
        assert!(shaper.should_use_harfbuzz_shaping("=> != <="));

        // Arabic text should use HarfBuzz
        assert!(shaper.should_use_harfbuzz_shaping("مرحبا"));
    }
}
