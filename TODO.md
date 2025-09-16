# OpenAgent Terminal — Project TODO (v1.0 readiness)

Last updated: 2025-09-16

This is a high-level execution list derived from the current plan. For code-level TODO/FIXME aggregation, see docs/TODO_FIXME_SUMMARY.md.

1) Security Lens to “complete” (#006)
- [x] Finish policy configuration surface
- [x] Expand risk patterns
- [x] Ensure command-exec gating is enforced everywhere
- [x] Add targeted tests for high-risk patterns and policy modes

2) Plugin system MVP hardening (#007)
- [x] Lock down host interfaces (read/write scope, notifications, command execution via policy)
- [x] Permission model enforcement + quotas
- [x] Test-runner coverage for host_execute_command
- [x] Test-runner coverage for storage APIs

3) Workflow foundations (#008)
- [x] Ship TOML/YAML parser
- [x] Basic parameterization
- [x] Minimal launcher panel
- [x] Persist workflow runs and add re-run capability

4) AI streaming reliability (#009)
- [x] Implement robust Retry-After handling across providers
- [x] Implement micro-batching/backpressure across providers
- [x] Improve streaming UX with clear states
- [x] Improve streaming logging

5) Persistence (#010)
- [x] Namespaced plugin storage
- [x] AI conversation history persistence
- [x] Schema + tests

6) Renderer/UX polish (#011)
- [x] Verify subpixel/gamma controls and perf HUD
- [x] Ensure text shaping and cache logic are stable across platforms
- [x] Expose missing UI config (e.g., show_tab_close_button)

7) Windows/Linux/macOS stability (#012)
- [x] Re-validate PTY drop-order
- [x] Update docs/TODO if resolved
- [x] Expand cross-platform integration tests

8) Testing and CI (#013)
- [x] Turn on GPU snapshot tests and perf CI with thresholds
- [ ] Increase coverage target to ≥80% for core areas
- [x] Enforce strict clippy
- [x] Add pre-commit hooks

9) Cleanup and dependency hygiene (#014)
- [ ] Reduce unused/dead code
- [ ] Prune dependencies where possible
- [ ] Standardize error handling/logging

10) Documentation updates (#015)
- [x] Reconcile docs/TODO_FIXME_SUMMARY.md with current reality (especially the PTY item)
- [x] Keep docs/implementation/IMPLEMENTATION_PROGRESS.md in sync
- [x] Align with docs/roadmaps/RELEASE_PLAN_V1.0.md milestones

References
- docs/TODO_FIXME_SUMMARY.md
- docs/implementation/IMPLEMENTATION_PROGRESS.md
- docs/roadmaps/RELEASE_PLAN_V1.0.md
- docs/roadmaps/ROADMAP.md
- openagent-terminal-ai/src/streaming.rs (Retry-After handling)
- crates/plugin-loader/ (host_execute_command policy)
- crates/plugin-api/ (storage preview)
- openagent-terminal/src/display/tab_bar.rs (tab bar config)
- openagent-terminal/src/main.rs (PTY drop order — historical context)
- src/testing/gpu-snapshot.ts (GPU snapshots)
