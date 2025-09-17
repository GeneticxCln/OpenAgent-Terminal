# Code Coverage Policy

We enforce a minimum of 80% line coverage across core crates in this workspace.

What’s included in the threshold:
- openagent-terminal-core (terminal core engine)
- openagent-terminal-ai (AI interfaces and agents)
- Other non-GUI crates under crates/ and plugins/

What’s intentionally excluded:
- openagent-terminal (GUI app crate)

Rationale: The GUI app integrates winit/wgpu and OS windowing systems. Exercising it in a headless CI
environment is non-trivial and can lead to flaky, platform-specific results. We instead drive
render-smoke tests and snapshot checks for GUI paths separately, and measure coverage on the core
libraries and subsystems.

Local verification commands:
- Install: cargo install cargo-llvm-cov --locked
- Clean:   cargo llvm-cov clean
- Run (excluding GUI crate):
  cargo llvm-cov --workspace --exclude openagent-terminal --summary-only

CI enforcement:
- The GitHub Actions workflow at .github/workflows/ci.yml runs cargo-llvm-cov over the workspace and
  explicitly excludes the GUI crate. It enforces a >= 80% line coverage threshold.
