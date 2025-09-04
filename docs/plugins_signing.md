# Plugin Signing and Verification (Warp-like)

This guide explains how to sign plugins and verify them in OpenAgent Terminal with a security posture similar to Warp Terminal: plugins must be signed by trusted publishers and verification is enforced by default in release builds.

1) Generate an ed25519 key pair (publisher)

- Using age-keygen (example) or any tool that outputs a raw 32-byte ed25519 public key in hex.

Example with OpenSSH key (convert to raw ed25519):
- Generate: ssh-keygen -t ed25519 -f publisher
- Convert the public key to raw 32-byte hex (varies by tooling; you can use a small script to extract and hex-encode the raw key).
- Place the hex bytes into a file like openagent_official.pub (no prefix, no 0x, just hex).

2) Sign the plugin (publisher)

- Compute the SHA-256 digest of plugin.wasm and sign the digest using ed25519.
- Write the signature bytes (64-byte ed25519 signature) as lowercase hex to plugin.sig and distribute it alongside plugin.wasm.

Pseudocode:
- digest = sha256(plugin.wasm)
- signature = ed25519_sign(private_key, digest)
- hex(signature) > plugin.sig

3) Install trusted keys (user)

- Place trusted publisher keys in:
  ~/.config/openagent-terminal/trusted_keys/*.pub (each file contains raw 32-byte ed25519 public key as lowercase hex)

- Or use the CLI helper:
  plugin-sdk-cli add-key <hex|file>

4) Verify manually (optional)

- plugin-sdk-cli verify <plugin.wasm> [--sig plugin.sig]
  Prints Signature: OK if any trusted key verifies the signature over the WASM’s SHA-256 digest.

5) Enforcement in release builds (Warp-like)

- Use example_release_config.toml to enforce signatures strictly:
  [plugins]
  enforce_signatures = true
  require_signatures_for_all = true
  hot_reload = false

  [plugins.paths.system]
  require_signatures = true
  [plugins.paths.user]
  require_signatures = true
  [plugins.paths.project]
  require_signatures = true

- With this policy, all plugins must be signed; unsigned or invalidly signed plugins are rejected.

Environment overrides (CI/releases)

- OPENAGENT_PLUGINS_REQUIRE_ALL=1|true — enforce signatures for all plugins at runtime
- OPENAGENT_PLUGINS_HOT_RELOAD=0|false — disable hot reload at runtime
- OPENAGENT_PLUGINS_USER_REQUIRE_SIGNED=1|true — require signatures in user dir
- OPENAGENT_PLUGINS_PROJECT_REQUIRE_SIGNED=1|true — require signatures in project dir

6) Bundling an official key (recommended)

- Ship an “OpenAgent official” public key in your package and install it to trusted_keys during post-install:
  mkdir -p ~/.config/openagent-terminal/trusted_keys
  cp openagent_official.pub ~/.config/openagent-terminal/trusted_keys/

- Alternatively, provide a first-run script or installer that prompts the user to install the publisher key.

7) Directory policies and hot reload

- For developer builds:
  - You can relax user/project paths and enable hot_reload = true for faster iteration.
- For releases:
  - Keep all paths strict and hot_reload = false for deterministic behavior.

8) Notes on format and safety

- Keys: raw ed25519 public key (32 bytes) as hex, no prefix; one per .pub file.
- Signatures: ed25519 signature bytes (64 bytes) as hex.
- Verification: loader verifies the signature over the WASM’s SHA-256 digest.
- WASI sandbox and permission checks still apply even when signatures verify.

