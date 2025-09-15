# Privacy and UI Testing README

This document explains how to run privacy redaction property tests and UI buffer snapshot tests using only public APIs exposed in the workspace.

- Redaction property tests
  - Location: openagent-terminal-ai/src/privacy/proptest.rs
  - Run: cargo test -p openagent-terminal-ai -- --nocapture

- UI buffer snapshot tests
  - Location: openagent-terminal/tests/ui_buffer_snapshots.rs
  - Run: cargo test -p openagent-terminal ui_buffer
  - To update snapshots: set environment variable INSTA_UPDATE=auto and re-run tests

Notes
- No private/internal APIs are required; tests use exported types like Grid<Cell> and redact_secrets().
- Ensure Rust version matches workspace MSRV (Cargo.toml workspace.package.rust-version).
