use crate::config::*;
use crate::parsers::{system_has_font_family, ConfigParser};
use anyhow::Result;

pub struct ITerm2Parser;

impl ITerm2Parser {
    pub fn new() -> Self {
        Self
    }
}

impl ConfigParser for ITerm2Parser {
    fn parse(&self, content: &str) -> Result<UnifiedConfig> {
        // Minimal plist parsing: attempt XML first, then JSON fallback
        let mut config = UnifiedConfig::new();
        // Try parsing as an XML plist dictionary into serde_json::Value for flexibility
        if let Ok(value) = plist::Value::from_reader_xml(content.as_bytes()) {
            if let Ok(json) = serde_json::to_value(value) {
                // Extract a few common fields
                if let Some(fonts) = json.get("New Bookmarks").and_then(|v| v.as_array()) {
                    if let Some(first) = fonts.first() {
                        if let Some(font_dict) = first.as_object() {
                            if let Some(font_name) =
                                font_dict.get("Normal Font").and_then(|v| v.as_str())
                            {
                                // iTerm2 font strings often look like "JetBrainsMono-Regular 12"
                                let family = font_name.split_whitespace().next().unwrap_or("");
                                if !family.is_empty() {
                                    let normal =
                                        config.font.normal.get_or_insert(Default::default());
                                    normal.family = Some(family.to_string());
                                }
                                if let Some(size_str) = font_name.split_whitespace().last() {
                                    if let Ok(size) = size_str.parse::<f32>() {
                                        config.font.size = Some(size);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(content) {
            // Some iTerm2 profiles can be exported as JSON
            if let Some(fonts) = json_val.get("New Bookmarks").and_then(|v| v.as_array()) {
                if let Some(first) = fonts.first() {
                    if let Some(font_name) = first.get("Normal Font").and_then(|v| v.as_str()) {
                        let family = font_name.split_whitespace().next().unwrap_or("");
                        if !family.is_empty() {
                            let normal = config.font.normal.get_or_insert(Default::default());
                            normal.family = Some(family.to_string());
                        }
                        if let Some(size_str) = font_name.split_whitespace().last() {
                            if let Ok(size) = size_str.parse::<f32>() {
                                config.font.size = Some(size);
                            }
                        }
                    }
                }
            }
        }
        // Append warning about missing font family if applicable
        if let Some(face) = &config.font.normal {
            if let Some(family) = &face.family {
                if !system_has_font_family(family) {
                    let arr = config
                        .custom
                        .get("font_warnings")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let mut new_arr = arr;
                    new_arr.push(serde_json::json!({
                        "family": family,
                        "warning": "Font family not found on this system; fallback will be used"
                    }));
                    // Re-insert into custom map
                    let mut custom = config.custom.clone();
                    custom.insert("font_warnings".to_string(), serde_json::Value::Array(new_arr));
                    config.custom = custom;
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_iterm2_json_font_parsing() {
        let json = r#"{
          "New Bookmarks": [ { "Normal Font": "FiraCode-Regular 14" } ]
        }"#;
        let cfg = ITerm2Parser::new().parse(json).unwrap();
        assert_eq!(cfg.font.size, Some(14.0));
        assert!(cfg.font.normal.as_ref().unwrap().family.as_ref().unwrap().contains("FiraCode"));
    }
}
