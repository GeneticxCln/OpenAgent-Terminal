use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, ValueEnum, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TerminalType {
    /// Alacritty terminal emulator
    Alacritty,
    /// iTerm2 for macOS
    #[value(name = "iterm2")]
    ITerm2,
    /// Windows Terminal
    #[value(name = "windows-terminal")]
    WindowsTerminal,
    /// Kitty terminal
    Kitty,
    /// Hyper terminal
    Hyper,
    /// Warp terminal
    Warp,
    /// WezTerm
    #[value(name = "wezterm")]
    WezTerm,
    /// GNOME Terminal
    #[value(name = "gnome-terminal")]
    GnomeTerminal,
    /// Konsole (KDE)
    Konsole,
    /// Terminator
    Terminator,
    /// Tilix
    Tilix,
    /// Tabby (formerly Terminus)
    Tabby,
}

impl TerminalType {
    /// Get all supported terminal types
    pub fn all() -> Vec<TerminalType> {
        vec![
            TerminalType::Alacritty,
            TerminalType::ITerm2,
            TerminalType::WindowsTerminal,
            TerminalType::Kitty,
            TerminalType::Hyper,
            TerminalType::Warp,
            TerminalType::WezTerm,
            TerminalType::GnomeTerminal,
            TerminalType::Konsole,
            TerminalType::Terminator,
            TerminalType::Tilix,
            TerminalType::Tabby,
        ]
    }

    /// Get typical file extensions for this terminal's config
    #[allow(dead_code)]
    pub fn config_extensions(&self) -> Vec<&'static str> {
        match self {
            TerminalType::Alacritty => vec!["yml", "yaml", "toml"],
            TerminalType::ITerm2 => vec!["plist", "json"],
            TerminalType::WindowsTerminal => vec!["json"],
            TerminalType::Kitty => vec!["conf"],
            TerminalType::Hyper => vec!["js", "json"],
            TerminalType::Warp => vec!["yaml", "yml"],
            TerminalType::WezTerm => vec!["lua", "toml"],
            TerminalType::GnomeTerminal => vec!["dconf"], // Special case
            TerminalType::Konsole => vec!["profile"],
            TerminalType::Terminator => vec!["config"],
            TerminalType::Tilix => vec!["dconf"], // Special case
            TerminalType::Tabby => vec!["yaml", "yml", "json"],
        }
    }

    /// Check if this terminal is typically available on the current platform
    pub fn is_platform_compatible(&self) -> bool {
        match self {
            TerminalType::ITerm2 => cfg!(target_os = "macos"),
            TerminalType::WindowsTerminal => cfg!(target_os = "windows"),
            TerminalType::GnomeTerminal
            | TerminalType::Konsole
            | TerminalType::Terminator
            | TerminalType::Tilix => cfg!(target_os = "linux"),
            _ => true, // Cross-platform terminals
        }
    }

    /// Get the typical config file/directory names for this terminal
    #[allow(dead_code)]
    pub fn config_names(&self) -> Vec<&'static str> {
        match self {
            TerminalType::Alacritty => vec!["alacritty.yml", "alacritty.yaml", "alacritty.toml"],
            TerminalType::ITerm2 => vec!["com.googlecode.iterm2.plist", "DynamicProfiles"],
            TerminalType::WindowsTerminal => vec!["settings.json"],
            TerminalType::Kitty => vec!["kitty.conf"],
            TerminalType::Hyper => vec![".hyper.js", "hyper.json"],
            TerminalType::Warp => vec!["prefs.yaml", "settings.yaml"],
            TerminalType::WezTerm => vec!["wezterm.lua", "wezterm.toml"],
            TerminalType::GnomeTerminal => vec!["gnome-terminal"],
            TerminalType::Konsole => vec!["konsolerc"],
            TerminalType::Terminator => vec!["config"],
            TerminalType::Tilix => vec!["tilix.dconf"],
            TerminalType::Tabby => vec!["config.yaml", "config.yml"],
        }
    }

    /// Get description of what this terminal is
    #[allow(dead_code)]
    pub fn description(&self) -> &'static str {
        match self {
            TerminalType::Alacritty => "A cross-platform, OpenGL terminal emulator",
            TerminalType::ITerm2 => "Terminal emulator for macOS",
            TerminalType::WindowsTerminal => "Microsoft's modern terminal for Windows",
            TerminalType::Kitty => "Cross-platform, fast, feature-rich, GPU based terminal",
            TerminalType::Hyper => "A terminal built on web technologies",
            TerminalType::Warp => "A modern, Rust-based terminal with AI features",
            TerminalType::WezTerm => "A GPU-accelerated cross-platform terminal emulator",
            TerminalType::GnomeTerminal => "The GNOME desktop environment terminal",
            TerminalType::Konsole => "KDE's terminal emulator",
            TerminalType::Terminator => "Multiple GNOME terminals in one window",
            TerminalType::Tilix => "A tiling terminal emulator for Linux",
            TerminalType::Tabby => "A highly configurable terminal emulator",
        }
    }
}

impl fmt::Display for TerminalType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            TerminalType::ITerm2 => "iTerm2",
            TerminalType::WindowsTerminal => "Windows Terminal",
            TerminalType::WezTerm => "WezTerm",
            TerminalType::GnomeTerminal => "GNOME Terminal",
            _ => {
                // Convert enum variant to string and capitalize first letter
                let s = format!("{:?}", self);
                return write!(f, "{}", s);
            }
        };
        write!(f, "{}", name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_type_display() {
        assert_eq!(TerminalType::Alacritty.to_string(), "Alacritty");
        assert_eq!(TerminalType::ITerm2.to_string(), "iTerm2");
        assert_eq!(TerminalType::WindowsTerminal.to_string(), "Windows Terminal");
    }

    #[test]
    fn test_config_extensions() {
        let alacritty_exts = TerminalType::Alacritty.config_extensions();
        assert!(alacritty_exts.contains(&"yml"));
        assert!(alacritty_exts.contains(&"yaml"));
        assert!(alacritty_exts.contains(&"toml"));
    }

    #[test]
    fn test_all_terminals_count() {
        let all = TerminalType::all();
        assert!(all.len() >= 12); // Should have at least 12 supported terminals
    }
}
