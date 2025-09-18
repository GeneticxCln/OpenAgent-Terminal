# Security Policy

Thank you for helping keep OpenAgent Terminal and its users safe.

Reporting a Vulnerability
- Please email security reports to security@openagent.dev. If you prefer encrypted communication, request our PGP key in your first message.
- Include: affected version/commit, environment, detailed reproduction steps, impact assessment, and any suggested mitigations.
- We aim to acknowledge receipt within 48 hours and provide a status update within 5 business days.

Coordinated Disclosure
- We follow a responsible disclosure process. Once a fix is available and users have a reasonable update window, we will publish details in the release notes and a security advisory.
- If the vulnerability is actively exploited or extremely severe, we may accelerate public disclosure.

Supported Versions
- We currently support the most recent minor release line (e.g., v1.y.z).
- Security fixes will generally be applied to the latest release; backports may be considered for high/critical issues when feasible.

Scope
- Core terminal (rendering, input, PTY management)
- AI integration (providers, history, command suggestions)
- Plugin loader/runtime (WASM/WASI sandbox)
- Configuration and workspace management

Out of Scope Examples
- Third-party GPU/driver bugs
- Non-default system configurations outside documented support

Security Hardening Notes
- No telemetry; privacy-first defaults
- Secrets must be provided via environment variables
- Optional Security Lens feature for command risk analysis
- WASM plugins run in a sandbox; native plugins are not supported in v1.x

Credits
- We thank all reporters who follow responsible disclosure for helping improve project security.
