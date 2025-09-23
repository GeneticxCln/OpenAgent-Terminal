# openagent-terminal-ide

IDE features for OpenAgent Terminal (LSP, DAP, editor, indexer).

Features:
- lsp
- editor
- indexer
- dap
- web-editors
- all (enables all of the above)

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
