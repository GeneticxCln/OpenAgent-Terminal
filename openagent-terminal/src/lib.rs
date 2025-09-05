//! Library interface for OpenAgent Terminal modules used by examples and tests.
#![warn(rust_2018_idioms, future_incompatible)]
#![warn(clippy::all, clippy::if_not_else, clippy::enum_glob_use)]

// Re-export SerdeReplace at crate root so config derive macros can refer to `crate::SerdeReplace`.
pub use crate::config::monitor::ConfigMonitor;
pub use openagent_terminal_config::SerdeReplace;

#[cfg(feature = "ai")]
pub mod ai_runtime;
#[cfg(feature = "ai")]
pub mod ai_context_provider;
pub mod cli;
pub mod clipboard;
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
pub mod security; // Feature-gated security module
#[cfg(feature = "security-lens")]
pub use security::security_lens as security_lens;
#[cfg(not(feature = "security-lens"))]
pub use security::stub as security_lens;
pub mod text_shaping;
pub mod ui_confirm;
pub mod workspace;

// Internal GL bindings used by display/render paths.
pub mod gl {
    #![allow(clippy::all, unsafe_op_in_unsafe_fn)]
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}
