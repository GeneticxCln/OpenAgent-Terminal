// Emoji Database Module - Handle emoji detection and rendering

use std::collections::HashMap;
use once_cell::sync::Lazy;

/// Emoji database for detection and color information
pub struct EmojiDatabase {
    emoji_ranges: Vec<(u32, u32)>,
    emoji_colors: HashMap<u32, (u8, u8, u8, u8)>,
    emoji_font_name: String,
}

impl EmojiDatabase {
    pub fn new(emoji_font: &str) -> Result<Self, super::ShapingError> {
        Ok(Self {
            emoji_ranges: Self::init_emoji_ranges(),
            emoji_colors: Self::init_emoji_colors(),
            emoji_font_name: emoji_font.to_string(),
        })
    }
    
    /// Check if a codepoint is an emoji
    pub fn is_emoji_codepoint(&self, codepoint: u32) -> bool {
        // Check common emoji ranges
        self.emoji_ranges.iter().any(|(start, end)| {
            codepoint >= *start && codepoint <= *end
        })
    }
    
    /// Get color information for an emoji
    pub fn get_emoji_color(&self, codepoint: u32) -> Option<(u8, u8, u8, u8)> {
        self.emoji_colors.get(&codepoint).copied()
    }
    
    /// Initialize emoji Unicode ranges
    fn init_emoji_ranges() -> Vec<(u32, u32)> {
        vec![
            (0x1F300, 0x1F5FF), // Miscellaneous Symbols and Pictographs
            (0x1F600, 0x1F64F), // Emoticons
            (0x1F680, 0x1F6FF), // Transport and Map Symbols
            (0x1F700, 0x1F77F), // Alchemical Symbols
            (0x1F780, 0x1F7FF), // Geometric Shapes Extended
            (0x1F800, 0x1F8FF), // Supplemental Arrows-C
            (0x1F900, 0x1F9FF), // Supplemental Symbols and Pictographs
            (0x1FA00, 0x1FA6F), // Chess Symbols
            (0x1FA70, 0x1FAFF), // Symbols and Pictographs Extended-A
            (0x2600, 0x26FF),   // Miscellaneous Symbols
            (0x2700, 0x27BF),   // Dingbats
            (0xFE00, 0xFE0F),   // Variation Selectors
            (0x1F1E6, 0x1F1FF), // Regional Indicator Symbols
        ]
    }
    
    /// Initialize common emoji colors (for demonstration)
    fn init_emoji_colors() -> HashMap<u32, (u8, u8, u8, u8)> {
        let mut colors = HashMap::new();
        
        // Some common emoji with their typical colors
        colors.insert(0x1F600, (255, 204, 77, 255));  // 😀 Grinning face
        colors.insert(0x1F602, (255, 204, 77, 255));  // 😂 Face with tears of joy
        colors.insert(0x1F60D, (255, 204, 77, 255));  // 😍 Heart eyes
        colors.insert(0x1F44D, (255, 204, 77, 255));  // 👍 Thumbs up
        colors.insert(0x2764, (255, 0, 0, 255));      // ❤️ Red heart
        colors.insert(0x1F4A9, (139, 69, 19, 255));   // 💩 Pile of poo
        colors.insert(0x1F525, (255, 140, 0, 255));   // 🔥 Fire
        colors.insert(0x1F4AF, (255, 0, 0, 255));     // 💯 100
        
        colors
    }
    
    /// Get the font name for emoji rendering
    pub fn get_emoji_font(&self) -> &str {
        &self.emoji_font_name
    }
}
