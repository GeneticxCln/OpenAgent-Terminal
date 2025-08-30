# Settings/History Sync (optional, privacy-first, opt-in)

Status: scaffolding only. The sync system is completely optional at build-time and runtime.

Build-time
- Disabled by default. Build with --features sync to include the sync plumbing.

Runtime
- Configure in the sync section of your config (openagent-terminal.toml):

```toml path=null start=null
[sync]
# Off by default
enabled = false
# Provider id (implementation-specific). Default: "null"
provider = "null"
# Environment variable names for endpoints/secrets
endpoint_env = "OPENAGENT_SYNC_ENDPOINT"
encryption_key_env = "OPENAGENT_SYNC_KEY"
# Optional data dir for file-based sync
# data_dir = "/path/to/state"
# What to sync
sync_history = true
sync_settings = true
```

Principles
- Zero default telemetry. No background network requests when disabled.
- Secrets must only be supplied via environment variables. Do not put secrets in config files.
- The feature can be entirely disabled at build time and at runtime.

