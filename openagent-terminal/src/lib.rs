//! Library interface for OpenAgent Terminal modules used by examples and tests.
#![warn(rust_2018_idioms, future_incompatible)]
#![warn(clippy::all, clippy::if_not_else, clippy::enum_glob_use)]

// Re-export SerdeReplace at crate root so config derive macros can refer to `crate::SerdeReplace`.
pub use crate::config::monitor::ConfigMonitor;
pub use openagent_terminal_config::SerdeReplace;

#[cfg(feature = "ai")]
pub mod ai_context_provider;
#[cfg(feature = "ai")]
pub mod ai_runtime;
pub mod cli;
pub mod clipboard;
pub mod command_history;
pub mod config;
pub mod daemon;
pub mod display;
pub mod event;
pub mod input;
#[cfg(unix)]
pub mod ipc;
pub mod logging;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod message_bar;
pub mod migrate;
#[cfg(windows)]
pub mod panic;
pub mod renderer;
pub mod scheduler;
#[cfg(feature = "blocks")]
pub mod storage;
pub mod string;
pub mod window_context;

// New component modules
pub mod blocks_v2;
pub mod components_init;
#[cfg(feature = "blocks")]
pub mod notebooks;
pub mod security; // Feature-gated security module
pub mod security_config;
#[cfg(feature = "security-lens")]
pub use security::security_lens;
#[cfg(not(feature = "security-lens"))]
pub use security::stub as security_lens;
pub mod text_shaping;
pub mod ui_confirm;
pub mod workspace;

#[cfg(feature = "completions")]
pub mod completions_spec;
