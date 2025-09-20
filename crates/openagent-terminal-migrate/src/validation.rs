use anyhow::{anyhow, Result};
use colored::*;
use std::path::Path;

/// Validate an OpenAgent Terminal configuration file
pub fn validate_config(config_path: &Path) -> Result<()> {
    println!("{}", format!("🔍 Validating configuration: {}", config_path.display()).cyan());

    if !config_path.exists() {
        return Err(anyhow!("Configuration file does not exist: {}", config_path.display()));
    }

    let content = std::fs::read_to_string(config_path)?;

    // Try to parse as TOML
    let parsed: Result<toml::Value, _> = toml::from_str(&content);

    match parsed {
        Ok(config) => {
            println!("{}", "✅ Configuration file is valid TOML".green());

            // Perform additional validation checks
            validate_sections(&config)?;

            println!("{}", "✅ Configuration validation passed".green());
            Ok(())
        }
        Err(e) => {
            println!("{}", format!("❌ TOML parsing error: {}", e).red());
            Err(anyhow!("Invalid TOML configuration: {}", e))
        }
    }
}

fn validate_sections(config: &toml::Value) -> Result<()> {
    let mut warnings = Vec::new();
    let errors: Vec<String> = Vec::new();

    // Check for required sections
    if let Some(table) = config.as_table() {
        // Font validation
        if let Some(font) = table.get("font") {
            if let Err(e) = validate_font_section(font) {
                warnings.push(format!("Font configuration: {}", e));
            }
        }

        // Window validation
        if let Some(window) = table.get("window") {
            if let Err(e) = validate_window_section(window) {
                warnings.push(format!("Window configuration: {}", e));
            }
        }

        // Colors validation
        if let Some(colors) = table.get("colors") {
            if let Err(e) = validate_colors_section(colors) {
                warnings.push(format!("Colors configuration: {}", e));
            }
        }

        // Terminal validation
        if let Some(terminal) = table.get("terminal") {
            if let Err(e) = validate_terminal_section(terminal) {
                warnings.push(format!("Terminal configuration: {}", e));
            }
        }

        // Key bindings validation
        if let Some(keyboard) = table.get("keyboard") {
            if let Some(bindings) = keyboard.get("bindings") {
                if let Err(e) = validate_key_bindings(bindings) {
                    warnings.push(format!("Key bindings: {}", e));
                }
            }
        }

        // AI configuration validation
        if let Some(ai) = table.get("ai") {
            if let Err(e) = validate_ai_section(ai) {
                warnings.push(format!("AI configuration: {}", e));
            }
        }
    }

    // Print warnings
    for warning in &warnings {
        println!("{}", format!("⚠️  {}", warning).yellow());
    }

    // Print errors
    for error in &errors {
        println!("{}", format!("❌ {}", error).red());
    }

    if !errors.is_empty() {
        return Err(anyhow!("Configuration validation failed with {} error(s)", errors.len()));
    }

    if !warnings.is_empty() {
        println!(
            "{}",
            format!("Configuration is valid but has {} warning(s)", warnings.len()).yellow()
        );
    }

    Ok(())
}

fn validate_font_section(font: &toml::Value) -> Result<()> {
    if let Some(size) = font.get("size") {
        if let Some(size_val) = size.as_float() {
            if size_val <= 0.0 || size_val > 144.0 {
                return Err(anyhow!("Font size {} is outside reasonable range (0-144)", size_val));
            }
        }
    }

    // Check if font family exists (basic check)
    if let Some(normal) = font.get("normal") {
        if let Some(family) = normal.get("family") {
            if let Some(family_str) = family.as_str() {
                if family_str.trim().is_empty() {
                    return Err(anyhow!("Font family cannot be empty"));
                }
                // TODO: Add system font availability check
            }
        }
    }

    Ok(())
}

fn validate_window_section(window: &toml::Value) -> Result<()> {
    if let Some(opacity) = window.get("opacity") {
        if let Some(opacity_val) = opacity.as_float() {
            if !(0.0..=1.0).contains(&opacity_val) {
                return Err(anyhow!("Window opacity {} must be between 0.0 and 1.0", opacity_val));
            }
        }
    }

    if let Some(decorations) = window.get("decorations") {
        if let Some(decorations_str) = decorations.as_str() {
            match decorations_str.to_lowercase().as_str() {
                "full" | "none" | "transparent" | "buttonless" => {}
                _ => return Err(anyhow!("Invalid decorations value: {}", decorations_str)),
            }
        }
    }

    Ok(())
}

fn validate_colors_section(colors: &toml::Value) -> Result<()> {
    // Validate color format for all color entries
    validate_color_subsection(colors.get("primary"), "primary")?;
    validate_color_subsection(colors.get("normal"), "normal")?;
    validate_color_subsection(colors.get("bright"), "bright")?;
    validate_color_subsection(colors.get("cursor"), "cursor")?;
    validate_color_subsection(colors.get("selection"), "selection")?;

    Ok(())
}

fn validate_color_subsection(section: Option<&toml::Value>, section_name: &str) -> Result<()> {
    if let Some(section_val) = section {
        if let Some(table) = section_val.as_table() {
            for (key, value) in table {
                if let Some(color_str) = value.as_str() {
                    if !is_valid_color(color_str) {
                        return Err(anyhow!(
                            "Invalid color '{}' in {} section: {}",
                            color_str,
                            section_name,
                            key
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn is_valid_color(color: &str) -> bool {
    // Check if it's a valid hex color
    if let Some(hex_part) = color.strip_prefix('#') {
        return (hex_part.len() == 3 || hex_part.len() == 6 || hex_part.len() == 8)
            && hex_part.chars().all(|c| c.is_ascii_hexdigit());
    }

    // Check if it's a named color (basic set)
    matches!(
        color.to_lowercase().as_str(),
        "black"
            | "red"
            | "green"
            | "yellow"
            | "blue"
            | "magenta"
            | "cyan"
            | "white"
            | "gray"
            | "grey"
            | "darkgray"
            | "darkgrey"
            | "lightgray"
            | "lightgrey"
            | "darkred"
            | "darkgreen"
            | "darkyellow"
            | "darkblue"
            | "darkmagenta"
            | "darkcyan"
            | "lightred"
            | "lightgreen"
            | "lightyellow"
            | "lightblue"
            | "lightmagenta"
            | "lightcyan"
    )
}

fn validate_terminal_section(terminal: &toml::Value) -> Result<()> {
    // Validate scrolling settings
    if let Some(scrolling) = terminal.get("scrolling") {
        if let Some(history) = scrolling.get("history") {
            if let Some(history_val) = history.as_integer() {
                if !(0..=1_000_000).contains(&history_val) {
                    return Err(anyhow!(
                        "Scrolling history {} is outside reasonable range (0-1,000,000)",
                        history_val
                    ));
                }
            }
        }
    }

    // Validate cursor settings
    if let Some(cursor) = terminal.get("cursor") {
        if let Some(style) = cursor.get("style") {
            if let Some(shape) = style.get("shape") {
                if let Some(shape_str) = shape.as_str() {
                    match shape_str.to_lowercase().as_str() {
                        "block" | "underline" | "beam" => {}
                        _ => return Err(anyhow!("Invalid cursor shape: {}", shape_str)),
                    }
                }
            }
        }
    }

    Ok(())
}

fn validate_key_bindings(bindings: &toml::Value) -> Result<()> {
    if let Some(bindings_array) = bindings.as_array() {
        for (i, binding) in bindings_array.iter().enumerate() {
            if let Some(key) = binding.get("key") {
                if let Some(key_str) = key.as_str() {
                    if key_str.trim().is_empty() {
                        return Err(anyhow!("Key binding {} has empty key", i + 1));
                    }
                }
            } else {
                return Err(anyhow!("Key binding {} missing 'key' field", i + 1));
            }

            // Must have either action or chars
            let has_action = binding.get("action").is_some();
            let has_chars = binding.get("chars").is_some();
            let has_command = binding.get("command").is_some();

            if !has_action && !has_chars && !has_command {
                return Err(anyhow!(
                    "Key binding {} must have 'action', 'chars', or 'command'",
                    i + 1
                ));
            }
        }
    }

    Ok(())
}

fn validate_ai_section(ai: &toml::Value) -> Result<()> {
    if let Some(enabled) = ai.get("enabled") {
        if let Some(enabled_val) = enabled.as_bool() {
            if enabled_val {
                // Check for required AI configuration when enabled
                if let Some(provider) = ai.get("provider") {
                    if let Some(provider_str) = provider.as_str() {
                        match provider_str.to_lowercase().as_str() {
                            "null" | "ollama" | "openai" | "anthropic" => {}
                            _ => return Err(anyhow!("Invalid AI provider: {}", provider_str)),
                        }

                        // Check provider-specific configuration
                        if provider_str != "null" {
                            // For real providers, check if endpoint/model configuration exists
                            let has_endpoint =
                                ai.get(format!("{}.endpoint", provider_str)).is_some()
                                    || ai.get("endpoint_env").is_some();
                            let has_model = ai.get(format!("{}.model", provider_str)).is_some()
                                || ai.get("model_env").is_some();

                            if !has_endpoint && provider_str != "ollama" {
                                return Err(anyhow!(
                                    "AI provider {} requires endpoint configuration",
                                    provider_str
                                ));
                            }
                            if !has_model {
                                return Err(anyhow!(
                                    "AI provider {} requires model configuration",
                                    provider_str
                                ));
                            }
                        }
                    }
                } else {
                    return Err(anyhow!("AI is enabled but no provider specified"));
                }
            }
        }
    }

    Ok(())
}

/// Quick validation for common configuration issues
#[cfg_attr(not(test), allow(dead_code))]
pub fn quick_validate_toml(content: &str) -> Result<Vec<String>> {
    let mut issues = Vec::new();

    // Check for common TOML syntax issues
    if content.contains("=\n") || content.contains("= \n") {
        issues.push("Found incomplete assignments (= followed by newline)".to_string());
    }

    if content.matches('[').count() != content.matches(']').count() {
        issues.push("Unmatched square brackets in TOML".to_string());
    }

    if content.matches('{').count() != content.matches('}').count() {
        issues.push("Unmatched curly braces in TOML".to_string());
    }

    // Check for common indentation issues (TOML doesn't use indentation like YAML)
    for (line_num, line) in content.lines().enumerate() {
        if line.starts_with("  ") && !line.trim_start().starts_with('#') {
            issues.push(format!(
                "Line {}: TOML doesn't use indentation (found leading spaces)",
                line_num + 1
            ));
        }
    }

    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_is_valid_color() {
        assert!(is_valid_color("#ff0000"));
        assert!(is_valid_color("#f00"));
        assert!(is_valid_color("#FF0000"));
        assert!(is_valid_color("red"));
        assert!(is_valid_color("BLACK"));

        assert!(!is_valid_color("invalid"));
        assert!(!is_valid_color("#gg0000"));
        assert!(!is_valid_color("##ff0000"));
    }

    #[test]
    fn test_quick_validate_toml() {
        let valid_toml = r#"
[window]
opacity = 0.9
"#;
        assert!(quick_validate_toml(valid_toml).unwrap().is_empty());

        let invalid_toml = r#"
[window
opacity = 
"#;
        let issues = quick_validate_toml(invalid_toml).unwrap();
        assert!(!issues.is_empty());
    }

    #[test]
    fn test_validate_font_section() {
        // Valid font
        let font = toml::from_str(
            r#"
size = 12.0
[normal]
family = "JetBrains Mono"
"#,
        )
        .unwrap();
        assert!(validate_font_section(&font).is_ok());

        // Invalid font size
        let invalid_font = toml::from_str(
            r#"
size = -5.0
"#,
        )
        .unwrap();
        assert!(validate_font_section(&invalid_font).is_err());
    }

    #[test]
    fn test_validate_window_section() {
        // Valid window
        let window = toml::from_str(
            r#"
opacity = 0.9
decorations = "full"
"#,
        )
        .unwrap();
        assert!(validate_window_section(&window).is_ok());

        // Invalid opacity
        let invalid_window = toml::from_str(
            r#"
opacity = 1.5
"#,
        )
        .unwrap();
        assert!(validate_window_section(&invalid_window).is_err());
    }

    #[test]
    fn test_validate_config_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"
[general]
live_config_reload = true

[font]
size = 12.0

[font.normal]
family = "JetBrains Mono"

[window]
opacity = 0.9
"#
        )
        .unwrap();

        let result = validate_config(temp_file.path());
        assert!(result.is_ok());
    }
}
