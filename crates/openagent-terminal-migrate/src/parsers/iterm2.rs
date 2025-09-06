use crate::config::*;
use crate::parsers::ConfigParser;
use anyhow::Result;

pub struct ITerm2Parser;

impl ITerm2Parser {
    pub fn new() -> Self {
        Self
    }
}

impl ConfigParser for ITerm2Parser {
    fn parse(&self, content: &str) -> Result<UnifiedConfig> {
        // Placeholder implementation for iTerm2 plist parsing
        // TODO: Implement full plist parsing using the `plist` crate
        let config = UnifiedConfig::new();

        // Basic pattern matching for common settings
        if content.contains("<key>Terminal Type</key>") {
            // This is likely an iTerm2 configuration
        }

        Ok(config)
    }

    fn supports_file_extension(&self, extension: &str) -> bool {
        matches!(extension.to_lowercase().as_str(), "plist" | "json")
    }

    fn parser_name(&self) -> &'static str {
        "iTerm2"
    }
}
