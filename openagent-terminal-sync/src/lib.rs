//! Sync interfaces for OpenAgent Terminal (optional, privacy-first).
//! This crate contains only traits and simple types; concrete implementations are pluggable.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod local_fs;
pub use local_fs::LocalFsProvider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncScope {
    Settings,
    History,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SyncStatus {
    pub last_push: Option<u64>,   // unix epoch seconds
    pub last_pull: Option<u64>,   // unix epoch seconds
    pub pending: bool,
}

#[derive(Debug)]
pub enum SyncError {
    Unsupported,
    Misconfigured(&'static str),
    Io(std::io::Error),
    Other(String),
}

impl From<std::io::Error> for SyncError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}

pub trait SyncProvider: Send + Sync {
    fn name(&self) -> &'static str;

    fn status(&self) -> Result<SyncStatus, SyncError> { Ok(SyncStatus::default()) }

    fn push(&self, _scope: SyncScope) -> Result<(), SyncError> { Ok(()) }

    fn pull(&self, _scope: SyncScope) -> Result<(), SyncError> { Ok(()) }
}

/// A no-op provider that satisfies the interface without performing any network I/O.
#[derive(Debug, Default)]
pub struct NullProvider;

impl SyncProvider for NullProvider {
    fn name(&self) -> &'static str { "null" }
}

/// Simple configuration passed in by the application layer (derived from user config/env).
#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub provider: String,              // e.g. "null", "fs", "http"
    pub data_dir: Option<PathBuf>,     // used by file-based providers
    pub endpoint_env: Option<String>,  // env var name for remote endpoint URL
    pub encryption_key_env: Option<String>, // env var name for encryption key
}

