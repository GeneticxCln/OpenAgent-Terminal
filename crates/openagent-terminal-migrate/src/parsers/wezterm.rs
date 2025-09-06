use crate::config::*;
use crate::parsers::ConfigParser;
use anyhow::Result;

pub struct WezTermParser;

impl WezTermParser {
    pub fn new() -> Self {
        Self
    }
}

impl ConfigParser for WezTermParser {
    fn parse(&self, content: &str) -> Result<UnifiedConfig> {
        let mut config = UnifiedConfig::new();

        // WezTerm uses Lua config, which is complex to parse
        // For now, implement basic pattern matching
        for line in content.lines() {
            let line = line.trim();

            if line.starts_with("config.font_size") {
                if let Some(size_str) = line.split('=').nth(1) {
                    if let Ok(size) = size_str.trim().parse::<f32>() {
                        config.font.size = Some(size);
                    }
                }
            }

            if line.starts_with("config.font") && line.contains("family") {
                // Basic parsing for font family
                // TODO: Implement proper Lua parsing
            }
        }

        Ok(config)
    }

    fn supports_file_extension(&self, extension: &str) -> bool {
        matches!(extension.to_lowercase().as_str(), "lua" | "toml")
    }

    fn parser_name(&self) -> &'static str {
        "WezTerm"
    }
}
