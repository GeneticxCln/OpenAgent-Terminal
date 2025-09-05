# IDE features (Editor, LSP, DAP, File Tree) — Quick Start

This repository now includes initial building blocks for IDE-like capabilities inside OpenAgent Terminal, focused on terminal UX and AI workflows. No web-based editor engines (Monaco/CodeMirror) are used.

Warp parity tailoring: the IDE overlays (file tree, editor, LSP, DAP) are designed to slot into the Warp-style workspace (tabs/splits) so each overlay is a pane-aware view. Cursor position, signature help, and debugging state are all scoped to the focused pane.

What’s included:
- Project Indexer and File Tree (crate: openagent-terminal-ide-indexer)
- Full LSP client over stdio (crate: openagent-terminal-ide-lsp): completion, hover, go-to-definition, references, rename, formatting, signature help, diagnostics
- DAP client over stdio (crate: openagent-terminal-ide-dap): initialize/launch, breakpoints, continue/step, stack/variables
- Native editor core (rope buffer) (crate: openagent-terminal-ide-editor) with in-terminal overlay
- Feature flags to opt-in per-component or as a bundle via `ide`

Build with features:
- All IDE components: `cargo build -p openagent-terminal --features ide`
- Individual components: `cargo build -p openagent-terminal --features indexer,lsp,dap,editor`

Configuration
- A new config module exists under `openagent-terminal/src/config/ide.rs`. Defaults include:
  - rust -> rust-analyzer
  - typescript -> typescript-language-server --stdio
  - python -> pyright-langserver --stdio
  - DAP: codelldb
- Future versions will load these from the user TOML.

Using the components
- File Tree overlay: Ctrl+Shift+O to open; navigate with arrows/PageUp/PageDown; Enter opens file in editor overlay
- Editor overlay: type to edit, Enter newline, Backspace delete, Ctrl+S save, Esc close; arrow keys move cursor; PageUp/PageDown scroll
- LSP in editor overlay:
  - Completions: Ctrl+Space (accept with Enter; arrows to select)
  - Go to Definition: F12; References list: Shift+F12 (arrows/Enter/Esc)
  - Rename symbol: F2 (type new name, Enter to apply)
  - Hover info: Ctrl+K; Signature help: auto on '(' and ',' with cursor-aware positions (UTF-16 correct)
  - Formatting: Ctrl+Shift+F
  - Diagnostics: underlines with severity colors + gutter markers
- DAP overlay: Ctrl+Shift+D to toggle; L to Launch (codelldb or debugpy); keys: F9 toggle breakpoint (at current editor cursor line, synced to adapter), F5 Continue, F10 Step Over, F11 Step In, Shift+F11 Step Out; shows threads, stack, and variables when stopped
- CLI: `openagent-terminal web-edit <file>` opens the editor overlay on startup

Notes
- These components are scaffolded to compile and run as part of the terminal process. UI integration (panels and keybindings) is intentionally staged and will land in a follow-up.
- Design remains terminal-first: no embedded web editors. Rendering will integrate with the existing GPU pipeline.
- The LSP client now exposes a DocumentBridge that tracks cursor-aware positions and makes signature help trivial to request from the focused editor buffer.
- The DAP client includes helpers to set breakpoints for files and list threads/stack/scopes/variables for Warp-style debug panes.

