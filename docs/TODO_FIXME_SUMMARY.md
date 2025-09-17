# OpenAgent Terminal TODO/FIXME Summary (Reconciled)

This document reflects the current state of TODO/FIXME markers cross-checked against code as of 2025-09-17. It supersedes outdated items and clarifies what is completed, what remains, and what is deferred.

## Executive Summary

- Total markers observed: ~50 (many are test scaffolds or template-related)
- Critical issues: 0 (Windows ConPTY drop-order remains resolved)
- Major deltas since prior summary:
  - Warp workspace integration core is implemented (tabs/splits/session restore/PT Y wiring)
  - WGPU-only rendering path active; cursor overlay and rect pipelines wired; perf HUD hooks present
  - AI streaming/retry-after completed; history persistence implemented (JSONL + SQLite)
  - Native plugin (host-integration) intentionally deferred; WASM runtime supported

## Critical Priority Issues 🔴

### 1. PTY Drop Order (Windows) — Resolved
- Enforced via typestate lifecycle in core; validated by tests
- Reference: docs/github-issues/001-critical-pty-drop-order.md

## High Priority Issues 🟠 (Current)

### 2. Native Search: Complete Filter Coverage
- File: `openagent-terminal/src/native_search.rs`
- Issue: Some filter kinds fall back to a default path (not implemented yet)
- Impact: Search precision/UX
- Action: Enumerate supported filters and implement the remaining ones (or document limits)

### 3. Input/UI: Tab Bar Interactions via Cached Geometry
- Files: `openagent-terminal/src/input/mod.rs` (see TODO around cached geometry)
- Issue: Handle close-button/new-tab clicks using cached tab bounds
- Impact: Usability polish

### 4. Testing & Coverage Target
- Goal: Raise coverage to ≥80% in core areas; expand GPU snapshot/perf CI thresholds
- Impact: Stability and regression confidence

## Items Now Completed (were previously listed as High) ✅

### A. Warp Integration Features
- File: `openagent-terminal/src/workspace/warp_integration.rs`
- Status: Implemented — create/close/next/prev tab; split right/down; pane navigation/resize; zoom/cycle/equalize; session save/load; PTY creation with working-dir fallback; pane focus updates
- Note: Keep minor polish/edge cases tracked with normal issue labels

### B. AI Runtime Context & Streaming Reliability
- Files: `openagent-terminal/src/ai_runtime.rs`, `openagent-terminal-ai/*`
- Status: Context persisted (working_directory/shell_kind fields supported in persistence), secure provider config path, streaming with Retry-After handling and backpressure implemented and tested
- Note: Agent-level usage of shell kind/confidence scoring still has TODOs (see Medium)

### C. WGPU Rendering Backend — Core Parity
- Files: `openagent-terminal/src/display/*`, `openagent-terminal/src/renderer/*`
- Status: WGPU-only path enforced; cursor overlay via renderer uniforms; rect pipelines present; shader-kind sync assertions in place
- Note: Keep perf HUD polish/validation as ongoing

## Medium Priority Issues 🟡 (Current)

### 5. AI Agents: Context Utilization & Scoring
- Files: `openagent-terminal/src/ai/agents/*`
- Issues: Confidence calculation; better NLP; parameter extraction; shell-kind propagation in suggestions
- Impact: Proposal quality/UX

### 6. AI CLI: JSONL Fallback Export
- File: `openagent-terminal/src/cli_ai.rs`
- Issue: When SQLite DB is unavailable, JSONL export path is not implemented
- Impact: CLI usability; easy fix

### 7. Event Error Handling
- File: `openagent-terminal-core/src/event.rs`
- Issue: TODO about erroring capability for notify
- Impact: Error resilience; clarify strategy or implement

### 8. Shader Rect Synchronization
- File: `openagent-terminal/src/renderer/rects.rs`
- Issue: Keep WGSL fragment defines in sync with RectKind enum (assertions exist)
- Impact: Maintainability/correctness

## Low Priority Issues 🟢

### 9. Platform-specific Notes
- File: `openagent-terminal-core/src/tty/unix.rs`
- Note: macOS-specific `exec -a` commentary; low impact

### 10. Documentation TODOs
- File: `docs/QUICK_START_DEVELOPMENT.md`
- Issue: Some sections remain to be completed

### 11. Test Artifacts
- Several `.recording` files contain embedded TODO markers; these are low-impact test data

## Deferred / Out of Scope for v1.0

### D1. Native Plugin Loading/Execution (host-integration)
- File: `crates/plugin-system/src/lib.rs`
- Status: Explicitly returns “not implemented”; WASM runtime is the supported plugin path
- Action: Track as a post-1.0 roadmap item

## Issues by Component (Updated)

### Rendering System
- WGPU backend: core complete; perf HUD polish/validation (Medium)
- Shader/enum sync (Medium)

### Workspace Management
- Warp integration: complete
- Continue routine polish and regression testing

### AI Features
- Streaming/backpressure/retry-after: complete
- Agents’ context usage (Medium)
- CLI JSONL fallback export (Medium)

### Plugin System
- WASM host/runtime supported
- Native plugin host-integration deferred

### Platform Support
- Windows PTY stability: Resolved
- Unix/macOS compatibility: Low

### User Interface
- Tab bar interaction using cached geometry (High)

## Recommended Action Plan (Revised)

### Phase 1: Quality & UX (Week 1–2)
1. Implement AI CLI JSONL fallback export
2. Wire tab close/new-tab interactions using cached geometry
3. Clarify/implement event notify error handling

### Phase 2: Search & Agents (Week 2–4)
1. Complete native_search filters (enumerate and implement)
2. Improve agent context usage (confidence, shell-kind propagation, parameter extraction)
3. Add unit/integration tests accordingly

### Phase 3: Coverage & Perf (Week 4–6)
1. Raise core coverage to ≥80%
2. Tighten GPU snapshot/perf thresholds
3. Perf HUD validation/polish

### Phase 4: Documentation & Hygiene (Week 6–8)
1. Reconcile remaining docs with current state
2. Dependency and dead-code pruning; standardize error/log patterns

## Implementation Notes

- Many TODO/FIXME markers are scaffolding or test-only and not runtime defects
- Major systems (Warp, WGPU core, AI streaming/persistence, WASM plugins) are in place

## Tracking (Suggested)

- New issues to open:
  - [ENHANCEMENT] AI CLI: JSONL fallback export when DB missing
  - [FEATURE] Native search: complete filter coverage
  - [ENHANCEMENT] Tab bar: interaction via cached geometry
  - [ENHANCEMENT] Event notify: error handling strategy
  - [QA] Coverage to ≥80% in core crates

---

Generated: 2025-09-17
Status basis: grep inventory + code inspection
