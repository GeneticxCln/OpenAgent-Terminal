// Enhanced glyph cache for shaped text support
// Extends the existing glyph cache to handle HarfBuzz shaped text

#![allow(dead_code)]
use ahash::RandomState;
use anyhow::Result;
use crossfont::{FontKey, GlyphKey, RasterizedGlyph};
use std::collections::HashMap;

use super::glyph_cache::{Glyph, GlyphCache, LoadGlyph};
#[cfg(feature = "harfbuzz")]
use crate::text_shaping::integration::{ShapedCell, ShapedCellGlyph, ShapedLine};

/// Key for shaped glyph caching that includes position information
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ShapedGlyphKey {
    pub font_key: FontKey,
    pub glyph_id: u32,
    pub size: crossfont::Size,
    /// Optional character for fallback lookup
    pub character: Option<char>,
}

/// Enhanced glyph cache that supports shaped text
pub struct ShapedGlyphCache {
    /// Base glyph cache for basic character rendering
    base_cache: GlyphCache,

    /// Cache for shaped glyphs by glyph ID
    shaped_cache: HashMap<ShapedGlyphKey, Glyph, RandomState>,

    /// Cache for shaped glyph positions by character cluster
    position_cache: HashMap<u32, Vec<ShapedGlyphPosition>, RandomState>,

    /// Cache for BiDi analysis results
    bidi_cache: HashMap<String, Vec<BidiRun>, RandomState>,

    /// Maximum number of cached shaped glyphs
    max_shaped_cache_size: usize,

    /// Statistics for cache performance
    shaped_cache_hits: u64,
    shaped_cache_misses: u64,
}

/// Cached position information for a shaped glyph
#[derive(Debug, Clone)]
pub struct ShapedGlyphPosition {
    pub glyph_id: u32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub x_advance: f32,
    pub y_advance: f32,
}

/// BiDi analysis result for caching
#[derive(Debug, Clone)]
pub struct BidiRun {
    pub start: usize,
    pub end: usize,
    #[cfg(feature = "harfbuzz")]
    pub direction: crate::text_shaping::harfbuzz::TextDirection,
    #[cfg(not(feature = "harfbuzz"))]
    pub direction: u8, // Placeholder when harfbuzz is disabled
    pub level: u8,
}

impl ShapedGlyphCache {
    /// Create a new shaped glyph cache
    pub fn new(base_cache: GlyphCache) -> Self {
        Self {
            base_cache,
            shaped_cache: HashMap::with_hasher(RandomState::new()),
            position_cache: HashMap::with_hasher(RandomState::new()),
            bidi_cache: HashMap::with_hasher(RandomState::new()),
            max_shaped_cache_size: 10000, // Configurable
            shaped_cache_hits: 0,
            shaped_cache_misses: 0,
        }
    }

    /// Get a shaped glyph, loading it if necessary
    #[cfg(feature = "harfbuzz")]
    pub fn get_shaped_glyph<L>(
        &mut self,
        shaped_glyph: &ShapedCellGlyph,
        _loader: &mut L,
    ) -> Result<Glyph>
    where
        L: LoadGlyph + ?Sized,
    {
        let key = ShapedGlyphKey {
            font_key: self.get_font_key_for_shaped_glyph(shaped_glyph),
            glyph_id: shaped_glyph.glyph_id,
            size: self.base_cache.font_size,
            character: None, // We'll determine this if needed
        };

        // Check shaped glyph cache first
        if let Some(glyph) = self.shaped_cache.get(&key) {
            self.shaped_cache_hits += 1;
            return Ok(*glyph);
        }

        self.shaped_cache_misses += 1;

        // If not in cache, we need to load it
        // For now, return the existing glyph from the shaped cell glyph
        // In a full implementation, we'd rasterize the glyph by ID
        let glyph = shaped_glyph.glyph;

        // Cache the result
        self.cache_shaped_glyph(key, glyph);

        Ok(glyph)
    }

    /// Load all glyphs for a shaped line
    #[cfg(feature = "harfbuzz")]
    pub fn load_shaped_line<L>(
        &mut self,
        shaped_line: &ShapedLine,
        loader: &mut L,
    ) -> Result<Vec<LoadedShapedCell>>
    where
        L: LoadGlyph + ?Sized,
    {
        let mut loaded_cells = Vec::new();

        for shaped_cell in &shaped_line.cells {
            let loaded_glyphs = self.load_shaped_cell(shaped_cell, loader)?;
            loaded_cells.push(LoadedShapedCell {
                cell_index: shaped_cell.cell_index,
                glyphs: loaded_glyphs,
                cell_width: shaped_cell.cell_width,
            });
        }

        Ok(loaded_cells)
    }

    /// Load all glyphs for a shaped cell
    #[cfg(feature = "harfbuzz")]
    pub fn load_shaped_cell<L>(
        &mut self,
        shaped_cell: &ShapedCell,
        loader: &mut L,
    ) -> Result<Vec<LoadedShapedGlyph>>
    where
        L: LoadGlyph + ?Sized,
    {
        let mut loaded_glyphs = Vec::new();

        for shaped_glyph in &shaped_cell.shaped_glyphs {
            let glyph = self.get_shaped_glyph(shaped_glyph, loader)?;
            loaded_glyphs.push(LoadedShapedGlyph {
                glyph_id: shaped_glyph.glyph_id,
                glyph,
                x_offset: shaped_glyph.x_offset,
                y_offset: shaped_glyph.y_offset,
                x_advance: shaped_glyph.x_advance,
                y_advance: shaped_glyph.y_advance,
                cluster: shaped_glyph.cluster,
                font_index: shaped_glyph.font_index,
            });
        }

        Ok(loaded_glyphs)
    }

    /// Get the appropriate font key for a shaped glyph
    #[cfg(feature = "harfbuzz")]
    fn get_font_key_for_shaped_glyph(&self, shaped_glyph: &ShapedCellGlyph) -> FontKey {
        // For now, use the font index to select the appropriate font key
        // In a full implementation, this would be more sophisticated
        match shaped_glyph.font_index {
            0 => self.base_cache.font_key,
            1 => self.base_cache.bold_key,
            2 => self.base_cache.italic_key,
            3 => self.base_cache.bold_italic_key,
            _ => self.base_cache.font_key, // Fallback
        }
    }

    /// Cache a shaped glyph with LRU eviction
    fn cache_shaped_glyph(&mut self, key: ShapedGlyphKey, glyph: Glyph) {
        // Simple eviction: remove random entries when cache is full
        if self.shaped_cache.len() >= self.max_shaped_cache_size {
            // Remove 20% of entries to avoid frequent evictions
            let keys_to_remove: Vec<ShapedGlyphKey> =
                self.shaped_cache.keys().take(self.max_shaped_cache_size / 5).cloned().collect();

            for key_to_remove in keys_to_remove {
                self.shaped_cache.remove(&key_to_remove);
            }
        }

        self.shaped_cache.insert(key, glyph);
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> ShapedCacheStats {
        ShapedCacheStats {
            shaped_cache_size: self.shaped_cache.len(),
            max_shaped_cache_size: self.max_shaped_cache_size,
            position_cache_size: self.position_cache.len(),
            cache_hits: self.shaped_cache_hits,
            cache_misses: self.shaped_cache_misses,
            hit_ratio: if self.shaped_cache_hits + self.shaped_cache_misses > 0 {
                self.shaped_cache_hits as f64
                    / (self.shaped_cache_hits + self.shaped_cache_misses) as f64
            } else {
                0.0
            },
        }
    }

    /// Clear all shaped caches
    pub fn clear_shaped_caches(&mut self) {
        self.shaped_cache.clear();
        self.position_cache.clear();
        self.bidi_cache.clear();
        self.shaped_cache_hits = 0;
        self.shaped_cache_misses = 0;
    }

    /// Reset the entire cache (including base cache)
    pub fn reset_all_caches<L: LoadGlyph>(&mut self, loader: &mut L) {
        self.base_cache.reset_glyph_cache(loader);
        self.clear_shaped_caches();
    }

    /// Get mutable reference to base cache for compatibility
    pub fn base_cache_mut(&mut self) -> &mut GlyphCache {
        &mut self.base_cache
    }

    /// Get reference to base cache
    pub fn base_cache(&self) -> &GlyphCache {
        &self.base_cache
    }

    /// Delegate font metrics to base cache
    pub fn font_metrics(&self) -> crossfont::Metrics {
        self.base_cache.font_metrics()
    }

    /// Update font size (delegates to base cache)
    pub fn update_font_size(
        &mut self,
        font: &crate::config::font::Font,
    ) -> Result<(), crossfont::Error> {
        // Clear shaped caches since font size changed
        self.clear_shaped_caches();

        // Update base cache
        self.base_cache.update_font_size(font)
    }

    /// Load common glyphs (delegates to base cache)
    pub fn load_common_glyphs<L: LoadGlyph>(&mut self, loader: &mut L) {
        self.base_cache.load_common_glyphs(loader);
    }

    /// Get a basic glyph (delegates to base cache)
    pub fn get<L>(&mut self, glyph_key: GlyphKey, loader: &mut L, show_missing: bool) -> Glyph
    where
        L: LoadGlyph + ?Sized,
    {
        self.base_cache.get(glyph_key, loader, show_missing)
    }

    /// Analyze text for BiDi runs and cache the result
    pub fn analyze_bidi(&mut self, text: &str) -> Vec<BidiRun> {
        if let Some(cached) = self.bidi_cache.get(text) {
            return cached.clone();
        }

        let runs = self.perform_bidi_analysis(text);
        self.bidi_cache.insert(text.to_string(), runs.clone());
        runs
    }

    /// Perform BiDi analysis on text
    fn perform_bidi_analysis(&self, _text: &str) -> Vec<BidiRun> {
        #[cfg(feature = "harfbuzz")]
        {
            use unicode_bidi::BidiInfo;

            let bidi_info = BidiInfo::new(_text, None);
            let mut runs = Vec::new();

            // Get the paragraph embedding level

            // Convert BiDi levels to our BidiRun structure
            let para_range = 0.._text.len();
            let (levels, runs_ranges) = bidi_info.visual_runs(&bidi_info.paragraphs[0], para_range);
            for (run_range, &level) in runs_ranges.iter().zip(levels.iter()) {
                let direction = if level.is_ltr() {
                    crate::text_shaping::harfbuzz::TextDirection::LeftToRight
                } else {
                    crate::text_shaping::harfbuzz::TextDirection::RightToLeft
                };

                runs.push(BidiRun {
                    start: run_range.start,
                    end: run_range.end,
                    direction,
                    level: level.number(),
                });
            }

            runs
        }
        #[cfg(not(feature = "harfbuzz"))]
        {
            // Without harfbuzz, assume left-to-right text
            vec![BidiRun {
                start: 0,
                end: _text.len(),
                direction: 0, // LTR placeholder
                level: 0,
            }]
        }
    }

    /// Cache shaped text with position offsets for ligatures
    pub fn cache_ligature_positions(&mut self, cluster: u32, positions: Vec<ShapedGlyphPosition>) {
        self.position_cache.insert(cluster, positions);
    }

    /// Get cached ligature positions
    pub fn get_ligature_positions(&self, cluster: u32) -> Option<&Vec<ShapedGlyphPosition>> {
        self.position_cache.get(&cluster)
    }

    /// Optimize cache by removing least recently used entries
    pub fn optimize_cache(&mut self) {
        // Remove oldest BiDi cache entries if we have too many
        if self.bidi_cache.len() > 1000 {
            let keys_to_remove: Vec<String> =
                self.bidi_cache.keys().take(self.bidi_cache.len() - 800).cloned().collect();
            for key in keys_to_remove {
                self.bidi_cache.remove(&key);
            }
        }

        // Remove old position cache entries
        if self.position_cache.len() > 5000 {
            let keys_to_remove: Vec<u32> = self
                .position_cache
                .keys()
                .take(self.position_cache.len() - 4000)
                .cloned()
                .collect();
            for key in keys_to_remove {
                self.position_cache.remove(&key);
            }
        }
    }
}

/// Loaded shaped cell with all glyphs ready for rendering
#[derive(Debug, Clone)]
pub struct LoadedShapedCell {
    pub cell_index: usize,
    pub glyphs: Vec<LoadedShapedGlyph>,
    pub cell_width: f32,
}

/// Loaded shaped glyph ready for rendering
#[derive(Debug, Clone)]
pub struct LoadedShapedGlyph {
    pub glyph_id: u32,
    pub glyph: Glyph,
    pub x_offset: f32,
    pub y_offset: f32,
    pub x_advance: f32,
    pub y_advance: f32,
    pub cluster: u32,
    pub font_index: usize,
}

/// Cache performance statistics
#[derive(Debug, Clone)]
pub struct ShapedCacheStats {
    pub shaped_cache_size: usize,
    pub max_shaped_cache_size: usize,
    pub position_cache_size: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub hit_ratio: f64,
}

/// Trait extension for loaders that support shaped glyphs
pub trait LoadShapedGlyph: LoadGlyph {
    /// Load a shaped glyph by ID instead of character
    fn load_shaped_glyph(&mut self, _glyph_id: u32, rasterized: &RasterizedGlyph) -> Glyph {
        // Default implementation falls back to regular glyph loading
        self.load_glyph(rasterized)
    }

    /// Check if the loader supports direct glyph ID loading
    fn supports_glyph_id_loading(&self) -> bool {
        false
    }
}

// Blanket implementation for all LoadGlyph implementors
impl<T: LoadGlyph> LoadShapedGlyph for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::font::Font;
    use crossfont::{Rasterize, Rasterizer, Size};

    #[test]
    fn test_shaped_glyph_key_equality() {
        let font_key = FontKey::next();
        let key1 =
            ShapedGlyphKey { font_key, glyph_id: 42, size: Size::new(16.0), character: Some('a') };

        let key2 = ShapedGlyphKey {
            font_key, // Same font key
            glyph_id: 42,
            size: Size::new(16.0),
            character: Some('a'),
        };

        assert_eq!(key1, key2);

        // Test that different font keys produce different keys
        let key3 = ShapedGlyphKey {
            font_key: FontKey::next(),
            glyph_id: 42,
            size: Size::new(16.0),
            character: Some('a'),
        };

        assert_ne!(key1, key3);
    }

    #[test]
    fn test_cache_stats() {
        let font = Font::default();
        let rasterizer = Rasterizer::new().unwrap();
        let base_cache = GlyphCache::new(rasterizer, &font).unwrap();
        let cache = ShapedGlyphCache::new(base_cache);

        let stats = cache.get_cache_stats();
        assert_eq!(stats.shaped_cache_size, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);
        assert_eq!(stats.hit_ratio, 0.0);
    }
}
