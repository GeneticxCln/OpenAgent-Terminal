# Configuration Reference

This page documents configuration options specific to OpenAgent Terminal’s UI/Workspace integrations.

## Global UI: Reduce Motion Override

The UI can reduce or disable motion/animations for accessibility. Themes may define a default value, but you can override it globally per user with `reduce_motion_override`.

Precedence:
- If `reduce_motion_override` is set, it takes precedence over any theme-provided value.
- If it is not set, the theme’s `reduce_motion` value (if any) is used.

Examples:

Enable reduced motion globally (minimize/disable animations):

```toml path=null start=null
# ~/.config/openagent-terminal/openagent-terminal.toml
reduce_motion_override = true
```

Explicitly disable reduced motion (allow full animations):

```toml path=null start=null
# ~/.config/openagent-terminal/openagent-terminal.toml
reduce_motion_override = false
```

Notes:
- The effective reduce‑motion setting is applied during window initialization and on live config reloads, propagating to tab/workspace animations, pane drag visuals, and other UI transitions.

## Workspace: Pane Drag Gesture

OpenAgent Terminal supports Warp‑style pane drag‑and‑drop between splits and tabs. This gesture is configurable under the `workspace.drag` section.

Defaults:
- Modifier: `Alt`
- Mouse button: `Left`

Configure or change the gesture:

```toml path=null start=null
[workspace.drag]
# Enable Alt+Left‑drag to move panes between splits/tabs (default: true)
enable_pane_drag = true
# Modifier required to start a pane drag: "Alt" | "Ctrl" | "Shift" | "None"
pane_drag_modifier = "Alt"
# Mouse button used to start a pane drag: "Left" | "Middle" | "Right"
pane_drag_button = "Left"
```

Example: Use Ctrl + Middle‑click to drag panes:

```toml path=null start=null
[workspace.drag]
enable_pane_drag = true
pane_drag_modifier = "Ctrl"
pane_drag_button = "Middle"
```

Behavioral notes:
- When dragging over the tab bar, OpenAgent Terminal uses precise, cached pixel bounds of tabs computed during rendering to select the correct drop target. If unavailable (rare), it falls back to even‑width approximations.
- When dropping into a split, visual edge zones (left/right/top/bottom) determine the split direction and placement (before/after).

## Tab Bar

The Warp-style tab bar is drawn as an overlay by default and no longer reserves a terminal row. Visibility can be set to "Auto", "Always", or "Hover"; when set to Auto, fullscreen windows behave like Hover.

## Rendering (WGPU): Subpixel Text and Gamma

OpenAgent Terminal’s WGPU backend supports LCD subpixel text rendering with adjustable gamma and orientation. These options live under the `[debug]` section and are active only when using the WGPU renderer.

- `subpixel_text`: "Auto" | "Enabled" | "Disabled"
  - Auto enables subpixel only on compatible surfaces; use Enabled to force it.
- `subpixel_orientation`: "RGB" | "BGR"
  - Matches your physical LCD stripe order (most are RGB; some panels are BGR).
- `subpixel_gamma`: float (typical 2.2)
  - Adjusts foreground linearization for per-channel coverage; increases perceived sharpness/contrast.

Example:

```toml path=null start=null
# ~/.config/openagent-terminal/openagent-terminal.toml
[debug]
# Subpixel text preferences (WGPU backend only)
subpixel_text = "Enabled"          # "Auto" | "Enabled" | "Disabled"
subpixel_orientation = "RGB"        # "RGB" | "BGR"
subpixel_gamma = 2.2                # Typical range: 1.8 – 2.4
```

Runtime shortcuts (default):
- Toggle subpixel text: Ctrl+Shift+L (Cmd+Shift+L on macOS)
- Cycle orientation RGB/BGR: Ctrl+Shift+Y (Cmd+Shift+Y on macOS)
- Perf HUD toggle: Ctrl+Shift+F (Cmd+Shift+F)
- Gamma +/−/reset: Ctrl+Shift+G / Ctrl+Shift+H / Ctrl+Shift+R (Cmd+Shift+… on macOS)

## Rendering backend selection and fallback

By default, when built with the `wgpu` feature, OpenAgent Terminal will initialize the WGPU backend first and automatically fall back to the OpenGL backend if WGPU initialization fails.

You can control this behavior via config or environment variables:

- Config (recommended):

```toml
# ~/.config/openagent-terminal/openagent-terminal.toml
[debug]
# Prefer WGPU first, then fallback to OpenGL if WGPU init fails
prefer_wgpu = true
```

- Environment variables (override config):
  - Force OpenGL backend only:
    - OPENAGENT_FORCE_GL=1
  - Disable fallback (fail instead of falling back to OpenGL if WGPU init fails):
    - OPENAGENT_DISABLE_GL_FALLBACK=1

Notes:
- Building without the `wgpu` feature produces an OpenGL-only binary.
- RendererPreference in config controls only the OpenGL shader variant (Glsl3/Gles2/Gles2Pure) when the OpenGL backend is active.

