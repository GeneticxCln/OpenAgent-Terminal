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
pub mod command_assistance;
pub mod conversation_management;
#[cfg(feature = "demos")]
pub mod conversation_demo;
#[cfg(feature = "demos")]
pub mod complete_integration_demo;
#[cfg(feature = "demos")]
pub mod block_sharing;
#[cfg(feature = "demos")]
pub mod block_sharing_demo;
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
pub mod ui_confirm;
pub mod components_init;

// Expose AI module for integration tests and external consumers
pub mod ai;

// Expose text shaping module for library consumers and internal modules (e.g., renderer/text/shaped_renderer)
pub mod text_shaping;

// New component modules
pub mod ai_context_provider;
pub mod ai_runtime;
#[cfg(feature = "demos")]
pub mod ai_event_integration;
#[cfg(feature = "demos")]
pub mod terminal_event_bridge;
#[cfg(feature = "demos")]
pub mod ai_terminal_integration;
pub mod workspace;

// Essential modules for core functionality
pub mod blocks_v2;
pub mod security_lens;
pub mod session_persistence;
pub mod session_service;
pub mod session_cli;
pub mod storage;

#[cfg(feature = "completions")]
pub mod completions_spec;
