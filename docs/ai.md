# AI integration (optional, privacy-first, opt-in)

This interface is optional at build-time and runtime.

Build-time
- Disabled by default. Build with --features ai to include the interface plumbing.

Runtime
- Enabled by default when built with the ai feature. You can disable it via [ai].enabled = false.

Runtime configuration
- Configure in the ai section of your config (openagent-terminal.toml):

```toml path=null start=null
[ai]
# On by default when built with the ai feature
enabled = true
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

# Context collection for enriching AI requests
[ai.context]
# Enable/disable contextual enrichment (safe by default; sanitized before sending)
enabled = true
# Approximate size budget for all providers combined
max_bytes = 32768
# Providers to include in order: "env", "git", "file_tree"
providers = ["env", "git", "file_tree"]

[ai.context.timeouts]
# Soft per-provider timeout (providers run in parallel)
per_provider_ms = 150
# Overall deadline for context collection
overall_ms = 300
# Optional per-provider overrides (take precedence over per_provider_ms)
# env_ms = 100
# git_ms = 200
# file_tree_ms = 150

[ai.context.file_tree]
# Limit number of file entries listed (respects .gitignore)
max_entries = 500
# "git" = repo root when available; "cwd" = current working directory
root_strategy = "git"

[ai.context.git]
include_branch = true
include_status = true
```

Privacy & sanitization
- All AI requests are sanitized before leaving the process. Paths like HOME and the exact working directory are redacted by default.
- To tweak redaction behavior via environment variables:
  - OPENAGENT_AI_STRIP_SENSITIVE: default "1" (set to "0" to disable)
  - OPENAGENT_AI_STRIP_CWD: default "1" (set to "0" to disable)

Secrets handling
- Secrets must only be supplied via environment variables. Do not put secrets in config files.
- The application reads these env vars without logging them and never prints their values.

UX principles
- Commands are authored in a scratch buffer, not in the shell line.
- The AI produces proposals shown in a side panel. The UI never auto-runs them.
- The feature can be entirely disabled at build time and at runtime.

Conversation history (Preview)
- Optional, local-only persistence of AI chats to improve continuity across sessions.
- What is stored: role (user/assistant/system), content, timestamp, and optional metadata.
- Privacy: history never leaves your machine unless you explicitly use a cloud provider for requests. Outbound requests are sanitized.

Configuration

```toml
[storage]
# Enable or disable history persistence
enable_ai_history = true
# Retention window in days
ai_history_days = 90
# Encrypt sensitive data at rest (uses platform keyring when available)
encrypt_sensitive_data = true
```

Context windowing
- When composing a new request, the runtime may include a window of prior messages, bounded by your [ai.context] max_bytes and provider token limits.
- Disable enrichment entirely by setting [ai.context].enabled = false.

Export/delete
- UI/CLI affordances for exporting or deleting history are planned. You can disable persistence at any time with storage.enable_ai_history = false.
