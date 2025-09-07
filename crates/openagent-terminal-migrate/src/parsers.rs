use crate::cli::TerminalType;
use crate::config::{MigrationConfig, UnifiedConfig};
use anyhow::{anyhow, Result};
use std::fs;

mod alacritty;
mod iterm2;
mod kitty;
mod wezterm;
mod windows_terminal;

pub use alacritty::AlacrittyParser;
pub use iterm2::ITerm2Parser;
pub use kitty::KittyParser;
pub use wezterm::WezTermParser;
pub use windows_terminal::WindowsTerminalParser;

/// Parse a configuration file based on the terminal type
pub fn parse_config(migration_config: &MigrationConfig) -> Result<UnifiedConfig> {
    let content = fs::read_to_string(&migration_config.config_path).map_err(|e| {
        anyhow!("Failed to read config file {}: {}", migration_config.config_path.display(), e)
    })?;

    match migration_config.terminal_type {
        TerminalType::Alacritty => AlacrittyParser::new().parse(&content),
        TerminalType::ITerm2 => ITerm2Parser::new().parse(&content),
        TerminalType::Kitty => KittyParser::new().parse(&content),
        TerminalType::WindowsTerminal => WindowsTerminalParser::new().parse(&content),
        TerminalType::WezTerm => WezTermParser::new().parse(&content),
        TerminalType::Hyper => parse_hyper_config(&content),
        TerminalType::Warp => parse_warp_config(&content),
        TerminalType::Tabby => parse_tabby_config(&content),
        _ => Err(anyhow!("Parser for {} is not yet implemented", migration_config.terminal_type)),
    }
}

/// Simple parser for Hyper terminal config (.hyper.js)
fn parse_hyper_config(content: &str) -> Result<UnifiedConfig> {
    let mut config = UnifiedConfig::new();

    // Hyper config is JavaScript, so we need basic pattern matching
    // This is a simplified parser - a full implementation would use a JS parser

    // Precompile regexes used in the loop
    let re_font_size = regex::Regex::new(r#"fontSize:\s*(\d+)"#)?;
    let re_font_family = regex::Regex::new(r#"fontFamily:\s*['"]([^'"]+)['"]"#)?;

    // Look for common patterns in Hyper config
    for line in content.lines() {
        let line = line.trim();

        // Font size
        if let Some(captures) = re_font_size.captures(line) {
            if let Some(size_str) = captures.get(1) {
                if let Ok(size) = size_str.as_str().parse::<f32>() {
                    config.font.size = Some(size);
                }
            }
        }

        // Font family
        if let Some(captures) = re_font_family.captures(line) {
            if let Some(family) = captures.get(1) {
                config.font.normal = Some(crate::config::FontFaceConfig {
                    family: Some(family.as_str().to_string()),
                    style: None,
                });
            }
        }

        // Cursor style
        if line.contains("cursorShape:") && line.contains("BEAM") {
            config.terminal.cursor = Some(crate::config::CursorConfig {
                style: Some(crate::config::CursorStyle {
                    shape: Some("Beam".to_string()),
                    blinking: None,
                }),
                ..Default::default()
            });
        }
    }

    Ok(config)
}

/// Simple parser for Warp terminal config (YAML)
fn parse_warp_config(content: &str) -> Result<UnifiedConfig> {
    let warp_config: serde_yaml::Value = serde_yaml::from_str(content)?;
    let mut config = UnifiedConfig::new();

    // Extract font settings
    if let Some(font) = warp_config.get("font") {
        if let Some(size) = font.get("size").and_then(|v| v.as_f64()) {
            config.font.size = Some(size as f32);
        }
        if let Some(family) = font.get("family").and_then(|v| v.as_str()) {
            config.font.normal = Some(crate::config::FontFaceConfig {
                family: Some(family.to_string()),
                style: None,
            });
        }
    }

    // Extract theme/color settings
    if let Some(theme) = warp_config.get("theme") {
        if let Some(theme_name) = theme.as_str() {
            // Store theme name in custom config for later processing
            config
                .custom
                .insert("theme".to_string(), serde_json::Value::String(theme_name.to_string()));
        }
    }

    // Extract window settings
    if let Some(window) = warp_config.get("window") {
        if let Some(opacity) = window.get("opacity").and_then(|v| v.as_f64()) {
            config.window.opacity = Some(opacity as f32);
        }
    }

    Ok(config)
}

/// Simple parser for Tabby terminal config (YAML/JSON)
fn parse_tabby_config(content: &str) -> Result<UnifiedConfig> {
    let mut config = UnifiedConfig::new();

    // Try parsing as YAML first, then JSON
    let tabby_config: serde_yaml::Value = if content.trim_start().starts_with('{') {
        // Looks like JSON
        let json_value: serde_json::Value = serde_json::from_str(content)?;
        serde_yaml::to_value(json_value)?
    } else {
        // Assume YAML
        serde_yaml::from_str(content)?
    };

    // Extract font settings
    if let Some(terminal) = tabby_config.get("terminal") {
        if let Some(font) = terminal.get("font") {
            if let Some(size) = font.get("size").and_then(|v| v.as_f64()) {
                config.font.size = Some(size as f32);
            }
            if let Some(family) = font.get("family").and_then(|v| v.as_str()) {
                config.font.normal = Some(crate::config::FontFaceConfig {
                    family: Some(family.to_string()),
                    style: None,
                });
            }
        }

        // Extract color scheme
        if let Some(colors) = terminal.get("colorScheme") {
            if let Some(background) = colors.get("background").and_then(|v| v.as_str()) {
                config.colors.primary = Some(crate::config::PrimaryColors {
                    background: Some(background.to_string()),
                    ..Default::default()
                });
            }
            if let Some(foreground) = colors.get("foreground").and_then(|v| v.as_str()) {
                if let Some(primary) = &mut config.colors.primary {
                    primary.foreground = Some(foreground.to_string());
                } else {
                    config.colors.primary = Some(crate::config::PrimaryColors {
                        foreground: Some(foreground.to_string()),
                        ..Default::default()
                    });
                }
            }
        }
    }

    // Extract appearance settings
    if let Some(appearance) = tabby_config.get("appearance") {
        if let Some(opacity) = appearance.get("opacity").and_then(|v| v.as_f64()) {
            config.window.opacity = Some(opacity as f32);
        }
    }

    Ok(config)
}

/// Trait for configuration parsers
pub trait ConfigParser {
    fn parse(&self, content: &str) -> Result<UnifiedConfig>;

    fn supports_file_extension(&self, extension: &str) -> bool;

    fn parser_name(&self) -> &'static str;
}

/// Generic parser dispatcher based on file extension
pub fn parse_by_extension(extension: &str, content: &str) -> Result<UnifiedConfig> {
    match extension.to_lowercase().as_str() {
        "yml" | "yaml" => {
            // Could be Alacritty, Warp, or Tabby
            // Try to determine by content structure
            if content.contains("theme:") || content.contains("warp") {
                // Warp configs typically include a top-level 'theme' key
                parse_warp_config(content)
            } else if content.contains("terminal:") {
                // Tabby often nests settings under 'terminal'
                parse_tabby_config(content)
            } else if content.contains("font:") {
                // Fallback to Alacritty when a top-level 'font' section exists
                AlacrittyParser::new().parse(content)
            } else {
                // Default to Tabby if indeterminate
                parse_tabby_config(content)
            }
        },
        "toml" => {
            // Could be Alacritty or WezTerm
            if content.contains("[window]") && content.contains("[font]") {
                AlacrittyParser::new().parse(content)
            } else {
                WezTermParser::new().parse(content)
            }
        },
        "json" => {
            // Could be Windows Terminal, iTerm2, or others
            if content.contains("profiles") && content.contains("schemes") {
                WindowsTerminalParser::new().parse(content)
            } else if content.contains("com.googlecode.iterm2") {
                ITerm2Parser::new().parse(content)
            } else {
                parse_tabby_config(content)
            }
        },
        "conf" => {
            // Likely Kitty
            KittyParser::new().parse(content)
        },
        "js" => {
            // Hyper terminal
            parse_hyper_config(content)
        },
        "lua" => {
            // WezTerm
            WezTermParser::new().parse(content)
        },
        "plist" => {
            // iTerm2
            ITerm2Parser::new().parse(content)
        },
        _ => Err(anyhow!("Unsupported file extension: {}", extension)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_by_extension() {
        // Test YAML parsing
        let yaml_content = r#"
font:
  size: 12.0
  normal:
    family: "JetBrains Mono"
"#;
        let result = parse_by_extension("yaml", yaml_content);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.font.size, Some(12.0));
    }

    #[test]
    fn test_hyper_config_parsing() {
        let hyper_content = r#"
module.exports = {
  config: {
    fontSize: 14,
    fontFamily: 'Menlo, Monaco, "DejaVu Sans Mono"',
    cursorShape: 'BEAM',
  }
}
"#;
        let result = parse_hyper_config(hyper_content);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.font.size, Some(14.0));
    }

    #[test]
    fn test_warp_config_parsing() {
        let warp_content = r#"
font:
  size: 13.0
  family: "SF Mono"
theme: "Dracula"
window:
  opacity: 0.9
"#;
        let result = parse_warp_config(warp_content);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.font.size, Some(13.0));
        assert_eq!(config.window.opacity, Some(0.9));
    }

    #[test]
    fn test_unsupported_extension() {
        let result = parse_by_extension("unknown", "content");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported file extension"));
    }
}
