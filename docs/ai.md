# AI integration (optional, privacy-first, opt-in)

Status: scaffolding only. This interface is completely optional at build-time and runtime.

Build-time
- Disabled by default. Build with --features ai to include the interface plumbing.

Runtime
- Configure in the ai section of your config (openagent-terminal.toml):

```toml path=null start=null
[ai]
# Off by default
enabled = false
# Provider id (implementation-specific). Default: "null"
provider = "null"
# Environment variable names for secrets and endpoints. Values are never printed.
endpoint_env = "OPENAGENT_AI_ENDPOINT"
api_key_env = "OPENAGENT_AI_API_KEY"
model_env = "OPENAGENT_AI_MODEL"
# UI behavior
scratch_autosave = true
propose_max_commands = 10
# Hard safety: UI must never auto-run proposals
never_auto_run = true
```

Secrets handling
- Secrets must only be supplied via environment variables. Do not put secrets in config files.
- The application must read these env vars into memory without logging them, and never print them.

UX principles
- Commands are authored in a scratch buffer, not in the shell line.
- The AI produces proposals shown in a side panel. The UI must never auto-run them.
- The feature can be entirely disabled at build time and at runtime.

