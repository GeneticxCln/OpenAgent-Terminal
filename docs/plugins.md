# Plugins

This project exposes a minimal, versioned plugin API for extending OpenAgent Terminal with additional capabilities. The goals are:

- Stability: a small, well-documented surface area with clear versioning
- Safety: sandboxed execution (e.g., WASI) for third-party code where possible
- Simplicity: quick-start examples that are easy to copy and adapt

Quick start (WASI “Hello” plugin)

- Start here: docs/guides/first_plugin.md (step-by-step walkthrough)
- Example: examples/plugins/hello-wasi
- Build: cargo build -p examples/plugins/hello-wasi --release
- Load: place the resulting .wasm in your plugin directory and register it in config

API versioning and stability

- The Rust crate crates/plugin-api is versioned independently (SemVer)
- Backwards-compatible changes (minor) may add new capabilities while preserving existing interfaces
- Breaking changes (major) are rare and announced in the changelog; a migration section will accompany them

Current status

- MVP in progress: Wasmtime runtime path is prioritized (WASM only); native plugins are not supported yet
- JSON-over-memory ABI implemented (metadata + event handling)
- Host-side loader at crates/plugin-loader with permission enforcement and event dispatch
- Multi-location discovery: system (/usr/share/openagent-terminal/plugins), user (~/.config/openagent-terminal/plugins), project (./plugins), and data-dir
- Hot reloading (polling-based): detects added/modified/removed plugins and reloads at runtime
- Optional signing/verification: if <plugin>.sig is present (ed25519, hex), verify against trusted keys in ~/.config/openagent-terminal/trusted_keys
- Plugin SDK exports `plugin_alloc` and `plugin_get_last_response` for ergonomics
- Host persistent storage functions available to plugins (`host_store_data`/`host_retrieve_data`) with SDK helpers `store_data`/`retrieve_data`
- Broadcast API: host can send an event to all plugins
- Example WASI plugin at examples/plugins/hello-wasi (demonstrates setting a response)

Threat model and directory policies

- Execution model: WASI sandbox via Wasmtime. No direct syscalls; only WASI preview1 functions exposed. Threads are disabled; SIMD and bulk memory enabled. Epoch-based interruption limits CPU.
- Allowed WASI calls (effective): fd_read/fd_write/fd_close, fd_fdstat_get, clock_time_get, random_get, path_open under preopened roots only, and other safe preview1 calls required by Wasmtime’s WASI. No arbitrary syscalls or raw sockets.
- File system caps: Plugins are confined to preopened directories. By default, only the plugin’s directory is preopened; additional preopens must be explicitly listed in the manifest and are sanitized to stay under the plugin directory. Dangerous system paths and traversal are blocked.
- Environment: Plugins receive only explicitly allowed environment variables from the manifest. Sensitive prefixes (AWS_, TOKEN_, SECRET_, KEY_, PASSWORD_, SSH_, GPG_, etc.) and sensitive exact names (HOME, USER, PATH, LD_LIBRARY_PATH, SUDO_USER, LOGNAME) are blocked unless explicitly allowed by the host.
- Network: Disabled by default. No direct sockets via WASI; future networking would require an audited host API.
- Command execution: Disabled by default. If permitted in the manifest and allowed by host policy, execution is funneled through a controlled host function that returns structured output.
- Resource limits: Linear memory growth is bounded (default ~50MB). Timeouts are enforced via epoch deadlines around host-ABI calls. Manifests are validated for reasonable memory/timeout limits.
- Auditability: All host calls can be logged; consider enabling structured tracing in dev builds.

Directory signature policy (matches example_config defaults):

- System plugins directory (e.g., /usr/share/openagent-terminal/plugins): signatures required.
- User plugins directory (~/.config/openagent-terminal/plugins): signatures optional by default (recommended to require in production).
- Project plugins directory (./plugins): signatures optional by default (developer-friendly).
- Strict releases: set require_signatures_for_all = true and disable hot_reload. See docs/plugins_signing.md.

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
