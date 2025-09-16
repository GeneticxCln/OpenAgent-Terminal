#006 — Complete Security Lens (Policies + Enforcement)

Status: Open
Priority: High

Scope
- Finalize policy configuration surface
- Expand risk patterns (containers, cloud, DBs, destructive ops)
- Ensure command-exec gating is enforced in all paths (UI, CLI, plugins)
- Add targeted tests for critical patterns and policy modes

References
- openagent-terminal/src/security/security_lens.rs
- openagent-terminal/docs/security_lens.md
- policies/bundles/*.toml
- .github/workflows/ci.yml (security-related tests)

Acceptance criteria
- Policy toggles configurable and persisted
- Blocking/confirmation behaves as configured across all execution paths
- Tests validate representative high-risk commands and policy outcomes
