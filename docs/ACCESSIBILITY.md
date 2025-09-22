# Accessibility Guide

This guide summarizes keyboard navigation, assistive tech considerations, and useful settings.

Keyboard-only navigation
- Global
  - Ctrl+Shift+P: Command Palette
  - Ctrl+Shift+A: AI Panel (toggle)
  - Ctrl+Shift+S: Block Search
  - Ctrl+Shift+W: Workflows Panel
  - Ctrl+Shift+F: Perf HUD (debug)
- Within lists/overlays
  - Arrow Up/Down or j/k: Move selection
  - Enter: Confirm
  - Escape: Close/Cancel
- Composer (bottom input)
  - Left/Right: Move caret (Ctrl/Alt+Left/Right to jump words)
  - Home/End: Start/End of line
  - Ctrl/Cmd+C/X/V: Copy/Cut/Paste
  - Ctrl/Cmd+A: Select all
  - Shift+Enter: Run composer text (native command pipeline)

Screen reader notes
- The terminal framebuffer is rendered via WGPU; screen readers cannot access the pixel buffer directly.
- Tips:
  - Enable terminal selection copy-on-select to allow copying text for screen readers.
  - Use Blocks Search to retrieve prior command output in plain text.
  - Keyboard hints feature can reveal actionable links and copy targets.

High contrast & motion
- Use config to reduce motion and improve contrast:
  - [debug]
    - reduce_motion = true
    - perf_hud = false
  - [theme]
    - Prefer higher contrast themes; adjust gamma for subpixel text when using WGPU.
- Runtime toggles (WGPU):
  - Ctrl+Shift+L: Toggle subpixel text
  - Ctrl+Shift+Y: Cycle RGB/BGR subpixel orientation
  - Ctrl+Shift+G/H/R: Gamma +/−/reset

Caret and cursor
- Caret blinking pauses while typing; can be disabled via config cursor.blinking settings.
- For better visibility:
  - Use block or underline cursor styles and increase cursor thickness.

Copying content
- Selection copies to the clipboard.
- Blocks Search panel supports copying command, output, or both via dedicated keys.

Known limitations
- No native screen reader exposure of the live framebuffer; use copy/search tools to retrieve text.
- Inline suggestions and AI panels rely on rendered UI; keyboard navigation is provided for all actions.

Feedback
- Please file issues with your platform, screen reader/version, and expected flow so we can improve accessibility coverage.