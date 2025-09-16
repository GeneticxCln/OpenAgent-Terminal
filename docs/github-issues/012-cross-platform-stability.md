#012 — Cross-platform stability (Windows/Linux/macOS)

Status: Open
Priority: Medium

Scope
- Re-validate Windows PTY lifecycle with integration tests
- Expand Linux/Wayland and macOS CI coverage as needed

References
- openagent-terminal-core/src/tty/windows/pty_lifecycle.rs
- .github/workflows/ci.yml (matrix build)

Acceptance criteria
- PTY lifecycle tests pass on Windows CI
- Platform smoke tests green across matrix, with known issues documented
