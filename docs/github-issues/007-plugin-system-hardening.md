#007 — Plugin system MVP hardening

Status: Open

Note: WASM runtime is gated behind the `wasm-runtime` feature and is disabled by default to avoid local API/CI mismatches. CI jobs enabling plugins will turn it on explicitly.
Priority: High

Scope
- Lock down host interfaces (read/write scope, notifications, host_execute_command via policy)
- Permission model enforcement + quotas (CPU/mem/fs)
- Tests for host_execute_command and storage APIs

References
- crates/plugin-loader/
- crates/plugin-system/
- crates/plugin-api/
- .github/workflows/ci.yml (plugins-tests)

Acceptance criteria
- Permissions and quotas enforced; attempts out of scope fail with clear errors
- Tests cover positive/negative cases for command execution and storage
