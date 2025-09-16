#014 — Cleanup and dependency hygiene

Status: Open
Priority: Medium

Scope
- Reduce unused/dead code
- Prune dependencies where possible
- Standardize error handling/logging patterns

References
- cargo-deny, cargo-audit, cargo-geiger steps in CI
- workspace Cargo.toml

Acceptance criteria
- cargo-deny/audit clean; dependency count reduced where safe
- Consistent error/logging patterns in core crates
