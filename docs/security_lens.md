# Security Lens and Confirmation Overlay

This document summarizes the Security Lens policy and the interactive confirmation overlay UX.

Prebuilt policy bundles
- policies/bundles/base.toml – balanced defaults suitable for most users
- policies/bundles/strict.toml – enterprise defaults (block critical, confirmations on risky)

Validate a policy
- In the terminal: `openagent-terminal security validate --policy policies/bundles/base.toml`
- Add `--json` to get a machine-readable report.

Org documentation links
- You can set docs_base_url to point to your organization’s internal guidance. Links shown in the overlay will be prefixed with this base URL.

Policy (UiConfig.security)
- enabled: whether risk analysis is active. If false, all commands are treated as Safe.
- block_critical: when true, commands detected as Critical are blocked outright.
- require_confirmation: map from RiskLevel to bool controlling whether a confirmation overlay is required before proceeding. Suggested defaults:
  - Safe: false
  - Caution: true (optional depending on your tolerance)
  - Warning: true
  - Critical: true (usually redundant if block_critical is true)
- require_reason: map from RiskLevel to bool for future extensions (e.g., prompt user for a typed reason). Currently informational.
- custom_patterns: list of regex patterns to classify specific commands with a chosen RiskLevel and message.

Risk Levels
- Safe: no known risky patterns were detected.
- Caution: potentially impactful operations (e.g., recursive chmod/chown).
- Warning: elevated risk patterns (e.g., curl | sh, chmod 777, DROP TABLE).
- Critical: extremely dangerous operations (e.g., rm -rf /, fork bombs, direct disk overwrite).

UX flow (high level)
- User performs an action that would execute a command (paste, AI apply, plugin).
- Security Lens analyzes text and assigns a RiskLevel with factors and mitigations.
- If `block_critical` and risk is Critical: block with an error message.
- Else if `require_confirmation[risk]` is true: show the confirmation overlay with details; user confirms or cancels.
- On confirm, execution proceeds; on cancel, nothing is executed. Optional `require_reason[risk]` may prompt for a reason (future).

Confirmation Overlay
- Triggered when require_confirmation[risk_level] is true and the command is not blocked.
- Keys while overlay is active:
  - Enter or Y: Confirm
  - Escape or N: Cancel
  - All other input is swallowed until you decide.
- The overlay shows:
  - Title indicating risk level
  - Explanation of risk factors
  - Suggested mitigations
  - The command for review

Timeouts
- If a confirmation request includes a timeout and it elapses, the overlay closes automatically and a warning message is posted: "Confirmation timed out". The pending request is cleaned up so future confirmations are not blocked.

Multi-window behavior
- The overlay opens in the target window where the action originated.
- When a confirmation is resolved (confirm or cancel) or times out, the overlay is closed across all windows to prevent stale UI.

Plugin and AI integration
- AI "Apply as command" flows are analyzed by the Security Lens; blocking or confirmation is enforced before a command is pasted to the prompt.
- Plugins that execute commands are required to honor the same policy and route confirmations through the overlay.

Configuration examples

TOML (recommended):
```toml path=null start=null
[security]
enabled = true
block_critical = true

[security.require_confirmation]
Safe = false
Caution = true
Warning = true
Critical = true

# Ask for typed reason on Critical (future UI)
[security.require_reason]
Critical = true

# Organization-/team-specific patterns
[[security.custom_patterns]]
pattern = "(?i)gcloud\s+compute\s+instances\s+delete"
risk_level = "Warning"
message = "Deleting a VM instance"

[[security.custom_patterns]]
pattern = "(?i)kubectl\s+delete\s+ns\s+prod"
risk_level = "Critical"
message = "Deleting the production namespace"
```

JSON (for external tooling/tests):
```json path=null start=null
{
  "enabled": true,
  "block_critical": true,
  "require_confirmation": {"Safe": false, "Caution": true, "Warning": true, "Critical": true},
  "require_reason": {"Critical": true},
  "custom_patterns": [
    {"pattern": "(?i)gcloud\\s+compute\\s+instances\\s+delete", "risk_level": "Warning", "message": "Deleting a VM instance"},
    {"pattern": "(?i)kubectl\\s+delete\\s+ns\\s+prod", "risk_level": "Critical", "message": "Deleting the production namespace"}
  ]
}
```
