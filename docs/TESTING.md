# Testing OpenAgent Terminal

This guide describes how to run tests, lints, and selected GUI/headless tests locally.

## Quick checks

- Lint the entire workspace

```bash
cargo clippy --workspace --all-features
```

- Run the full test suite (non-interactive tests)

```bash
cargo test --workspace --all-features --no-fail-fast
```

Useful flags:
- Use `--no-fail-fast` to see all failures in one run.
- Use `-p <crate>` to run tests for a single crate.
- Use `-- --ignored` to include tests marked as `#[ignore]`.

## Headless GUI tests (WGPU + winit)

OpenAgent Terminal includes a small number of GUI tests which create real hidden windows using winit/WGPU. These require:

- A desktop session (DISPLAY or WAYLAND_DISPLAY set)
- The `wgpu` renderer enabled
- Opt-in via environment variable

To run the consolidated headless GUI test:

```bash
# Enable explicitly and ensure a GUI session
export OPENAGENT_HEADLESS_GUI_TESTS=1

# Run the suite in the main binary crate (features required by the test)
cargo test -p openagent-terminal --features native-extras,wgpu --test keyboard_headless -- --nocapture
```

Notes:
- The suite runs all GUI scenarios under a single winit EventLoop (winit 0.30 allows only one event loop per process).
- The older per-scenario tests are compiled out and not run by default (see tests/keyboard_headless.rs for details).

## Long-running provider tests

Some AI provider tests perform network requests (e.g., OpenAI) and may run longer on first execution. By default they are included in the full test run; use `-p` filters to narrow scope if necessary:

```bash
# Example: run only core library tests
cargo test -p openagent-terminal-core --all-features
```

## Coverage, privacy and UI testing

- Coverage: see docs/COVERAGE.md
- Privacy & UI compliance: see docs/TESTING_PRIVACY_UI_COMPLIANCE.md and docs/PRIVACY_AND_UI.md

## Common troubleshooting

- WGPU/Vulkan init failures: ensure your system has an up-to-date GPU driver and a supported backend (Vulkan/Metal/DirectX). See renderer notes in the configuration manual.
- Headless GUI tests skipped: ensure `OPENAGENT_HEADLESS_GUI_TESTS=1` is set and a GUI session is available (DISPLAY or WAYLAND_DISPLAY).