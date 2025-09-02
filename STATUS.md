# OpenAgent Terminal — Status (Single Source of Truth)

Last updated: 2025-09-02
Branch: main
Maturity: Beta

This document is the canonical source for the current state of OpenAgent Terminal. Other documents (like PHASE3_COMPLETE.md or various plans/summaries) may be historical snapshots; defer to this file for the latest status.

Summary
- Core terminal: Stable (Alacritty-based), cross-platform.
- AI assistant: Functional and safe-by-design. Providers: Ollama (local, default), OpenAI, Anthropic. Streaming supported where available. No auto-execution.
- Rendering: OpenGL default; WGPU backend is experimental/in progress (not default). HarfBuzz/Swash text shaping available via feature flag.
- Security Lens (command risk analysis): In progress (MVP planned for Phase 4).
- Blocks v2: In progress (command/result grouping and basic actions).
- Plugin system and workflows: Prototypes exist in the workspace; API not yet stable or broadly integrated.
- Testing & CI: In progress. GPU snapshot, performance CI, and fuzz testing are planned and partially scaffolded.
- Privacy stance: No telemetry; local AI by default; API keys via environment variables.

Feature completion snapshot
- Core Terminal: 100%
- AI Integration: 90%
- Security Features (Security Lens): 20%
- Testing Infrastructure: 30%
- Workspace Management: 10%
- Plugin System: 0% (prototypes exist, API not stable)
- Collaboration/Sharing: 0%
- Performance Targets: 70%
- Documentation: 60%

Near-term priorities (Phase 4 highlights)
1) Security Lens MVP: risky command detection, risk levels, confirmations, and suggested safer alternatives.
2) WGPU parity: ensure a selectable, reliable alternative to OpenGL; visual regression tests.
3) Blocks v2 essentials: reliable grouping, search/filter, and quick actions (copy/export).
4) Test & performance CI: GPU snapshot testing, performance smoke tests, and input fuzzing.
5) Plugin API stabilization: versioned traits, 2–3 official example plugins, and documentation.

Performance targets (subject to validation in CI)
- Startup time: <100ms
- Render latency: <16ms typical (60 FPS)
- Memory (idle): <50MB; with AI: <150MB
- Local AI response (Ollama): sub-second for typical prompts

Where to look next
- Roadmap: DEVELOPMENT_PLAN.md and FEATURE_ROADMAP.md
- Implementation details and task breakdown: IMPLEMENTATION_PROGRESS.md
- Comparison with other terminals: COMPARISON_ANALYSIS.md

Notes
- Windows requires ConPTY (Win10 1809+); Wayland/X11 auto-detection on Linux, can be forced via env.
- This file supersedes conflicting statements elsewhere. If you find a conflict, treat this file as authoritative and open an issue to fix the outdated document.

