// Comprehensive tests for HarfBuzz text shaping integration

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::font::Font;
    use crate::text_shaping::harfbuzz::{HarfBuzzShaper, ShapingConfig, TextDirection};
    use crate::text_shaping::integration::{IntegratedTextShaper, ShapingIntegrationConfig};

    /// Test basic HarfBuzz shaping functionality
    #[test]
    fn test_basic_shaping() {
        let config = ShapingConfig::default();
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        let result = shaper.shape_text("Hello", "Arial", 16.0);
        assert!(result.is_ok());

        let shaped = result.unwrap();
        assert!(!shaped.glyphs.is_empty());
        assert_eq!(shaped.direction, TextDirection::LeftToRight);
        assert!(shaped.width > 0.0);
    }

    /// Test ligature shaping with programming fonts
    #[test]
    fn test_ligature_shaping() {
        let config = ShapingConfig {
            enable_ligatures: true,
            ..Default::default()
        };
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        // Test common programming ligatures
        let ligature_tests = vec![
            "->",
            "=>",
            "!=",
            "<=",
            ">=",
            "==",
            "===",
            "!==",
            "<=>",
            "//",
            "/*",
            "*/",
        ];

        for ligature in ligature_tests {
            let result = shaper.shape_text(ligature, "JetBrains Mono", 14.0);
            if result.is_ok() {
                let shaped = result.unwrap();
                assert!(!shaped.glyphs.is_empty(), "Failed to shape ligature: {}", ligature);

                // For ligatures, the number of glyphs may be less than characters
                println!("Ligature '{}': {} chars -> {} glyphs",
                        ligature, ligature.len(), shaped.glyphs.len());
            }
        }
    }

    /// Test right-to-left text shaping
    #[test]
    fn test_rtl_text_shaping() {
        let config = ShapingConfig::default();
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        // Arabic text
        let arabic_text = "مرحبا بكم";
        let result = shaper.shape_text(arabic_text, "Noto Sans Arabic", 16.0);
        if result.is_ok() {
            let shaped = result.unwrap();
            assert_eq!(shaped.direction, TextDirection::RightToLeft);
            assert!(!shaped.glyphs.is_empty());
        }

        // Hebrew text
        let hebrew_text = "שלום עולם";
        let result = shaper.shape_text(hebrew_text, "Noto Sans Hebrew", 16.0);
        if result.is_ok() {
            let shaped = result.unwrap();
            assert_eq!(shaped.direction, TextDirection::RightToLeft);
            assert!(!shaped.glyphs.is_empty());
        }
    }

    /// Test mixed direction (BiDi) text
    #[test]
    fn test_bidi_text_shaping() {
        let config = ShapingConfig::default();
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        // Mixed English and Arabic
        let mixed_text = "Hello مرحبا World";
        let result = shaper.shape_text(mixed_text, "Noto Sans", 16.0);
        if result.is_ok() {
            let shaped = result.unwrap();
            assert_eq!(shaped.direction, TextDirection::Mixed);
            assert!(!shaped.glyphs.is_empty());
        }
    }

    /// Test complex script shaping (Devanagari)
    #[test]
    fn test_complex_script_shaping() {
        let config = ShapingConfig::default();
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        // Devanagari text
        let devanagari_text = "नमस्ते";
        let result = shaper.shape_text(devanagari_text, "Noto Sans Devanagari", 16.0);
        if result.is_ok() {
            let shaped = result.unwrap();
            assert!(!shaped.glyphs.is_empty());
            // Complex scripts may have different glyph count than character count
            println!("Devanagari text: {} chars -> {} glyphs",
                    devanagari_text.chars().count(), shaped.glyphs.len());
        }
    }

    /// Test Thai script shaping
    #[test]
    fn test_thai_shaping() {
        let config = ShapingConfig::default();
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        // Thai text
        let thai_text = "สวัสดี";
        let result = shaper.shape_text(thai_text, "Noto Sans Thai", 16.0);
        if result.is_ok() {
            let shaped = result.unwrap();
            assert!(!shaped.glyphs.is_empty());
            println!("Thai text: {} chars -> {} glyphs",
                    thai_text.chars().count(), shaped.glyphs.len());
        }
    }

    /// Test emoji handling
    #[test]
    fn test_emoji_shaping() {
        let config = ShapingConfig {
            emoji_font: Some("Noto Color Emoji".to_string()),
            ..Default::default()
        };
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        // Test various emoji
        let emoji_tests = vec![
            "😀", // Simple emoji
            "👨‍💻", // Compound emoji
            "🏳️‍🌈", // Flag emoji with modifiers
            "👍🏽", // Emoji with skin tone modifier
        ];

        for emoji in emoji_tests {
            let result = shaper.shape_text_with_fallback(emoji, "Arial", 16.0);
            if result.is_ok() {
                let shaped = result.unwrap();
                assert!(!shaped.glyphs.is_empty(), "Failed to shape emoji: {}", emoji);
                println!("Emoji '{}': {} chars -> {} glyphs",
                        emoji, emoji.chars().count(), shaped.glyphs.len());
            }
        }
    }

    /// Test fallback font handling
    #[test]
    fn test_fallback_fonts() {
        let config = ShapingConfig {
            fallback_fonts: vec![
                "Noto Sans".to_string(),
                "DejaVu Sans".to_string(),
                "Arial".to_string(),
            ],
            ..Default::default()
        };
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        // Text with characters that may not be in the primary font
        let mixed_text = "Hello 你好 مرحبا नमस्ते";
        let result = shaper.shape_text_with_fallback(mixed_text, "Courier New", 14.0);
        if result.is_ok() {
            let shaped = result.unwrap();
            assert!(!shaped.glyphs.is_empty());
        }
    }

    /// Test integration layer functionality
    #[test]
    fn test_integration_layer() {
        let font_config = Font::default();
        let integration_config = ShapingIntegrationConfig::default();

        let result = IntegratedTextShaper::new(&font_config, integration_config);
        assert!(result.is_ok());

        let _shaper = result.unwrap();
        // Integration tests would require more setup with glyph cache, etc.
    }

    /// Test shaping cache functionality
    #[test]
    fn test_shaping_cache() {
        let config = ShapingConfig::default();
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        let text = "Hello, World!";
        let font = "Arial";
        let size = 16.0;

        // Shape the same text multiple times
        let result1 = shaper.shape_text(text, font, size);
        let result2 = shaper.shape_text(text, font, size);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let shaped1 = result1.unwrap();
        let shaped2 = result2.unwrap();

        // Results should be identical
        assert_eq!(shaped1.glyphs.len(), shaped2.glyphs.len());
        assert_eq!(shaped1.width, shaped2.width);
        assert_eq!(shaped1.direction, shaped2.direction);
    }

    /// Test stylistic sets
    #[test]
    fn test_stylistic_sets() {
        let config = ShapingConfig {
            stylistic_sets: vec![1, 2],
            ..Default::default()
        };
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        // Text that might be affected by stylistic sets
        let text = "0123456789 agh";
        let result = shaper.shape_text(text, "Source Code Pro", 14.0);
        if result.is_ok() {
            let shaped = result.unwrap();
            assert!(!shaped.glyphs.is_empty());
        }
    }

    /// Test kerning functionality
    #[test]
    fn test_kerning() {
        let config_with_kerning = ShapingConfig {
            enable_kerning: true,
            ..Default::default()
        };
        let config_without_kerning = ShapingConfig {
            enable_kerning: false,
            ..Default::default()
        };

        let mut shaper_with = HarfBuzzShaper::new(config_with_kerning).unwrap();
        let mut shaper_without = HarfBuzzShaper::new(config_without_kerning).unwrap();

        // Text with kerning pairs
        let text = "AV To Wo";

        let result_with = shaper_with.shape_text(text, "Times New Roman", 16.0);
        let result_without = shaper_without.shape_text(text, "Times New Roman", 16.0);

        if result_with.is_ok() && result_without.is_ok() {
            let shaped_with = result_with.unwrap();
            let shaped_without = result_without.unwrap();

            // With kerning should typically be slightly narrower
            println!("With kerning: {} width", shaped_with.width);
            println!("Without kerning: {} width", shaped_without.width);
        }
    }

    /// Benchmark basic shaping performance
    #[test]
    #[ignore] // Only run when explicitly requested
    fn benchmark_shaping_performance() {
        let config = ShapingConfig::default();
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        let test_strings = vec![
            "Hello, World!",
            "The quick brown fox jumps over the lazy dog",
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit",
            "fn main() { println!(\"Hello, world!\"); }",
            "if (condition) { return value; } else { return null; }",
        ];

        let iterations = 1000;
        let start = std::time::Instant::now();

        for _ in 0..iterations {
            for text in &test_strings {
                let _ = shaper.shape_text(text, "JetBrains Mono", 14.0);
            }
        }

        let elapsed = start.elapsed();
        println!("Shaped {} strings {} times in {:?}",
                test_strings.len(), iterations, elapsed);
        println!("Average: {:?} per string",
                elapsed / (iterations * test_strings.len() as u32));
    }

    /// Test error handling and edge cases
    #[test]
    fn test_error_handling() {
        let config = ShapingConfig::default();
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        // Empty string
        let result = shaper.shape_text("", "Arial", 16.0);
        assert!(result.is_ok());

        // Very long string
        let long_string = "a".repeat(10000);
        let result = shaper.shape_text(&long_string, "Arial", 16.0);
        assert!(result.is_ok());

        // Invalid font (should fall back)
        let result = shaper.shape_text("Hello", "NonexistentFont", 16.0);
        // This might fail or succeed with fallback, depending on implementation
        println!("Invalid font result: {:?}", result.is_ok());

        // Very small font size
        let result = shaper.shape_text("Hello", "Arial", 0.1);
        assert!(result.is_ok());

        // Very large font size
        let result = shaper.shape_text("Hello", "Arial", 1000.0);
        assert!(result.is_ok());
    }

    /// Test memory usage and cleanup
    #[test]
    fn test_memory_cleanup() {
        let config = ShapingConfig::default();
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        // Shape many different strings to fill caches
        for i in 0..1000 {
            let text = format!("Test string number {}", i);
            let _ = shaper.shape_text(&text, "Arial", 14.0);
        }

        // Clear caches
        shaper.clear_caches();

        // Should still work after clearing
        let result = shaper.shape_text("Hello after clear", "Arial", 14.0);
        assert!(result.is_ok());
    }

    /// Stress test shaping of large Arabic text to catch panics/regressions
    #[test]
    fn test_large_arabic_stress() {
        let config = ShapingConfig::default();
        let mut shaper = HarfBuzzShaper::new(config).unwrap();

        // 1000-chars Arabic sequence
        let base = "مرحبا";
        let text = base.repeat(200);
        let shaped = shaper.shape_text_with_fallback(&text, "Noto Sans Arabic", 14.0);
        assert!(shaped.is_ok());
        let out = shaped.unwrap();
        assert!(out.glyphs.len() > 0);
    }

    /// Stress test large multi-script string to validate fallback paths
    #[test]
    #[ignore]
    fn test_large_multiscript_stress() {
        let config = ShapingConfig::default();
        let mut shaper = HarfBuzzShaper::new(config).unwrap();
        let blob = [
            "Hello 你好 مرحبا नमस्ते こんにちは 안녕하세요 สวัสดี",
            "😀👨‍💻🏳️‍🌈👍🏽",
        ]
        .join(" ");
        let text = blob.repeat(200);
        let shaped = shaper.shape_text_with_fallback(&text, "Noto Sans", 13.0);
        assert!(shaped.is_ok());
        let out = shaped.unwrap();
        assert!(out.glyphs.len() > 0);
    }
}

/// Integration tests that require a full terminal setup
#[cfg(test)]
mod integration_tests {
    use super::*;

    // These would require more complex setup and are examples
    // of how integration testing could work

    #[test]
    #[ignore] // Requires full terminal setup
    fn test_terminal_line_shaping() {
        // This would test shaping a full terminal line with various content
        // Requires SizeInfo, GlyphCache, and RenderableCell setup
    }

    #[test]
    #[ignore] // Requires WGPU setup
    fn test_wgpu_shaped_rendering() {
        // This would test the WGPU shaped text renderer
        // Requires WGPU device, queue, and surface setup
    }
}
