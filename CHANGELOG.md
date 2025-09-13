# Changelog (openagent-terminal crate)

## Unreleased

- UI/Workspace
  - Add `workspace.drag.highlight_min_alpha` (default 0.08) to keep drag highlights visible on light themes. Value is clamped to [0.0, 1.0].
  - Enforce clamping of highlight alphas to [0.0, 1.0] and hover ≥ base for consistency across themes.
  - Clamp snap margins to be non-negative.
  - Improve diagnostics for invalid drag `highlight_color` values (fall back to theme).
- Blocks
  - Add tests for header chip hit-testing under constrained widths and unicode headers.
  - Add tests for copy/export collection: exclude header line and trim trailing newline.
- Clipboard
  - Add optional runtime diagnostics for backend selection (Wayland/X11/macOS/Windows) and WSL detection. Enable with `OPENAGENT_CLIPBOARD_LOG=1`.
