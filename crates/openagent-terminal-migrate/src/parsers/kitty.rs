use crate::config::*;
use crate::parsers::ConfigParser;
use anyhow::Result;

pub struct KittyParser;

impl KittyParser {
    pub fn new() -> Self {
        Self
    }
}

impl ConfigParser for KittyParser {
    fn parse(&self, content: &str) -> Result<UnifiedConfig> {
        let mut config = UnifiedConfig::new();

        // Parse Kitty config format (key value pairs)
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            if let Some((key, value)) = line.split_once(' ') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "font_size" => {
                        if let Ok(size) = value.parse::<f32>() {
                            config.font.size = Some(size);
                        }
                    },
                    "font_family" => {
                        config.font.normal =
                            Some(FontFaceConfig { family: Some(value.to_string()), style: None });
                    },
                    "background_opacity" => {
                        if let Ok(opacity) = value.parse::<f32>() {
                            config.window.opacity = Some(opacity);
                        }
                    },
                    "background" => {
                        config.colors.primary = Some(PrimaryColors {
                            background: Some(value.to_string()),
                            ..Default::default()
                        });
                    },
                    "foreground" => {
                        if let Some(primary) = &mut config.colors.primary {
                            primary.foreground = Some(value.to_string());
                        } else {
                            config.colors.primary = Some(PrimaryColors {
                                foreground: Some(value.to_string()),
                                ..Default::default()
                            });
                        }
                    },
                    _ => {},
                }
            }
        }

        Ok(config)
    }

    fn supports_file_extension(&self, extension: &str) -> bool {
        matches!(extension.to_lowercase().as_str(), "conf")
    }

    fn parser_name(&self) -> &'static str {
        "Kitty"
    }
}
