//! Utility functions for OpenAgent Terminal
//!
//! This crate consolidates utility functionality including:
//! - Theme management and loading
//! - Code snippets and templates
//! - Migration tools for configuration and data

#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

#[cfg(feature = "themes")]
pub mod themes;

#[cfg(feature = "snippets")]
pub mod snippets;

#[cfg(feature = "migrate")]
pub mod migrate;

/// Common utility error types
#[derive(Debug, thiserror::Error)]
pub enum UtilsError {
    #[error("Theme error: {0}")]
    Theme(String),

    #[error("Snippet error: {0}")]
    Snippet(String),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

pub type UtilsResult<T> = Result<T, UtilsError>;

/// Common utility configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UtilsConfig {
    pub enable_themes: bool,
    pub enable_snippets: bool,
    pub enable_migration: bool,
    pub theme_directory: Option<std::path::PathBuf>,
    pub snippet_directory: Option<std::path::PathBuf>,
}

impl Default for UtilsConfig {
    fn default() -> Self {
        Self {
            enable_themes: true,
            enable_snippets: true,
            enable_migration: false,
            theme_directory: None,
            snippet_directory: None,
        }
    }
}
