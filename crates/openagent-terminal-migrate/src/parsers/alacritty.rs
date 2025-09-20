use crate::config::*;
use crate::parsers::ConfigParser;
use anyhow::Result;

pub struct AlacrittyParser;

impl AlacrittyParser {
    pub fn new() -> Self {
        Self
    }
}

impl ConfigParser for AlacrittyParser {
    fn parse(&self, content: &str) -> Result<UnifiedConfig> {
        // Try TOML first, then YAML
        let alacritty_config: serde_yaml::Value = if content.trim_start().starts_with('[') {
            // Looks like TOML
            let toml_value: toml::Value = toml::from_str(content)?;
            serde_yaml::to_value(toml_value)?
        } else {
            // Assume YAML
            serde_yaml::from_str(content)?
        };

        let mut config = UnifiedConfig::new();

        // Parse window settings
        if let Some(window) = alacritty_config.get("window") {
            config.window.opacity =
                window.get("opacity").and_then(|v| v.as_f64()).map(|v| v as f32);

            if let Some(padding) = window.get("padding") {
                let x = padding.get("x").and_then(|v| v.as_i64()).map(|v| v as i32);
                let y = padding.get("y").and_then(|v| v.as_i64()).map(|v| v as i32);
                if x.is_some() || y.is_some() {
                    config.window.padding = Some(PaddingConfig { x, y });
                }
            }

            config.window.decorations =
                window.get("decorations").and_then(|v| v.as_str()).map(|s| s.to_string());

            config.window.startup_mode =
                window.get("startup_mode").and_then(|v| v.as_str()).map(|s| s.to_string());

            config.window.title =
                window.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());

            config.window.dynamic_title = window.get("dynamic_title").and_then(|v| v.as_bool());
        }

        // Parse font settings
        if let Some(font) = alacritty_config.get("font") {
            config.font.size = font.get("size").and_then(|v| v.as_f64()).map(|v| v as f32);

            config.font.builtin_box_drawing =
                font.get("builtin_box_drawing").and_then(|v| v.as_bool());

            // Parse font faces
            if let Some(normal) = font.get("normal") {
                config.font.normal = Some(FontFaceConfig {
                    family: normal.get("family").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    style: normal.get("style").and_then(|v| v.as_str()).map(|s| s.to_string()),
                });
            }

            if let Some(bold) = font.get("bold") {
                config.font.bold = Some(FontFaceConfig {
                    family: bold.get("family").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    style: bold.get("style").and_then(|v| v.as_str()).map(|s| s.to_string()),
                });
            }

            if let Some(italic) = font.get("italic") {
                config.font.italic = Some(FontFaceConfig {
                    family: italic.get("family").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    style: italic.get("style").and_then(|v| v.as_str()).map(|s| s.to_string()),
                });
            }

            if let Some(bold_italic) = font.get("bold_italic") {
                config.font.bold_italic = Some(FontFaceConfig {
                    family: bold_italic
                        .get("family")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    style: bold_italic.get("style").and_then(|v| v.as_str()).map(|s| s.to_string()),
                });
            }
        }

        // Parse colors
        if let Some(colors) = alacritty_config.get("colors") {
            // Primary colors
            if let Some(primary) = colors.get("primary") {
                config.colors.primary = Some(PrimaryColors {
                    background: primary
                        .get("background")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    foreground: primary
                        .get("foreground")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    bright_foreground: primary
                        .get("bright_foreground")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    dim_foreground: primary
                        .get("dim_foreground")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                });
            }

            // Normal colors
            if let Some(normal) = colors.get("normal") {
                config.colors.normal = Some(NormalColors {
                    black: normal.get("black").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    red: normal.get("red").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    green: normal.get("green").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    yellow: normal.get("yellow").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    blue: normal.get("blue").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    magenta: normal.get("magenta").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    cyan: normal.get("cyan").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    white: normal.get("white").and_then(|v| v.as_str()).map(|s| s.to_string()),
                });
            }

            // Bright colors
            if let Some(bright) = colors.get("bright") {
                config.colors.bright = Some(BrightColors {
                    black: bright.get("black").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    red: bright.get("red").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    green: bright.get("green").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    yellow: bright.get("yellow").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    blue: bright.get("blue").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    magenta: bright.get("magenta").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    cyan: bright.get("cyan").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    white: bright.get("white").and_then(|v| v.as_str()).map(|s| s.to_string()),
                });
            }

            // Cursor colors
            if let Some(cursor) = colors.get("cursor") {
                config.colors.cursor = Some(CursorColors {
                    text: cursor.get("text").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    cursor: cursor.get("cursor").and_then(|v| v.as_str()).map(|s| s.to_string()),
                });
            }

            // Selection colors
            if let Some(selection) = colors.get("selection") {
                config.colors.selection = Some(SelectionColors {
                    text: selection.get("text").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    background: selection
                        .get("background")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                });
            }
        }

        // Parse scrolling settings
        if let Some(scrolling) = alacritty_config.get("scrolling") {
            config.terminal.scrolling = Some(ScrollingConfig {
                history: scrolling.get("history").and_then(|v| v.as_u64()).map(|v| v as u32),
                multiplier: scrolling.get("multiplier").and_then(|v| v.as_u64()).map(|v| v as u32),
            });
        }

        // Parse cursor settings
        if let Some(cursor) = alacritty_config.get("cursor") {
            let cursor_style = cursor.get("style").map(|style| CursorStyle {
                shape: style.get("shape").and_then(|v| v.as_str()).map(|s| s.to_string()),
                blinking: style.get("blinking").and_then(|v| v.as_str()).map(|s| s.to_string()),
            });

            let vi_mode_style = cursor.get("vi_mode_style").map(|vi_style| CursorStyle {
                shape: vi_style.get("shape").and_then(|v| v.as_str()).map(|s| s.to_string()),
                blinking: vi_style.get("blinking").and_then(|v| v.as_str()).map(|s| s.to_string()),
            });

            config.terminal.cursor = Some(CursorConfig {
                style: cursor_style,
                vi_mode_style,
                blinking: cursor.get("blinking").and_then(|v| v.as_str()).map(|s| s.to_string()),
                thickness: cursor.get("thickness").and_then(|v| v.as_f64()).map(|v| v as f32),
            });
        }

        // Parse selection settings
        if let Some(selection) = alacritty_config.get("selection") {
            config.terminal.selection = Some(SelectionConfig {
                semantic_escape_chars: selection
                    .get("semantic_escape_chars")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                save_to_clipboard: selection.get("save_to_clipboard").and_then(|v| v.as_bool()),
            });
        }

        // Parse mouse settings
        if let Some(mouse) = alacritty_config.get("mouse") {
            // Parse mouse bindings if present
            let bindings = mouse.get("bindings").and_then(|v| v.as_sequence()).map(|seq| {
                seq.iter()
                    .filter_map(|b| {
                        let mouse_btn = b.get("mouse").and_then(|v| v.as_str())?.to_string();
                        let action = b.get("action").and_then(|v| v.as_str())?.to_string();
                        let mods = b.get("mods").and_then(|v| v.as_str()).map(|s| {
                            s.split('|').map(|m| m.trim().to_string()).collect::<Vec<_>>()
                        });
                        let mode = b.get("mode").and_then(|v| v.as_str()).map(|s| s.to_string());
                        Some(MouseBinding { mouse: mouse_btn, action, mods, mode })
                    })
                    .collect::<Vec<_>>()
            });

            config.terminal.mouse = Some(MouseConfig {
                hide_when_typing: mouse.get("hide_when_typing").and_then(|v| v.as_bool()),
                bindings,
            });
        }

        // Parse shell settings
        if let Some(shell) = alacritty_config.get("shell") {
            if let Some(program) = shell.get("program") {
                config.shell.program = program.as_str().map(|s| s.to_string());
            }
            if let Some(args) = shell.get("args") {
                if let Some(args_array) = args.as_sequence() {
                    config.shell.args = Some(
                        args_array
                            .iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect(),
                    );
                }
            }
        }

        // Font family availability warning (if configured)
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

        // Parse key bindings
        if let Some(key_bindings) = alacritty_config.get("key_bindings") {
            if let Some(bindings_array) = key_bindings.as_sequence() {
                for binding in bindings_array {
                    if let Some(key) = binding.get("key").and_then(|v| v.as_str()) {
                        let mods = binding
                            .get("mods")
                            .and_then(|v| v.as_str())
                            .map(|s| s.split('|').map(|m| m.trim().to_string()).collect());

                        let command = if let Some(cmd) = binding.get("command") {
                            let program =
                                cmd.get("program").and_then(|v| v.as_str()).map(|s| s.to_string());
                            let args = cmd.get("args").and_then(|v| v.as_sequence()).map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str())
                                    .map(|s| s.to_string())
                                    .collect::<Vec<_>>()
                            });
                            program.map(|p| Command { program: p, args })
                        } else {
                            None
                        };

                        let key_binding = KeyBinding {
                            key: key.to_string(),
                            mods,
                            action: binding
                                .get("action")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            chars: binding
                                .get("chars")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            command,
                            mode: binding
                                .get("mode")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        };

                        config.key_bindings.push(key_binding);
                    }
                }
            }
        }

        Ok(config)
    }

    fn supports_file_extension(&self, extension: &str) -> bool {
        matches!(extension.to_lowercase().as_str(), "yml" | "yaml" | "toml")
    }

    fn parser_name(&self) -> &'static str {
        "Alacritty"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alacritty_yaml_parsing() {
        let yaml_content = r#"
window:
  opacity: 0.9
  padding:
    x: 10
    y: 10
  decorations: full

font:
  size: 12.0
  normal:
    family: "JetBrains Mono"
    style: Regular

colors:
  primary:
    background: '#1e1e2e'
    foreground: '#cdd6f4'
  normal:
    black: '#45475a'
    red: '#f38ba8'

scrolling:
  history: 10000

cursor:
  style:
    shape: Block
    blinking: On

shell:
  program: /bin/zsh
  args:
    - --login
"#;

        let parser = AlacrittyParser::new();
        let result = parser.parse(yaml_content);

        assert!(result.is_ok());
        let config = result.unwrap();

        assert_eq!(config.window.opacity, Some(0.9));
        assert_eq!(config.font.size, Some(12.0));
        assert_eq!(config.font.normal.as_ref().unwrap().family, Some("JetBrains Mono".to_string()));
        assert_eq!(config.colors.primary.as_ref().unwrap().background, Some("#1e1e2e".to_string()));
        assert_eq!(config.terminal.scrolling.as_ref().unwrap().history, Some(10000));
        assert_eq!(config.shell.program, Some("/bin/zsh".to_string()));
    }

    #[test]
    fn test_alacritty_toml_parsing() {
        let toml_content = r#"
[window]
opacity = 0.95
decorations = "full"

[window.padding]
x = 5
y = 5

[font]
size = 11.0

[font.normal]
family = "Fira Code"
style = "Regular"

[colors.primary]
background = '#282c34'
foreground = '#abb2bf'
"#;

        let parser = AlacrittyParser::new();
        let result = parser.parse(toml_content);

        assert!(result.is_ok());
        let config = result.unwrap();

        assert_eq!(config.window.opacity, Some(0.95));
        assert_eq!(config.font.size, Some(11.0));
        assert_eq!(config.font.normal.as_ref().unwrap().family, Some("Fira Code".to_string()));
    }

    #[test]
    fn test_supports_extensions() {
        let parser = AlacrittyParser::new();
        assert!(parser.supports_file_extension("yml"));
        assert!(parser.supports_file_extension("yaml"));
        assert!(parser.supports_file_extension("toml"));
        assert!(!parser.supports_file_extension("json"));
    }
}
