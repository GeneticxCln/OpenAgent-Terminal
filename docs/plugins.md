# Plugins

This project exposes a minimal, versioned plugin API for extending OpenAgent Terminal with additional capabilities. The goals are:

- Stability: a small, well-documented surface area with clear versioning
- Safety: sandboxed execution (e.g., WASI) for third-party code where possible
- Simplicity: quick-start examples that are easy to copy and adapt

Quick start (WASI “Hello” plugin)

- Example: examples/plugins/hello-wasi
- Build: cargo build -p examples/plugins/hello-wasi --release
- Load: place the resulting .wasm in your plugin directory and register it in config

API versioning and stability

- The Rust crate crates/plugin-api is versioned independently (SemVer)
- Backwards-compatible changes (minor) may add new capabilities while preserving existing interfaces
- Breaking changes (major) are rare and announced in the changelog; a migration section will accompany them

Current status

- JSON-over-memory ABI implemented (metadata + event handling)
- Host-side loader at crates/plugin-loader with permission enforcement and event dispatch
- Multi-location discovery: system (/usr/share/openagent-terminal/plugins), user (~/.config/openagent-terminal/plugins), project (./plugins), and data-dir
- Hot reloading (polling-based): detects added/modified/removed plugins and reloads at runtime
- Optional signing/verification: if <plugin>.sig is present (ed25519, hex), verify against trusted keys in ~/.config/openagent-terminal/trusted_keys
- Plugin SDK exports `plugin_alloc` and `plugin_get_last_response` for ergonomics
- Host persistent storage functions available to plugins (`host_store_data`/`host_retrieve_data`) with SDK helpers `store_data`/`retrieve_data`
- Broadcast API: host can send an event to all plugins
- Example WASI plugin at examples/plugins/hello-wasi (demonstrates setting a response)

Best practices

- Keep plugins focused on one task
- Avoid spawning external processes unless necessary
- Prefer streaming interfaces for long-running tasks

For detailed design notes see docs/adr/003-plugin-system.md

Release profile (strict, Warp-like)

- Use example_release_config.toml for a ready-to-use strict policy:
  - [plugins] enforce_signatures = true, require_signatures_for_all = true, hot_reload = false
  - [plugins.paths.*].require_signatures = true
- See docs/plugins_signing.md for signing/verification and key management.

Environment toggles (override at runtime)

- OPENAGENT_PLUGINS_REQUIRE_ALL=1|true — require signatures for all plugins (reject unsigned)
- OPENAGENT_PLUGINS_HOT_RELOAD=0|false — disable hot reload (recommended for releases)
- OPENAGENT_PLUGINS_USER_REQUIRE_SIGNED=1|true — require signatures in user plugins dir
- OPENAGENT_PLUGINS_PROJECT_REQUIRE_SIGNED=1|true — require signatures in project ./plugins
