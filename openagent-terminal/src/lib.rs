//! Library interface for OpenAgent Terminal modules used by examples and tests.
#![warn(warnings)]
#![warn(rust_2018_idioms, future_incompatible)]
#![warn(clippy::all, clippy::if_not_else, clippy::enum_glob_use)]
#![allow(
    clippy::pedantic,
    clippy::similar_names,
    clippy::unnested_or_patterns,
    clippy::needless_raw_string_hashes,
    clippy::unreadable_literal,
    clippy::redundant_else,
    clippy::many_single_char_names
)]

// Re-export SerdeReplace at crate root so config derive macros can refer to `crate::SerdeReplace`.
pub use crate::config::monitor::ConfigMonitor;
pub use openagent_terminal_config::SerdeReplace;

pub mod cli;
pub mod clipboard;
pub mod command_history;
pub mod command_pipeline;
pub mod config;
pub mod daemon;
pub mod display;
pub mod event;
pub mod ide;
pub mod input;
#[cfg(unix)]
pub mod ipc;
pub mod native_search;
pub mod logging;
pub mod shell_integration;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod message_bar;
pub mod migrate;
#[cfg(windows)]
pub mod panic;
pub mod renderer;
pub mod scheduler;
pub mod string;
pub mod utils;
pub mod window_context;

// New component modules
pub mod components_init;
pub mod text_shaping;
pub mod ui_confirm;
pub mod workspace;

// Essential modules for core functionality
pub mod blocks_v2;
pub mod security_lens;

#[cfg(feature = "completions")]
pub mod completions_spec;
