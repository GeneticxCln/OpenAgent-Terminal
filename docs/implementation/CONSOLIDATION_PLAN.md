# Phase 1: Crate Consolidation & Dependency Pinning (RFC)

Date: 2025-09-22
Status: Draft (for review)
Owner: Core team

Summary
This proposal executes Phase 1 of FORWARD_PLAN_2025.md: reduce workspace complexity by consolidating crates and pin a few core dependencies to resolve version skew and improve build reproducibility.

Goals
- Reduce workspace members from ~25 to ~15 without breaking public APIs.
- Stabilize clean build time (target <4 min) and reduce dependency graph churn.
- Keep AI runtime and core terminal stable; changes are internal restructuring only.

Crate consolidation plan
1) IDE crates (4 → 1)
- Merge:
  - openagent-terminal-ide-editor
  - openagent-terminal-ide-lsp
  - openagent-terminal-ide-indexer
  - openagent-terminal-ide-dap
- Into:
  - openagent-terminal-ide/
- Notes:
  - Keep submodules (editor, lsp, indexer, dap) under src/ide/…
  - Re-export stable APIs from lib.rs for backward compatibility (pub use ide::lsp::…, etc.).
  - Deprecate old crate names in docs. Add rustdoc deprecation notes where appropriate.

2) Utilities (3 → 1)
- Merge:
  - openagent-terminal-themes
  - openagent-terminal-snippets
  - openagent-terminal-migrate
- Into:
  - openagent-terminal-utils/
- Notes:
  - Keep binaries (migrate) under src/bin/migrate.rs
  - Provide modules: utils::themes, utils::snippets, utils::migrate

3) Plugin system (4 → 2)
- Merge:
  - plugin-api + plugin-sdk → plugin-sdk/
  - plugin-loader + plugin-system → plugin-runtime/
- Notes:
  - Ensure Wasmtime/WASI runtime remains encapsulated under plugin-runtime
  - Keep plugin-sdk with stable API; cross-crate imports updated accordingly

4) Documentation and tooling updates
- Update workspace Cargo.toml members
- Update CI workflows and feature matrix
- Update docs paths (README, guides, docs/implementation/) and examples

Dependency pinning plan
Pin versions in [workspace.dependencies] in workspace Cargo.toml:
- base64 = "0.22.1" (resolve 0.21.x vs 0.22.x split)
- rustix = "1.1.2"
- sqlx = { version = "0.8.1", features = ["sqlite", "runtime-tokio-rustls"] }

Additional guidelines
- Avoid API breakage: use re-exports and clear deprecations
- Maintain feature flags parity across merges
- Keep test coverage stable; add smoke tests for consolidated crates
- Ensure docs builds and example code compile after path updates

Milestones & sequencing
- Milestone A (Docs & Scaffolding): This RFC merged, skeleton dirs created, no code moved yet
- Milestone B (IDE consolidation): Move IDE crates into openagent-terminal-ide, re-exports, tests pass
- Milestone C (Utilities consolidation): Move themes/snippets/migrate, adjust examples and docs, tests pass
- Milestone D (Plugin consolidation): Merge loader/system into runtime; api+sdk into sdk; update plugins/examples; tests pass
- Milestone E (Dependency pins): Add [workspace.dependencies] and resolve build issues; ensure minimal lockfile diff and CI green

Success criteria
- 25 → ~15 crates
- Clean build < 4 minutes (target; subject to runner)
- No public API breakage for end users (re-exports/deprecations OK)
- CI green across Linux/macOS/Windows

Risks & mitigations
- Risk: Hidden cross-crate feature dependencies
  - Mitigation: Compile with feature combinations in CI; add integration tests
- Risk: Doc and example path drift
  - Mitigation: Grep repo and fix all references in the same PRs
- Risk: Lockfile churn
  - Mitigation: Pin via workspace.dependencies and audit duplicate versions

Review
- Please comment on namespace layout, sequencing, and pins.
- After approval, we’ll cut PRs per milestone with isolated diffs for easier review.
