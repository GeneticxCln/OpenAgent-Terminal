//! IDE features for OpenAgent Terminal
//!
//! This crate consolidates IDE functionality including:
//! - LSP (Language Server Protocol) client
//! - DAP (Debug Adapter Protocol) client  
//! - Editor integration
//! - Code indexing and search
//! - Web-based editors

#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

#[cfg(feature = "editor")]
pub mod editor;

#[cfg(feature = "lsp")]
pub mod lsp;

#[cfg(feature = "indexer")]
pub mod indexer;

#[cfg(feature = "dap")]
pub mod dap;

#[cfg(feature = "web-editors")]
pub mod web_editors;

#[cfg(feature = "web-editors")]
mod gtk4_ui;

/// Common IDE error types
#[derive(Debug, thiserror::Error)]
pub enum IdeError {
    #[error("LSP error: {0}")]
    Lsp(String),

    #[error("DAP error: {0}")]
    Dap(String),

    #[error("Editor error: {0}")]
    Editor(String),

    #[error("Indexer error: {0}")]
    Indexer(String),

    #[error("Web editor error: {0}")]
    WebEditor(String),

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
    pub enable_dap: bool,
    pub enable_editor: bool,
    pub enable_indexer: bool,
    pub enable_web_editors: bool,
}
