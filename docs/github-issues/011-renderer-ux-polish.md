#011 — Renderer/UX polish

Status: Open
Priority: Medium

Scope
- Verify subpixel/gamma controls and perf HUD across platforms
- Ensure text shaping/cache logic stability (HarfBuzz/Swash)
- Expose missing UI config (e.g., show_tab_close_button)

References
- openagent-terminal/src/display/tab_bar.rs
- openagent-terminal/docs/configuration.md
- tests snapshot scenarios (tab_bar/*)

Acceptance criteria
- Configurable tab close button; docs updated
- Perf HUD and subpixel controls tested and documented
