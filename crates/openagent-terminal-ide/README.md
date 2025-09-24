# openagent-terminal-ide

IDE features for OpenAgent Terminal (LSP, DAP, editor, indexer).

Features:
- lsp
- editor
- indexer
- dap
- web-editors (GTK4 + WebKitGTK backend)
- all (enables all of the above)

Build (Linux / GTK4 + WebKitGTK):
- Install system libs
  - Arch/EndeavourOS: pacman -S gtk4 webkit2gtk-4.1
  - Ubuntu/Debian: apt-get install libgtk-4-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev libglib2.0-dev pkg-config
- Build with feature:
  - PKG_CONFIG_PATH="$PWD/build-support/pkgconfig:$PKG_CONFIG_PATH" \
    cargo check -p openagent-terminal-ide --no-default-features --features web-editors
- Clippy (warnings-as-errors):
  - PKG_CONFIG_PATH="$PWD/build-support/pkgconfig:$PKG_CONFIG_PATH" \
    cargo clippy -p openagent-terminal-ide --no-default-features --features web-editors -- -D warnings

Testing:
- cargo test -p openagent-terminal-ide
- Feature-specific:
  - cargo test -p openagent-terminal-ide --features lsp
  - cargo test -p openagent-terminal-ide --features editor

Docs:
- ../../docs/TESTING.md
- ../../docs/features.md

License:
- Apache-2.0 OR MIT
