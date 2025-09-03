// Text shaping configuration for OpenAgent Terminal
// Configures HarfBuzz text shaping, BiDi support, and advanced typography

use serde::{Deserialize, Serialize};

/// Configuration for advanced text shaping features
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextShapingConfig {
    /// Enable HarfBuzz text shaping
    pub enabled: bool,
    
    /// Enable ligatures (programming fonts, etc.)
    pub ligatures: bool,
    
    /// Enable kerning adjustments
    pub kerning: bool,
    
    /// Enable contextual alternates
    pub contextual_alternates: bool,
    
    /// Enable stylistic sets (comma-separated list like "1,2,5")
    pub stylistic_sets: Vec<u32>,
    
    /// Enable bidirectional text support
    pub bidi_support: bool,
    
    /// Default language for text shaping (ISO 639-1 code)
    pub default_language: String,
    
    /// Fallback fonts for missing glyphs
    pub fallback_fonts: Vec<String>,
    
    /// Emoji font name
    pub emoji_font: Option<String>,
    
    /// Enable complex script support (Arabic, Thai, Devanagari, etc.)
    pub complex_scripts: bool,
    
    /// Cache settings
    pub cache: TextShapingCacheConfig,
    
    /// Performance settings
    pub performance: TextShapingPerformanceConfig,
}

/// Text shaping cache configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextShapingCacheConfig {
    /// Enable caching of shaped text runs
    pub enable_line_cache: bool,
    
    /// Maximum number of cached shaped lines
    pub max_cached_lines: usize,
    
    /// Maximum number of cached shaped glyphs
    pub max_cached_glyphs: usize,
    
    /// Maximum number of cached BiDi analysis results
    pub max_cached_bidi: usize,
    
    /// Enable glyph position caching for ligatures
    pub cache_ligature_positions: bool,
}

/// Text shaping performance configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextShapingPerformanceConfig {
    /// Fallback to basic rendering for performance
    pub fallback_on_error: bool,
    
    /// Use HarfBuzz shaping only when necessary
    pub lazy_shaping: bool,
    
    /// Batch size for shaped glyph rendering
    pub batch_size: usize,
    
    /// Enable subpixel rendering for shaped text
    pub subpixel_rendering: bool,
    
    /// Enable GPU-accelerated text rendering
    pub gpu_acceleration: bool,
}

impl Default for TextShapingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ligatures: true,
            kerning: true,
            contextual_alternates: true,
            stylistic_sets: vec![],
            bidi_support: true,
            default_language: "en".to_string(),
            fallback_fonts: vec![
                "Noto Sans".to_string(),
                "DejaVu Sans".to_string(),
                "Liberation Sans".to_string(),
                "Arial".to_string(),
            ],
            emoji_font: Some("Noto Color Emoji".to_string()),
            complex_scripts: true,
            cache: TextShapingCacheConfig::default(),
            performance: TextShapingPerformanceConfig::default(),
        }
    }
}

impl Default for TextShapingCacheConfig {
    fn default() -> Self {
        Self {
            enable_line_cache: true,
            max_cached_lines: 1000,
            max_cached_glyphs: 10000,
            max_cached_bidi: 500,
            cache_ligature_positions: true,
        }
    }
}

impl Default for TextShapingPerformanceConfig {
    fn default() -> Self {
        Self {
            fallback_on_error: true,
            lazy_shaping: true,
            batch_size: 4096,
            subpixel_rendering: true,
            gpu_acceleration: true,
        }
    }
}

impl TextShapingConfig {
    /// Check if any advanced shaping features are enabled
    pub fn has_advanced_features(&self) -> bool {
        self.enabled && (
            self.ligatures ||
            self.kerning ||
            self.contextual_alternates ||
            !self.stylistic_sets.is_empty() ||
            self.complex_scripts
        )
    }
    
    /// Check if BiDi support should be enabled
    pub fn needs_bidi_analysis(&self) -> bool {
        self.enabled && self.bidi_support
    }
    
    /// Get HarfBuzz shaping configuration
    pub fn to_harfbuzz_config(&self) -> crate::text_shaping::harfbuzz::ShapingConfig {
        crate::text_shaping::harfbuzz::ShapingConfig {
            enable_ligatures: self.ligatures,
            enable_kerning: self.kerning,
            enable_contextual_alternates: self.contextual_alternates,
            stylistic_sets: self.stylistic_sets.clone(),
            default_language: self.default_language.clone(),
            fallback_fonts: self.fallback_fonts.clone(),
            emoji_font: self.emoji_font.clone(),
        }
    }
    
    /// Get integration configuration for the text shaper
    pub fn to_integration_config(&self) -> crate::text_shaping::integration::ShapingIntegrationConfig {
        crate::text_shaping::integration::ShapingIntegrationConfig {
            enable_ligatures: self.ligatures,
            enable_kerning: self.kerning,
            enable_complex_scripts: self.complex_scripts,
            cache_shaped_lines: self.cache.enable_line_cache,
            max_cached_lines: self.cache.max_cached_lines,
            fallback_to_basic_rendering: self.performance.fallback_on_error,
        }
    }
    
    /// Validate configuration and return any issues
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();
        
        if self.cache.max_cached_lines == 0 {
            issues.push("max_cached_lines must be greater than 0".to_string());
        }
        
        if self.cache.max_cached_glyphs == 0 {
            issues.push("max_cached_glyphs must be greater than 0".to_string());
        }
        
        if self.performance.batch_size == 0 {
            issues.push("batch_size must be greater than 0".to_string());
        }
        
        // Check if language code is valid (basic check)
        if self.default_language.len() != 2 {
            issues.push("default_language should be a 2-letter ISO 639-1 code".to_string());
        }
        
        // Validate stylistic sets (should be in range 1-20)
        for &set in &self.stylistic_sets {
            if !(1..=20).contains(&set) {
                issues.push(format!("Stylistic set {} is out of range (1-20)", set));
            }
        }
        
        issues
    }
    
    /// Create a minimal configuration for better performance
    pub fn minimal() -> Self {
        Self {
            enabled: true,
            ligatures: false,
            kerning: false,
            contextual_alternates: false,
            stylistic_sets: vec![],
            bidi_support: false,
            default_language: "en".to_string(),
            fallback_fonts: vec!["Arial".to_string()],
            emoji_font: None,
            complex_scripts: false,
            cache: TextShapingCacheConfig {
                enable_line_cache: false,
                max_cached_lines: 100,
                max_cached_glyphs: 1000,
                max_cached_bidi: 50,
                cache_ligature_positions: false,
            },
            performance: TextShapingPerformanceConfig {
                fallback_on_error: true,
                lazy_shaping: true,
                batch_size: 1024,
                subpixel_rendering: false,
                gpu_acceleration: false,
            },
        }
    }
    
    /// Create a configuration optimized for programming
    pub fn programming() -> Self {
        Self {
            enabled: true,
            ligatures: true,
            kerning: true,
            contextual_alternates: true,
            stylistic_sets: vec![1, 2], // Common programming ligature sets
            bidi_support: false, // Most code is LTR
            default_language: "en".to_string(),
            fallback_fonts: vec![
                "JetBrains Mono".to_string(),
                "Fira Code".to_string(),
                "Source Code Pro".to_string(),
            ],
            emoji_font: Some("Noto Color Emoji".to_string()),
            complex_scripts: false,
            cache: TextShapingCacheConfig::default(),
            performance: TextShapingPerformanceConfig::default(),
        }
    }
    
    /// Create a configuration optimized for multilingual text
    pub fn multilingual() -> Self {
        Self {
            enabled: true,
            ligatures: true,
            kerning: true,
            contextual_alternates: true,
            stylistic_sets: vec![],
            bidi_support: true,
            default_language: "en".to_string(),
            fallback_fonts: vec![
                "Noto Sans".to_string(),
                "Noto Sans CJK".to_string(),
                "Noto Sans Arabic".to_string(),
                "Noto Sans Devanagari".to_string(),
                "DejaVu Sans".to_string(),
            ],
            emoji_font: Some("Noto Color Emoji".to_string()),
            complex_scripts: true,
            cache: TextShapingCacheConfig {
                max_cached_bidi: 2000, // More BiDi caching
                ..TextShapingCacheConfig::default()
            },
            performance: TextShapingPerformanceConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = TextShapingConfig::default();
        assert!(config.enabled);
        assert!(config.ligatures);
        assert!(config.has_advanced_features());
        assert!(config.needs_bidi_analysis());
    }
    
    #[test]
    fn test_minimal_config() {
        let config = TextShapingConfig::minimal();
        assert!(config.enabled);
        assert!(!config.ligatures);
        assert!(!config.has_advanced_features());
        assert!(!config.needs_bidi_analysis());
    }
    
    #[test]
    fn test_programming_config() {
        let config = TextShapingConfig::programming();
        assert!(config.enabled);
        assert!(config.ligatures);
        assert!(!config.bidi_support);
        assert_eq!(config.stylistic_sets, vec![1, 2]);
    }
    
    #[test]
    fn test_multilingual_config() {
        let config = TextShapingConfig::multilingual();
        assert!(config.enabled);
        assert!(config.bidi_support);
        assert!(config.complex_scripts);
        assert!(config.fallback_fonts.len() > 3);
    }
    
    #[test]
    fn test_config_validation() {
        let mut config = TextShapingConfig::default();
        assert!(config.validate().is_empty());
        
        config.cache.max_cached_lines = 0;
        let issues = config.validate();
        assert!(!issues.is_empty());
        assert!(issues[0].contains("max_cached_lines"));
    }
    
    #[test]
    fn test_stylistic_set_validation() {
        let mut config = TextShapingConfig::default();
        config.stylistic_sets = vec![0, 25];
        let issues = config.validate();
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().any(|issue| issue.contains("out of range")));
    }
}
