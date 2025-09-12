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
# Examples: "null", "local_fs", "secure"
provider = "null"
# Environment variable names for endpoints/secrets
endpoint_env = "OPENAGENT_SYNC_ENDPOINT"
# Which environment variable to read the encryption password from.
# If unset, defaults to OPENAGENT_SYNC_PASSWORD at runtime.
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

Key management (Ed25519)
- On first use of the secure sync provider (provider = "secure"), an Ed25519 keypair is generated per installation.
- Private key is written with restrictive permissions (0600 on Unix):
  - $XDG_STATE_HOME/openagent-terminal/secure-sync/keys/ed25519_private.pk8
- Public key is stored as:
  - $XDG_STATE_HOME/openagent-terminal/secure-sync/keys/ed25519_public.bin
  - Also embedded in $XDG_STATE_HOME/openagent-terminal/secure-sync/installation.json
- If XDG_STATE_HOME is not set, the state directory defaults to ~/.local/state.

Encrypted payload password
- Encrypted sync payloads require a password at runtime, read from an environment variable.
- The variable name is taken from [sync].encryption_key_env. If that is not set, the system falls back to OPENAGENT_SYNC_PASSWORD.
- Do not store this password in configuration files.

Peer authentication (handshake)
- Peers authenticate using a challenge/response flow with Ed25519 signatures and domain separation.
- Flow:
  1) A generates a 32-byte random challenge for B (includes A's installation_id and timestamp).
  2) B responds with a signature over: "openagent-terminal.secure-sync.handshake.v1" | from_id | to_id | challenge.
  3) A verifies B's signature using the public key recorded for B.
- Known peers and their public keys are stored in $STATE/openagent-terminal/secure-sync/peers.json.
- Each peer has: installation_id, display_name, last_seen, public_key (raw Ed25519 bytes), capabilities.

Storage layout (secure provider)
- Base directory (STATE):
  - $XDG_STATE_HOME/openagent-terminal/secure-sync
  - or ~/.local/state/openagent-terminal/secure-sync
- Files:
  - installation.json            # installation_id, KDF params, sync_version, public_key
  - keys/ed25519_private.pk8     # private key (0600 on Unix)
  - keys/ed25519_public.bin      # public key
  - peers.json                   # trust store of known peers
  - encrypted/<scope>/<timestamp>.json  # encrypted payloads per scope

Security notes
- Private keys are stored on disk with 0600 permissions on Unix. For stronger protection, integrating system keyrings/Keychain/DPAPI is recommended in future iterations.
- Handshake includes domain separation to prevent cross-protocol signature reuse.
- Challenge values are random per handshake; design transport to reject stale challenges and consider replay windows if necessary.
- KDF parameters and per-install salt are stored alongside metadata to support decryption and key derivation portability.
