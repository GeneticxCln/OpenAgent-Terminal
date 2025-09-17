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

OpenAgent Terminal also exposes visual tuning for drag drop highlights and snap behavior near the tab bar.

Advanced options:

```toml path=null start=null
[workspace.drag]
# Optional explicit highlight color for drag drop zones (overrides theme tokens when set)
# highlight_color = "#7aa2f7"

# Minimum alpha for highlights in light themes (ensures visibility)
# highlight_min_alpha = 0.08

# Base/hover alpha for split targets
# Values are clamped to [0.0, 1.0] at load time; hover is coerced to be >= base
highlight_alpha_base = 0.15
highlight_alpha_hover = 0.50

# Base/hover alpha for tab highlight when hovering a tab as a drop target
# Values are clamped to [0.0, 1.0] at load time; hover is coerced to be >= base
tab_highlight_alpha_base = 0.12
tab_highlight_alpha_hover = 0.40

# Base/hover alpha for the New Tab area highlight
# Values are clamped to [0.0, 1.0] at load time; hover is coerced to be >= base
new_tab_highlight_alpha_base = 0.10
new_tab_highlight_alpha_hover = 0.45

# Snap behavior near the tab bar band (in pixels)
# Treats cursor as "inside" when within this vertical margin (clamped to be non-negative)
tab_drop_snap_px = 6.0
# Extra horizontal margin near the right edge to ease targeting the New Tab area
new_tab_snap_extra_px = 24.0
```

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

The Warp-style tab bar is drawn as an overlay by default and no longer reserves a terminal row. Visibility can be set to "Auto", "Always", or "Hover". Auto behaves as Always unless the window is fullscreen; in fullscreen it behaves like Hover.

- Always: Tab bar is always visible.
- Hover: Tab bar appears when the mouse is near the configured edge (Top/Bottom), within a small tolerance band. Close button rendering also supports hover-only via `workspace.tab_bar.close_button_on_hover`.
- Auto: Behaves like Always, except on fullscreen where it behaves like Hover.

## Rendering (WGPU): Subpixel Text and Gamma

OpenAgent Terminal’s WGPU backend supports LCD subpixel text rendering with adjustable gamma and orientation. These options live under the `[debug]` section and are active only when using the WGPU renderer.

- `subpixel_text`: "Auto" | "Enabled" | "Disabled"
  - Auto enables subpixel only on compatible surfaces; use Enabled to force it.
- `subpixel_orientation`: "RGB" | "BGR"
  - Matches your physical LCD stripe order (most are RGB; some panels are BGR).
- `subpixel_gamma`: float (typical 2.2). Valid range: 1.4 – 3.0 (clamped).
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
- Toggle subpixel: Ctrl+Shift+L (Cmd+Shift+L on macOS)
- Cycle RGB/BGR: Ctrl+Shift+Y (Cmd+Shift+Y)
- Gamma +/−/reset: Ctrl+Shift+G / Ctrl+Shift+H / Ctrl+Shift+R (Cmd+Shift+…)
- Gamma +/−/reset: Ctrl+Shift+G / Ctrl+Shift+H / Ctrl+Shift+R (Cmd+Shift+… on macOS)

## Rendering backend

OpenAgent Terminal uses the WGPU renderer exclusively. OpenGL fallback has been removed.

Notes:
- Ensure your system has a supported graphics API (Vulkan/Metal/DirectX) and up-to-date drivers.
- The `debug.prefer_wgpu` option remains for compatibility but no longer affects backend selection.

