# HarfBuzz Text Shaping Integration

OpenAgent Terminal includes advanced text shaping capabilities powered by HarfBuzz, enabling support for complex scripts, ligatures, bidirectional text, and advanced typography features.

## Overview

The HarfBuzz integration provides the following features:

- **Complex Script Support**: Proper rendering of Arabic, Hebrew, Thai, Devanagari, and other complex scripts
- **Bidirectional Text (BiDi)**: Correct handling of mixed left-to-right and right-to-left text
- **Ligatures**: Support for programming font ligatures and traditional typographic ligatures
- **Kerning**: Advanced kerning for better letter spacing
- **Contextual Alternates**: Context-sensitive glyph selection
- **Stylistic Sets**: Access to alternative glyph designs
- **Emoji Support**: Proper rendering of emoji and color fonts

## Configuration

Text shaping can be configured through the terminal configuration file:

```toml
[text_shaping]
# Enable HarfBuzz text shaping
enabled = true

# Enable ligatures (great for programming fonts)
ligatures = true

# Enable kerning adjustments
kerning = true

# Enable contextual alternates
contextual_alternates = true

# Enable stylistic sets (comma-separated list)
stylistic_sets = [1, 2]

# Enable bidirectional text support
bidi_support = true

# Default language for text shaping (ISO 639-1 code)
default_language = "en"

# Fallback fonts for missing glyphs
fallback_fonts = [
    "Noto Sans",
    "DejaVu Sans", 
    "Liberation Sans",
    "Arial"
]

# Emoji font
emoji_font = "Noto Color Emoji"

# Enable complex script support
complex_scripts = true

[text_shaping.cache]
# Enable caching of shaped text runs
enable_line_cache = true

# Maximum number of cached shaped lines
max_cached_lines = 1000

# Maximum number of cached shaped glyphs
max_cached_glyphs = 10000

# Maximum number of cached BiDi analysis results
max_cached_bidi = 500

# Enable glyph position caching for ligatures
cache_ligature_positions = true

[text_shaping.performance]
# Fallback to basic rendering for performance
fallback_on_error = true

# Use HarfBuzz shaping only when necessary
lazy_shaping = true

# Batch size for shaped glyph rendering
batch_size = 4096

# Enable subpixel rendering for shaped text
subpixel_rendering = true

# Enable GPU-accelerated text rendering
gpu_acceleration = true
```

## Predefined Configurations

The terminal includes several predefined configurations for common use cases:

### Programming Configuration

Optimized for programming with ligature support:

```rust
let config = TextShapingConfig::programming();
```

Features:
- Ligatures enabled for programming symbols (`->`, `=>`, `!=`, etc.)
- Contextual alternates for better code readability
- BiDi support disabled (most code is left-to-right)
- Fallback fonts optimized for monospace programming fonts

### Multilingual Configuration

Optimized for international text:

```rust
let config = TextShapingConfig::multilingual();
```

Features:
- Full BiDi support for mixed text
- Complex script support for all major writing systems
- Comprehensive fallback font stack
- Enhanced BiDi caching

### Minimal Configuration

Optimized for performance:

```rust
let config = TextShapingConfig::minimal();
```

Features:
- Basic shaping only
- Reduced cache sizes
- Disabled advanced features for better performance
- Suitable for resource-constrained environments

## Architecture

### Components

1. **HarfBuzz Shaper** (`harfbuzz.rs`): Core HarfBuzz wrapper
2. **Integration Layer** (`integration.rs`): Bridges HarfBuzz with terminal rendering
3. **Shaped Glyph Cache** (`shaped_glyph_cache.rs`): Caching for performance
4. **WGPU Renderer** (`shaped_renderer.rs`): GPU-accelerated rendering
5. **Configuration** (`text_shaping.rs`): Configuration management

### Text Shaping Pipeline

1. **Input**: Terminal cell content with styling information
2. **BiDi Analysis**: Determine text direction and reorder if necessary
3. **Script Detection**: Identify the writing system for proper shaping
4. **Font Selection**: Choose primary font and fallbacks
5. **HarfBuzz Shaping**: Apply shaping rules, ligatures, and positioning
6. **Glyph Loading**: Load shaped glyphs into the atlas
7. **Rendering**: Render shaped glyphs with proper positioning

### Performance Optimizations

- **Multi-level Caching**: Shaped text, glyphs, and BiDi results
- **Lazy Shaping**: Only shape when necessary
- **Batched Rendering**: Efficient GPU rendering
- **Fallback Strategy**: Graceful degradation for performance

## Usage Examples

### Basic Text Shaping

```rust
use openagent_terminal::text_shaping::harfbuzz::{HarfBuzzShaper, ShapingConfig};

let config = ShapingConfig::default();
let mut shaper = HarfBuzzShaper::new(config)?;

let shaped = shaper.shape_text("Hello, World!", "Arial", 16.0)?;
println!("Shaped {} glyphs", shaped.glyphs.len());
```

### Programming Ligatures

```rust
let config = ShapingConfig {
    enable_ligatures: true,
    ..Default::default()
};
let mut shaper = HarfBuzzShaper::new(config)?;

// These will be rendered as ligatures in supporting fonts
let code_samples = vec!["->", "=>", "!=", "<=", ">=", "==="];
for sample in code_samples {
    let shaped = shaper.shape_text(sample, "JetBrains Mono", 14.0)?;
    // Ligatures may reduce glyph count compared to character count
    println!("{}: {} chars -> {} glyphs", sample, sample.len(), shaped.glyphs.len());
}
```

### Bidirectional Text

```rust
let mixed_text = "Hello مرحبا World שלום";
let shaped = shaper.shape_text(mixed_text, "Noto Sans", 16.0)?;

match shaped.direction {
    TextDirection::LeftToRight => println!("Text is LTR"),
    TextDirection::RightToLeft => println!("Text is RTL"),
    TextDirection::Mixed => println!("Text has mixed direction"),
}
```

### Complex Scripts

```rust
// Arabic text with proper contextual shaping
let arabic = "مرحبا بكم في العالم";
let shaped = shaper.shape_text(arabic, "Noto Sans Arabic", 16.0)?;

// Devanagari text with complex glyph composition
let devanagari = "नमस्ते दुनिया";
let shaped = shaper.shape_text(devanagari, "Noto Sans Devanagari", 16.0)?;
```

## Font Recommendations

### Programming Fonts with Ligatures

- **JetBrains Mono**: Excellent ligature support, open source
- **Fira Code**: Popular programming font with extensive ligatures
- **Cascadia Code**: Microsoft's programming font
- **Source Code Pro**: Adobe's monospace font
- **Victor Mono**: Cursive programming font with ligatures

### Multilingual Support

- **Noto Sans**: Google's comprehensive font family
- **DejaVu Sans**: Good fallback with wide character coverage
- **Liberation Sans**: Open source alternative to Arial
- **Source Sans Pro**: Adobe's sans-serif font family

### Emoji Support

- **Noto Color Emoji**: Google's color emoji font
- **Apple Color Emoji**: macOS system emoji font
- **Segoe UI Emoji**: Windows system emoji font

## Troubleshooting

### Common Issues

1. **Missing Glyphs**: Add appropriate fallback fonts
2. **Poor Performance**: Reduce cache sizes or disable advanced features
3. **Incorrect BiDi**: Check text direction detection and Unicode normalization
4. **Ligature Issues**: Verify font supports the desired ligatures

### Performance Tuning

```toml
[text_shaping.performance]
# For better performance on slower systems
fallback_on_error = true
lazy_shaping = true
batch_size = 2048
gpu_acceleration = false

[text_shaping.cache]
# Reduce cache sizes
max_cached_lines = 500
max_cached_glyphs = 5000
```

### Debug Options

Enable debug logging to troubleshoot shaping issues:

```toml
[logging]
level = "debug"

[debug]
print_glyph_cache = true
log_font_loading = true
```

## Testing

The text shaping system includes comprehensive tests:

```bash
# Run all text shaping tests
cargo test --features harfbuzz text_shaping

# Run specific test categories
cargo test --features harfbuzz test_ligature_shaping
cargo test --features harfbuzz test_bidi_text_shaping
cargo test --features harfbuzz test_complex_script_shaping

# Run performance benchmarks
cargo test --features harfbuzz benchmark_shaping_performance -- --ignored
```

## Building with HarfBuzz Support

### Prerequisites

- HarfBuzz library (`libharfbuzz-dev` on Ubuntu/Debian)
- FreeType library (`libfreetype6-dev`)
- System fonts for testing

### Build Commands

```bash
# Enable HarfBuzz feature
cargo build --features harfbuzz

# Enable both HarfBuzz and WGPU for full GPU acceleration
cargo build --features "harfbuzz,wgpu"

# Development build with all text features
cargo build --features "harfbuzz,wgpu" --bin openagent-terminal
```

### Cross-Platform Notes

- **Linux**: Install HarfBuzz through package manager
- **macOS**: Use Homebrew (`brew install harfbuzz`)
- **Windows**: Use vcpkg or build from source

## API Reference

### Core Types

```rust
pub struct ShapedGlyph {
    pub glyph_id: u32,
    pub cluster: u32,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub font_index: usize,
}

pub struct ShapedText {
    pub glyphs: Vec<ShapedGlyph>,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
    pub direction: TextDirection,
}

pub enum TextDirection {
    LeftToRight,
    RightToLeft,
    Mixed,
}
```

### Configuration Types

```rust
pub struct ShapingConfig {
    pub enable_ligatures: bool,
    pub enable_kerning: bool,
    pub enable_contextual_alternates: bool,
    pub stylistic_sets: Vec<u32>,
    pub default_language: String,
    pub fallback_fonts: Vec<String>,
    pub emoji_font: Option<String>,
}
```

## Future Enhancements

Planned improvements for the text shaping system:

1. **Variable Font Support**: OpenType variable font features
2. **Color Font Support**: COLR/CPAL color font tables
3. **Advanced Layout**: Multi-line text layout with line breaking
4. **Performance Improvements**: GPU-based glyph caching
5. **Text Effects**: Outline, shadow, and gradient effects
6. **Interactive Features**: Text selection with proper shaping

## Contributing

Contributions to the text shaping system are welcome:

1. **Testing**: Test with different languages and scripts
2. **Performance**: Profile and optimize critical paths
3. **Features**: Implement additional OpenType features
4. **Documentation**: Improve examples and troubleshooting guides

## License

The HarfBuzz integration follows the same license as OpenAgent Terminal. HarfBuzz itself is licensed under the MIT license.
