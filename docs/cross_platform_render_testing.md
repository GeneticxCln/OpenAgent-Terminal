# Cross-platform rendering, animation, and performance testing

This checklist helps verify rendering correctness, animation smoothness, and performance across backends and platforms:

- Backends: WGPU and OpenGL (GLSL3/GLES2)
- Platforms: Linux (X11, Wayland), macOS (CGL/Metal via WGPU), Windows (WGL/D3D12/Vulkan via WGPU)

Note: WGPU availability and backend mapping depend on the system drivers. Use the debug config and environment variables below to steer selection.

## 1) Build configuration

- Ensure required features are enabled in Cargo.toml or via CLI
- For WGPU builds, enable the `wgpu` feature where applicable

Example (binary):

```bash path=null start=null
cargo build --release
```

## 2) Selecting rendering backend

Use config or environment flags to pick a backend.

- Config (preferred):
  - debug.prefer_wgpu = true|false
  - debug.renderer = "Glsl3" | "Gles2" | "Gles2Pure" (forces OpenGL path)

- Environment (WGPU specific):
  - WGPU_BACKEND: vulkan | metal | dx12 | dx11 | gl | webgpu

Examples:

```bash path=null start=null
# Prefer WGPU (if available)
OPENAGENT_TERMINAL_DEBUG_PREFER_WGPU=1 cargo run --release

# Force OpenGL legacy path via renderer preference (in config TOML)
# [debug]
# renderer = "Glsl3"
# or: renderer = "Gles2"

# Force WGPU + a specific backend (Linux)
WGPU_BACKEND=vulkan cargo run --release

# Force WGPU + GL backend (fallback path)
WGPU_BACKEND=gl cargo run --release
```

## 3) Platform coverage

- Linux
  - X11: ensure `WAYLAND_DISPLAY` is unset and an X11 session is active
  - Wayland: ensure `WAYLAND_DISPLAY` is set; confirm at startup logs

- macOS
  - WGPU: metal backend (default). Verify selection in logs
  - OpenGL path uses CGL via glutin

- Windows
  - WGPU: dx12 (default) or vulkan depending on drivers
  - OpenGL path uses WGL/ANGLE/EGL depending on config

## 4) Animation smoothness checklist

Focus areas: tab open/close/switch, drag start/move/end, hover/focus highlights.

- Enable/disable reduce motion to validate fallbacks
- Validate consistent easing and duration across backends

Config knobs:

```toml path=null start=null
[workspace]
# ...
[debug]
prefer_wgpu = true
# renderer = "Glsl3" # Uncomment to force GL path

# Optional: top-level reduce motion override
# reduce_motion_override = true
```

Validation steps:

- Open/close 20+ tabs quickly; ensure no jank or dropped frames
- Drag a tab across the bar; check shadow/scale smoothness
- Switch tabs repeatedly; verify highlight animation timing
- Resize window rapidly; ensure animations remain smooth

## 5) Performance micro-benchmarks

Use existing examples and tests to measure baseline performance:

```bash path=null start=null
# WGPU perf harness (if present)
cargo run --release -p openagent-terminal --example perf_wgpu

# Snapshot tests (GPU)
node ./src/testing/gpu-snapshot.ts   # See repo script docs

# Render smoke tests
cargo test -p openagent-terminal --tests render_smoke -- --nocapture
```

Metrics to observe:

- Startup time to first frame
- Frame pacing under interaction (e.g., resizing, scrolling)
- Memory footprint deltas between backends
- GPU backend selection and device limits in logs

## 6) Troubleshooting

- Backend not selected as expected
  - Check logs for: "Render backend selected: ..."
  - Confirm `debug.prefer_wgpu` and `debug.renderer` settings
  - Verify `WGPU_BACKEND` env var

- Wayland vs X11 detection
  - Logs will print detected display server at startup

- Tearing/stutter
  - Try toggling vsync in config (if exposed), or window compositor settings
  - On Linux, test both X11 and Wayland

## 7) Reporting

Capture the following when reporting results:

- OS and version, GPU, driver versions
- Backend selected and (for WGPU) backend adapter name
- Summary of animation smoothness and any jank cases
- Perf numbers: startup time, frame pacing qualitative notes, memory deltas (if available)
- Issues with screenshots or logs

