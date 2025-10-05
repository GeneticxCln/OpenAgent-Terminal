// CLI argument parsing and configuration precedence
//
// Implements CLI > Environment > File precedence for configuration overrides

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// OpenAgent-Terminal: AI-Native Terminal Emulator
///
/// An intelligent terminal that combines traditional shell interaction with
/// AI-powered assistance for enhanced productivity.
#[derive(Parser, Debug)]
#[command(name = "openagent-terminal")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Path to Unix socket for IPC with Python backend
    ///
    /// Overrides OPENAGENT_SOCKET environment variable and default path.
    /// Default: $XDG_RUNTIME_DIR/openagent-terminal-test.sock
    #[arg(short, long, value_name = "PATH")]
    pub socket: Option<PathBuf>,

    /// Path to configuration file
    ///
    /// Overrides default config location.
    /// Default: $XDG_CONFIG_HOME/openagent-terminal/config.toml
    #[arg(short, long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Logging level for the application
    ///
    /// Controls verbosity of log output. Use 'trace' for maximum detail.
    #[arg(short, long, value_name = "LEVEL", value_enum)]
    pub log_level: Option<LogLevel>,

    /// Generate a default configuration file and exit
    ///
    /// Creates config.toml with default settings at the standard config location.
    /// Useful for initial setup or resetting configuration.
    #[arg(long)]
    pub generate_config: bool,

    /// AI model to use for queries
    ///
    /// Overrides model setting from config file.
    /// Examples: "mock", "gpt-4", "claude-3-opus"
    #[arg(short, long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Enable verbose output (equivalent to --log-level debug)
    #[arg(short, long)]
    pub verbose: bool,

    /// Suppress all output except errors (equivalent to --log-level error)
    #[arg(short, long)]
    pub quiet: bool,
}

/// Log level for the application
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LogLevel {
    /// Show all log messages (most verbose)
    Trace,
    /// Show debug, info, warning, and error messages
    Debug,
    /// Show info, warning, and error messages (default)
    Info,
    /// Show warning and error messages only
    Warn,
    /// Show error messages only
    Error,
    /// Suppress all log output
    Off,
}

impl LogLevel {
    /// Convert to log::LevelFilter
    pub fn to_level_filter(self) -> log::LevelFilter {
        match self {
            LogLevel::Trace => log::LevelFilter::Trace,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Off => log::LevelFilter::Off,
        }
    }

    /// Convert to string for env_logger filter
    pub fn to_filter_str(self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
            LogLevel::Off => "off",
        }
    }
}

impl Cli {
    /// Parse CLI arguments
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Get the effective log level from CLI args and flags
    ///
    /// Precedence: --quiet > --verbose > --log-level > default (info)
    pub fn effective_log_level(&self) -> LogLevel {
        if self.quiet {
            LogLevel::Error
        } else if self.verbose {
            LogLevel::Debug
        } else {
            self.log_level.unwrap_or(LogLevel::Info)
        }
    }

    /// Get socket path with precedence: CLI > Environment > Default
    pub fn effective_socket_path(&self) -> String {
        // CLI argument takes highest precedence
        if let Some(ref socket) = self.socket {
            return socket.to_string_lossy().to_string();
        }

        // Environment variable is second
        if let Ok(socket) = std::env::var("OPENAGENT_SOCKET") {
            return socket;
        }

        // Default path
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/openagent-terminal-test.sock", runtime_dir)
    }

    /// Get config path with precedence: CLI > Default
    pub fn effective_config_path(&self) -> Option<PathBuf> {
        self.config.clone()
    }

    /// Check if we should only generate config and exit
    pub fn should_generate_config(&self) -> bool {
        self.generate_config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_conversion() {
        assert_eq!(LogLevel::Info.to_filter_str(), "info");
        assert_eq!(LogLevel::Debug.to_filter_str(), "debug");
        assert_eq!(LogLevel::Error.to_filter_str(), "error");
    }

    #[test]
    fn test_effective_log_level() {
        // Test quiet flag takes precedence
        let cli = Cli {
            socket: None,
            config: None,
            log_level: Some(LogLevel::Debug),
            generate_config: false,
            model: None,
            verbose: false,
            quiet: true,
        };
        assert!(matches!(cli.effective_log_level(), LogLevel::Error));

        // Test verbose flag
        let cli = Cli {
            socket: None,
            config: None,
            log_level: None,
            generate_config: false,
            model: None,
            verbose: true,
            quiet: false,
        };
        assert!(matches!(cli.effective_log_level(), LogLevel::Debug));

        // Test explicit log level
        let cli = Cli {
            socket: None,
            config: None,
            log_level: Some(LogLevel::Trace),
            generate_config: false,
            model: None,
            verbose: false,
            quiet: false,
        };
        assert!(matches!(cli.effective_log_level(), LogLevel::Trace));
    }
}
