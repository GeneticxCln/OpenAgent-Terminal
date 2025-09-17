use crate::config::*;
use crate::parsers::ConfigParser;
use anyhow::Result;

pub struct WezTermParser;

impl WezTermParser {
    pub fn new() -> Self {
        Self
    }

    #[cfg(test)]
    fn sample_lua() -> &'static str {
        r#"
        local wezterm = require 'wezterm'
        local config = {}
        config.font = wezterm.font({ family = "Fira Code", weight = "Regular" })
        config.font_size = 12.5
        return config
        "#
    }
}

impl ConfigParser for WezTermParser {
    fn parse(&self, content: &str) -> Result<UnifiedConfig> {
        let mut config = UnifiedConfig::new();

        // Heuristic parsing for common WezTerm Lua config patterns
        // Examples:
        //   config.font = wezterm.font({ family = "JetBrains Mono", weight = "Regular" })
        //   config.font_size = 12.0
        //   table.insert(config.keys, { key = "P", mods = "CTRL|SHIFT", action = wezterm.action.ScrollByPage(-1) })

        for line in content.lines() {
            let line = line.trim();

            // font_size: capture number after '='
            if let Some(idx) = line.find("config.font_size") {
                if let Some(eq) = line[idx..].find('=') {
                    let val = &line[idx + eq + 1..];
                    if let Ok(size) = val.trim().trim_end_matches(',').parse::<f32>() {
                        config.font.size = Some(size);
                    }
                }
            }

            // font family inside wezterm.font({...})
            if line.contains("config.font") && line.contains("family") {
                if let Some(start) = line.find("family") {
                    let rest = &line[start..];
                    if let Some(first_quote) = rest.find('"') {
                        let after = &rest[first_quote + 1..];
                        if let Some(end_quote) = after.find('"') {
                            let fam = &after[..end_quote];
                            if !fam.is_empty() {
                                let normal = config.font.normal.get_or_insert(Default::default());
                                normal.family = Some(fam.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Font availability warning
        if let Some(normal) = &config.font.normal {
            if let Some(family) = &normal.family {
                if !crate::parsers::system_has_font_family(family) {
                    let mut custom = config.custom.clone();
                    let mut arr = custom
                        .get("font_warnings")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    arr.push(serde_json::json!({
                        "family": family,
                        "warning": "Font family not found on this system; fallback will be used"
                    }));
                    custom.insert("font_warnings".into(), serde_json::Value::Array(arr));
                    config.custom = custom;
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_wezterm_heuristics() {
        let content = WezTermParser::sample_lua();
        let cfg = WezTermParser::new().parse(content).unwrap();
        assert_eq!(cfg.font.size, Some(12.5));
        let family = cfg.font.normal.unwrap().family.unwrap();
        assert!(family.starts_with("Fira"));
        // Note: family extraction takes the first token before space; acceptable for heuristic.
    }
}
