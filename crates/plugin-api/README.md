# Plugin API

This crate defines the minimal, versioned API for plugins.

Versioning

- Crate version (SemVer) communicates compatibility
- Minor versions may add optional interfaces; patch versions fix bugs only
- Major versions introduce breaking changes; see the root CHANGELOG for migration notes

Stability expectations

- The core types and traits used by the current examples are considered stable
- Experimental additions are added behind feature flags where appropriate

Getting started

- See examples/plugins/hello-wasi for a minimal sample
- Host integration lives in crates/plugin-loader

Changelog

- See the repository-level CHANGELOG.md for updates affecting the plugin API

## Storage (Preview)

Per-plugin, namespaced key-value storage for small configuration and state.

- Isolation: each plugin can only access its own namespace
- Quotas: host-enforced limits on total size, value size, and number of keys
- Permissions: requires the `storage` permission in the plugin manifest

Manifest snippet
```toml
[plugin.capabilities]
types = ["ai_provider"]
permissions = ["storage"]
```

SDK usage (subject to change)
```rust
// Requires enabling the `storage` feature in the SDK/crate when available
use openagent_terminal_plugin::storage::Storage;

fn save_theme() -> anyhow::Result<()> {
    let storage = Storage::new()?;
    storage.put("settings/theme", b"dark")?;
    Ok(())
}

fn read_theme() -> anyhow::Result<Option<Vec<u8>>> {
    let storage = Storage::new()?;
    let bytes = storage.get("settings/theme")?;
    Ok(bytes)
}
```

Raw host calls (C-ABI) may be used by advanced plugins; see ADR-003 for signatures and semantics.
