use crate::config::*;
use crate::parsers::ConfigParser;
use anyhow::Result;

pub struct WindowsTerminalParser;

impl WindowsTerminalParser {
    pub fn new() -> Self {
        Self
    }
}

impl ConfigParser for WindowsTerminalParser {
    fn parse(&self, content: &str) -> Result<UnifiedConfig> {
        let wt_config: serde_json::Value = serde_json::from_str(content)?;
        let mut config = UnifiedConfig::new();

        // Parse Windows Terminal JSON config
        if let Some(profiles) = wt_config.get("profiles") {
            if let Some(defaults) = profiles.get("defaults") {
                // Font settings
                if let Some(font) = defaults.get("font") {
                    if let Some(size) = font.get("size").and_then(|v| v.as_f64()) {
                        config.font.size = Some(size as f32);
                    }
                    if let Some(face) = font.get("face").and_then(|v| v.as_str()) {
                        config.font.normal =
                            Some(FontFaceConfig { family: Some(face.to_string()), style: None });
                    }
                }

                // Opacity
                if let Some(opacity) = defaults.get("opacity").and_then(|v| v.as_f64()) {
                    config.window.opacity = Some((opacity / 100.0) as f32);
                }
            }
        }

        // Parse color schemes
        if let Some(schemes) = wt_config.get("schemes") {
            if let Some(schemes_array) = schemes.as_array() {
                if let Some(first_scheme) = schemes_array.first() {
                    if let Some(background) =
                        first_scheme.get("background").and_then(|v| v.as_str())
                    {
                        config.colors.primary = Some(PrimaryColors {
                            background: Some(background.to_string()),
                            ..Default::default()
                        });
                    }
                    if let Some(foreground) =
                        first_scheme.get("foreground").and_then(|v| v.as_str())
                    {
                        if let Some(primary) = &mut config.colors.primary {
                            primary.foreground = Some(foreground.to_string());
                        } else {
                            config.colors.primary = Some(PrimaryColors {
                                foreground: Some(foreground.to_string()),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }

        Ok(config)
    }

    fn supports_file_extension(&self, extension: &str) -> bool {
        matches!(extension.to_lowercase().as_str(), "json")
    }

    fn parser_name(&self) -> &'static str {
        "Windows Terminal"
    }
}
