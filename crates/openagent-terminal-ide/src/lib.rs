//! IDE features for OpenAgent Terminal
//!
//! This crate consolidates IDE functionality including:
//! - LSP (Language Server Protocol) client
//! - Editor integration

#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

#[cfg(feature = "editor")]
pub mod editor;

#[cfg(feature = "lsp")]
pub mod lsp;



/// Common IDE error types
#[derive(Debug, thiserror::Error)]
pub enum IdeError {
    #[error("LSP error: {0}")]
    Lsp(String),

    #[error("Editor error: {0}")]
    Editor(String),


    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type IdeResult<T> = Result<T, IdeError>;

/// Common IDE configuration
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct IdeConfig {
    pub enable_lsp: bool,
    pub enable_editor: bool,
}
