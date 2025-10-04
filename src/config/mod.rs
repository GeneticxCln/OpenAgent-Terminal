// Configuration module for OpenAgent-Terminal
//
// Provides TOML-based configuration with sensible defaults.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Complete configuration for OpenAgent-Terminal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Terminal-specific settings
    pub terminal: TerminalConfig,
    
    /// Agent-specific settings
    pub agent: AgentConfig,
    
    /// Keyboard shortcuts
    pub keybindings: Keybindings,
    
    /// Tool execution settings
    pub tools: ToolsConfig,
}

/// Terminal display and rendering settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Font family name
    pub font_family: String,
    
    /// Font size in points
    pub font_size: u16,
    
    /// Color theme name
    pub theme: String,
    
    /// Number of lines to keep in scrollback buffer
    pub scrollback_lines: u32,
    
    /// Enable syntax highlighting in blocks
    pub syntax_highlighting: bool,
}

/// AI agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Model to use (e.g., "mock", "gpt-4", "claude-3")
    pub model: String,
    
    /// Enable automatic command suggestions
    pub auto_suggest: bool,
    
    /// Require approval for all tool executions
    pub require_approval: bool,
    
    /// Maximum tokens per query
    pub max_tokens: u32,
    
    /// Temperature for LLM sampling (0.0 - 2.0)
    pub temperature: f32,
}

/// Keyboard shortcut configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keybindings {
    /// Toggle AI pane
    pub toggle_ai: String,
    
    /// Send query to AI
    pub send_query: String,
    
    /// Cancel current operation
    pub cancel: String,
    
    /// Clear screen
    pub clear_screen: String,
    
    /// Show command history
    pub show_history: String,
}

/// Tool execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    /// Enable real file operations (vs demo mode)
    pub enable_real_execution: bool,
    
    /// Directories where tools are allowed to operate
    pub safe_directories: Vec<String>,
    
    /// Timeout for shell commands in seconds
    pub command_timeout: u64,
}

impl Config {
    /// Load configuration from file, or use defaults if not found
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        
        if config_path.exists() {
            log::info!("Loading config from: {:?}", config_path);
            let contents = std::fs::read_to_string(&config_path)
                .context("Failed to read config file")?;
            let config: Config = toml::from_str(&contents)
                .context("Failed to parse config file")?;
            Ok(config)
        } else {
            log::info!("No config file found, using defaults");
            Ok(Self::default())
        }
    }
    
    /// Load configuration from a specific path
    #[allow(dead_code)] // Will be used when CLI args are added
    pub fn load_from(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        log::info!("Loading config from: {:?}", path);
        let contents = std::fs::read_to_string(&path)
            .context("Failed to read config file")?;
        let config: Config = toml::from_str(&contents)
            .context("Failed to parse config file")?;
        Ok(config)
    }
    
    /// Save configuration to file
    #[allow(dead_code)] // Will be used for config generation
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        
        // Create config directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create config directory")?;
        }
        
        let contents = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;
        std::fs::write(&config_path, contents)
            .context("Failed to write config file")?;
        
        log::info!("Saved config to: {:?}", config_path);
        Ok(())
    }
    
    /// Get the path to the configuration file
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?;
        Ok(config_dir.join("openagent-terminal").join("config.toml"))
    }
    
    /// Generate and save a default configuration file
    #[allow(dead_code)] // Will be used via CLI command
    pub fn generate_default() -> Result<()> {
        let config = Self::default();
        config.save()?;
        println!("Generated default config at: {:?}", Self::config_path()?);
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            terminal: TerminalConfig::default(),
            agent: AgentConfig::default(),
            keybindings: Keybindings::default(),
            tools: ToolsConfig::default(),
        }
    }
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            font_family: "DejaVu Sans Mono".to_string(),
            font_size: 14,
            theme: "monokai".to_string(),
            scrollback_lines: 10000,
            syntax_highlighting: true,
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "mock".to_string(),
            auto_suggest: true,
            require_approval: true,
            max_tokens: 2000,
            temperature: 0.7,
        }
    }
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            toggle_ai: "Ctrl+A".to_string(),
            send_query: "Enter".to_string(),
            cancel: "Ctrl+C".to_string(),
            clear_screen: "Ctrl+K".to_string(),
            show_history: "Ctrl+L".to_string(),
        }
    }
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            enable_real_execution: false, // Safe default
            safe_directories: vec![
                "~".to_string(), // User home
                ".".to_string(), // Current directory
            ],
            command_timeout: 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.terminal.font_size, 14);
        assert_eq!(config.agent.model, "mock");
        assert!(!config.tools.enable_real_execution);
    }
    
    #[test]
    fn test_serialize_deserialize() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.terminal.font_family, config.terminal.font_family);
    }
    
    #[test]
    fn test_config_path() {
        let path = Config::config_path().unwrap();
        assert!(path.to_str().unwrap().contains("openagent-terminal"));
        assert!(path.to_str().unwrap().ends_with("config.toml"));
    }
}
