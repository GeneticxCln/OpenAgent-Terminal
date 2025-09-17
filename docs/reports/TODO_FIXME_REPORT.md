# OpenAgent Terminal — TODO/FIXME Updated Inventory (2025-09-17)

This report crosslinks the current in-code TODO/FIXME/unimplemented markers with the previous documentation (docs/TODO_FIXME_SUMMARY.md & TODO.md). It highlights what is now complete, what remains, and any mismatches.

Date: 2025-09-17

Key sources reviewed:
- In-repo grep scan (excluding docs/, target/, .git, .dev): TODO/FIXME/WIP/HACK/XXX/TBD/Not implemented/unimplemented!/todo!
- docs/TODO_FIXME_SUMMARY.md, TODO.md, roadmaps
- Code hotspots: warp_integration.rs, display/mod.rs, renderer/rects.rs, input/mod.rs, native_search.rs, plugin-system, ai_runtime.rs, cli_ai.rs

Executive summary
- Warp workspace integration: implemented and significantly advanced vs older doc claims; session restore + PTY creation + actions are in place.
- WGPU-only path is active; cursor overlay and text rect pipelines wired; shader-kind sync assertions exist. Perf HUD hooks present; validate final polish.
- AI: providers + secure config + persistence (JSONL+SQLite) implemented; streaming and retry-after are done. Minor CLI fallback missing when DB is absent.
- Plugin system: WASM path functional; native plugin (host-integration feature) explicitly not implemented.
- Test scaffolds include unimplemented!() in unit-test contexts only — not runtime faults.

Changes since docs/TODO_FIXME_SUMMARY.md
1) Warp Integration
- Docs listed many TODOs (split ops, pane mgmt, session restore, PTY wiring). Current code in openagent-terminal/src/workspace/warp_integration.rs implements:
  - create/close/next/prev tab; split right/down; navigate/resize; zoom/cycle/equalize; save/load session
  - session restoration with PTY creation and working-dir fallback; pane context wiring
- Action: Update docs/TODO_FIXME_SUMMARY.md to reflect that Warp integration core is implemented. Keep any remaining edge cases (e.g., window context binding where deferred) as “polish”.

2) AI Runtime Context & Persistence
- Docs suggested missing working directory and shell kind; current ai_runtime persists history to JSONL and SQLite, includes provider switching with secure creds, and stores working_directory & shell_kind in history entries. Shell-kind/WD population appears present in some agent code as TODO, but persistence pipeline supports it.
- Action: Mark persistence complete. For “derive context” in agents (e.g., natural_language.rs TODOs for shell kind, spans, confidence calc), keep listed as medium-priority polish.

3) WGPU Rendering Backend
- The codebase is WGPU-only; display/mod.rs sets cursor overlay via renderer; rects/shapes have shader sync assertions in renderer/rects.rs. Perf HUD drawing hooks present.
- Action: Update docs to reflect WGPU-only path shipped; keep “perf HUD polish and validation” as ongoing.

4) Tab Bar Configuration (show_tab_close_button)
- Docs listed missing config. Display/input mention cached geometry for close buttons with TODO note to handle interactions using cached geometry (openagent-terminal/src/input/mod.rs:1619).
- Action: Keep as a targeted UI task: implement click handlers for close/new-tab using cached geometry.

5) Plugin Storage and Native Plugins
- Docs referenced plugin storage and host_execute_command policy tests — now present. However, native plugin loading/execution (host-integration feature) intentionally returns Not Implemented.
- Action: Clarify in docs that native plugins are deferred; WASM is the supported path. Keep native plugin support as future roadmap, not a v1.0 blocker.

6) Event error handling (core/event.rs)
- A TODO notes erroring in notify (openagent-terminal-core/src/event.rs:107). Overall, the event path is stable; address the TODO or reword to reflect current error strategy.

New/confirmed items from current scan (highlights)
- AI agents
  - natural_language.rs: confidence calculation; NLP improvements; spans; shell_kind context usage; parameter extraction.
  - quality_validation.rs, project_context.rs, workflow_orchestration.rs: stubs marked TODO — scope and prioritize.
  - code_generation.rs: track is_busy/concurrency.
- Communication hub/workflow orchestrator
  - communication_hub.rs: TODOs for parallel and dependency-based execution; direct message routing.
  - workflow_orchestrator.rs is rich; validation currently minimal — consider additional validations.
- Input/UI
  - input/mod.rs: several unimplemented! in test scaffolding for ActionContext; plus TODO to wire close/new-tab using cached geometry.
- Native search
  - native_search.rs: “other filters not implemented yet” fallback — enumerate remaining filters and implement or document.
- AI CLI
  - cli_ai.rs: JSONL export fallback not implemented when SQLite DB missing.
- Plugin system (native)
  - plugin-system/src/lib.rs: native plugin load/exec not implemented for host-integration feature.
- Themes marketplace
  - openagent-terminal-themes/src/marketplace.rs: todo!("Implement theme details/download/publish"). Treat as optional ecosystem feature.
- Migrate parsers
  - iTerm2/WezTerm parsers have TODOs for full parsing; track under migrate hardening.

Items that are just tests/scaffolds (not runtime defects)
- openagent-terminal-core/src/grid/tests.rs and storage.rs: unimplemented!() in dummy GridCell flags used only for testing
- input/mod.rs unimplemented!() methods inside test impls of ActionContext
- renderer/rects.rs invalid-flag path uses unimplemented!() — defensive, not expected in normal flow

Actionable reconciliation deltas
- Update docs/TODO_FIXME_SUMMARY.md:
  - Mark Warp integration core as completed; list any remaining polish.
  - Mark WGPU-only and perf HUD hooks as implemented; polish TBD.
  - Mark AI streaming/retry-after as completed; add minor CLI export fallback.
  - Reclassify native plugin loading/execution as “deferred”; WASM supported.
- Update TODO.md checkboxes: testing coverage target still pending; dep hygiene pending.
- Add two new tasks:
  - Implement AI CLI JSONL fallback export when DB missing.
  - Complete native_search filter kinds or explicitly document which are supported.

Proposed priority list
1) Quality/coverage: raise core coverage; CI thresholds for renderer/UI paths
2) Small UX: input/mod.rs handle tab close/new-tab interactions via cached geometry
3) AI polish: parameter extraction, confidence calc, shell-kind usage in agents
4) Native search filter completion
5) Docs cleanup to reflect current state

See the accompanying raw inventory for full matches and line references.
