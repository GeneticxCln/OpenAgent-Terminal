// Font Fallback Chain Module - Manage font fallback for missing glyphs

use std::sync::Arc;
use harfbuzz_rs::Font;
use super::Script;

/// Font fallback chain manager
pub struct FontFallbackChain {
    fonts: Vec<Arc<Font<'static>>>,
    script_support: Vec<ScriptSupport>,
}

struct ScriptSupport {
    font_index: usize,
    supported_scripts: Vec<Script>,
    is_emoji_font: bool,
}

impl FontFallbackChain {
    pub fn new(fonts: &[Arc<Font<'static>>]) -> Self {
        let mut script_support = Vec::new();

        // Analyze each font for script support
        for (index, _font) in fonts.iter().enumerate() {
            let mut supported = Vec::new();

            // Heuristic: assume first font supports Latin
            if index == 0 {
                supported.push(Script::Latin);
            }

            // Check for specific font names that indicate script support
            // In production, you'd analyze the font's character coverage
            script_support.push(ScriptSupport {
                font_index: index,
                supported_scripts: supported,
                is_emoji_font: false, // Would check font tables
            });
        }

        Self {
            fonts: fonts.to_vec(),
            script_support,
        }
    }

    /// Find a font that supports the given script
    pub fn find_font_for_script(&self, script: Script) -> Option<usize> {
        for support in &self.script_support {
            if support.supported_scripts.contains(&script) {
                return Some(support.font_index);
            }
        }

        // Fall back to primary font
        Some(0)
    }

    /// Find the emoji font
    pub fn find_emoji_font(&self) -> Option<usize> {
        self.script_support
            .iter()
            .find(|s| s.is_emoji_font)
            .map(|s| s.font_index)
    }

    /// Get font by index
    pub fn get_font(&self, index: usize) -> Option<&Arc<Font<'static>>> {
        self.fonts.get(index)
    }
}
