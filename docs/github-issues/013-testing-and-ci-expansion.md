#013 — Testing and CI expansion

Status: Open
Priority: High

Scope
- Ensure GPU snapshot tests and perf thresholds run on PRs
- Increase coverage to ≥80% for core crates
- Enforce strict clippy; add pre-commit hooks

References
- .github/workflows/ci.yml
- .github/workflows/performance.yml
- .github/workflows/wgpu-nightly.yml

Acceptance criteria
- CI fails on snapshot/perf regressions
- Coverage job reports ≥80% for core targets
- Pre-commit hooks documented and in place
