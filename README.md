# OpenAgent Terminal

**AI-enhanced terminal emulator with local privacy and high performance**

[![CI](https://github.com/GeneticxCln/OpenAgent-Terminal/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/GeneticxCln/OpenAgent-Terminal/actions/workflows/ci.yml) [![Latest release](https://img.shields.io/github/v/release/GeneticxCln/OpenAgent-Terminal?include_prereleases&sort=semver)](https://github.com/GeneticxCln/OpenAgent-Terminal/releases) [![License](https://img.shields.io/github/license/GeneticxCln/OpenAgent-Terminal)](https://github.com/GeneticxCln/OpenAgent-Terminal/blob/main/LICENSE-APACHE)

## 💬 Community

**Join our Discord server for support, discussions, and updates:**

[![Discord](https://img.shields.io/badge/Discord-Join%20Server-5865F2?style=for-the-badge&logo=discord&logoColor=white)](https://discord.gg/PP8cRXAz3V)

🗨️ **[Join Discord Community](https://discord.gg/PP8cRXAz3V)**
- Get help with installation and setup
- Share your AI terminal workflows
- Discuss feature requests and bug reports
- Connect with other OpenAgent Terminal users

---

## Overview

OpenAgent Terminal combines the speed of [Alacritty](https://github.com/alacritty/alacritty) with AI command assistance. Convert natural language to shell commands without sacrificing performance or privacy.

**Key Features:**
- 🤖 **AI Command Generation** - Natural language to shell commands
- 🔒 **Privacy-First** - Local AI with Ollama (default), optional cloud providers
- ⚡ **High Performance** - <50MB memory, GPU rendering, <100ms startup
- 🎨 **Modern UI** - Command blocks, workflows, Warp-style interface
- 🔐 **Security Lens** - Risk analysis for AI-suggested commands

## Quick Start

### 1. Build & Install
```bash
git clone https://github.com/GeneticxCln/OpenAgent-Terminal.git
cd OpenAgent-Terminal
cargo build --release
```

### 2. Setup AI (Optional)

**Local AI (Recommended):**
```bash
curl -fsSL https://ollama.ai/install.sh | sh
ollama serve
ollama pull codellama
```

Security note: Always review install scripts before piping curl to sh. Prefer your OS package manager or the official installer when available. See docs/AI_ENVIRONMENT_SECURITY.md.

**Cloud AI:**
```bash
# OpenAI
export OPENAGENT_OPENAI_API_KEY="your-key"

# Anthropic  
export OPENAGENT_ANTHROPIC_API_KEY="your-key"
```

Client environment variables (preferred)
- For local Ollama client config, set:
```bash
export OPENAGENT_OLLAMA_ENDPOINT="http://localhost:11434"
export OPENAGENT_OLLAMA_MODEL="codellama:7b"
```
Note: OLLAMA_HOST configures the Ollama server process/container; it is not read by OpenAgent Terminal. Use OPENAGENT_OLLAMA_* on the client side.

### 3. Usage

**AI Assistant:**
- `Ctrl+Shift+A` - Open AI panel
- Type: "find all python files modified last week"
- Get intelligent command suggestions
- Never auto-executes - always your choice

**Built-in Features:**
- `Ctrl+Shift+P` - Command Palette
- `Ctrl+Shift+S` - Block Search  
- `Ctrl+Shift+W` - Workflows Panel
- `Alt+f` - Toggle command folding
- `Alt+j/k` - Navigate between blocks

## Command Notebooks (CLI)

Record and run sequences of shell commands with outputs, like a lightweight terminal notebook.

Examples:

```bash
openagent-terminal notebook create "Setup"
openagent-terminal notebook add-command --notebook Setup --command "echo hello"
openagent-terminal notebook add-markdown --notebook Setup --text "## Step 1"
openagent-terminal notebook show Setup
openagent-terminal notebook run --notebook Setup
```

Data is stored under:
- Linux: ~/.local/share/openagent-terminal/notebooks/notebooks.db

Note: Command notebooks integrate with Blocks (history) and will link each executed cell to a Block for search/export.

## Configuration

### AI streaming, logs, and history storage

- OPENAGENT_AI_STREAM_REDRAW_MS: Throttle AI streaming redraws and batch chunk flushes; default 16 ms.
  - Lower values (e.g., 8) update more frequently during AI streaming; higher (e.g., 32) reduce redraw load.
- Paste preview privacy: Previews strip ANSI escape codes, truncate to 10 lines (~1200 chars), and redact obvious secrets (e.g., Authorization: Bearer ..., api_key=..., password: ...).
- Verbose AI logs: Set OPENAGENT_AI_LOG_VERBOSITY=summary or verbose to log AI streaming events and proposal outcomes. Frame timings are logged with render.frame and render.frame_complete spans via tracing.

AI history storage and export
- Location: Linux: ~/.local/share/openagent-terminal/ai_history/
- Files:
  - history.db (SQLite) — primary store
  - history.jsonl (JSON Lines) — append-only log with one JSON object per line
- CLI export: If the SQLite database isn’t available, the CLI automatically falls back to exporting from history.jsonl (supports --format json or csv).

### Workspace pane drag and precise tab drop targets

- Pane drag gesture is configurable under the workspace section in your config:

```toml path=null start=null
[workspace.drag]
# Enable Alt+Left-drag to move panes between splits/tabs (default: true)
enable_pane_drag = true
# Modifier required to start a pane drag: "Alt" | "Ctrl" | "Shift" | "None"
pane_drag_modifier = "Alt"
# Mouse button used to start a pane drag: "Left" | "Middle" | "Right"
pane_drag_button = "Left"
```

- The tab bar now caches precise pixel bounds for all tabs during rendering to improve drag-and-drop accuracy. When dragging a pane over the tab strip, drop targets use these cached bounds (falling back to even-width approximation only when bounds are unavailable).

### Rendering (WGPU): Subpixel text & gamma

When using the WGPU backend, you can enable LCD subpixel text rendering and tune its gamma/orientation under the `[debug]` section:

```toml path=null start=null
[debug]
subpixel_text = "Enabled"       # "Auto" | "Enabled" | "Disabled"
subpixel_orientation = "RGB"     # "RGB" | "BGR"
subpixel_gamma = 2.2             # Typical range: 1.8 – 2.4
```

Runtime shortcuts:
- Toggle subpixel: Ctrl+Shift+L (Cmd+Shift+L on macOS)
- Cycle RGB/BGR: Ctrl+Shift+Y (Cmd+Shift+Y)
- Perf HUD: Ctrl+Shift+F (Cmd+Shift+F)
- Gamma +/−/reset: Ctrl+Shift+G / Ctrl+Shift+H / Ctrl+Shift+R (Cmd+Shift+… on macOS)

Rendering backend:

- WGPU only. OpenGL fallback has been removed.
- If WGPU initialization fails on your system, the app will exit with an error explaining why.

Minimal AI config (`~/.config/openagent-terminal/openagent-terminal.toml`):

```toml
[ai]
enabled = true
provider = "ollama"  # or "openai", "anthropic", "openrouter"
trigger_key = "Ctrl+Shift+A"
never_auto_run = true  # Safety first
```

See `examples/openagent-terminal.example.toml` for a fuller starter configuration.

## Build variants & features

Plugin system status

- v1.0 supports WebAssembly (WASM/WASI) plugins only via the Wasmtime-based loader
- Native host-integration is not supported in v1.0 and remains on the roadmap; no native plugin runtime or UI is exposed to end users

Common builds for the main binary (feature-gated components):

- Terminal only (no AI, default features):
  ```bash
  cargo build -p openagent-terminal
  ```
- With local AI (Ollama):
  ```bash
  cargo build -p openagent-terminal --features "ai-ollama"
  ```
- With OpenAI, Anthropic, or OpenRouter:
  ```bash
  cargo build -p openagent-terminal --features "ai-openai"
  # or
  cargo build -p openagent-terminal --features "ai-anthropic"
  # or
  cargo build -p openagent-terminal --features "ai-openrouter"
  ```
- With plugin system (WASI sandbox) and workflows:
  ```bash
  cargo build -p openagent-terminal --features "plugins,workflow"
  ```
- Full dev build (no AI provider):
  ```bash
  cargo build -p openagent-terminal --features "full"
  ```
- Full set for local development (AI + plugins + security lens):
  ```bash
  cargo build -p openagent-terminal --features "full,ai-ollama"
  ```

Notes:
- AI is opt-in at build time; pick a single backend umbrella feature (ai-ollama, ai-openai, ai-anthropic, ai-openrouter).
- Secrets must be supplied via environment variables; never hardcode API keys.
- Renderer is WGPU-only by default; X11/Wayland features are included via defaults.

## Dev Tools (Node/TypeScript)

- Location: ./.dev
- Node.js: >= 20.x (CI uses Node 20)
- Typical workflow:

```bash
cd .dev
npm ci
npm run type-check
npm run lint
npm run build
npm run test
```

Notes:
- The GitHub Actions workflow runs these with working-directory set to ./.dev.
- The ESLint config lives at .dev/eslint.config.js and targets src/**/*.ts at the repo root via scripts.

## Rust version policy (MSRV)

- MSRV: 1.79.0 (declared in [workspace.package.rust-version] in Cargo.toml)
- Policy:
  - Crates inherit rust-version from the workspace by default (rust-version.workspace = true).
  - If a crate explicitly opts into a newer edition (e.g., edition = "2024") or uses language features that need a higher compiler, set a crate-specific rust-version in that Cargo.toml.
  - CI enforces MSRV builds and validates that any edition = "2024" crate has a rust-version set.

## Installation

- See INSTALL.md for platform-specific install and troubleshooting.
- Accessibility guidance: docs/ACCESSIBILITY.md

**From Source:**
```bash
cargo install --git https://github.com/GeneticxCln/OpenAgent-Terminal.git
```

**Releases:**
Download prebuilt binaries from [GitHub Releases](https://github.com/GeneticxCln/OpenAgent-Terminal/releases).

## Privacy & Security

No cloud accounts or sync
- OpenAgent Terminal intentionally excludes cloud account systems and hosted sync.
- Any synchronization is strictly optional, disabled by default, and intended for local or self-hosted setups only (behind a feature flag).
- The project is open-source and local-first by design.

- **Local by default** - Ollama runs entirely on your machine
- **No telemetry** - Zero data collection
- **Command analysis** - Built-in Security Lens reviews AI suggestions
- **Never auto-executes** - You approve every command
- **Open source** - Audit the code yourself

## Documentation

Privacy & UI testing
- docs/PRIVACY_AND_UI.md
- docs/TESTING_PRIVACY_UI_COMPLIANCE.md

Metrics exporter (Prometheus)
- Enable via `--metrics-port <port>` or `OPENAGENT_PROM_PORT`.
- Scrape `http://127.0.0.1:<port>/metrics`.

- Documentation Hub: docs/README.md
- Installation Guide: INSTALL.md
- Configuration Manual: openagent-terminal/docs/configuration.md  
- AI Environment Security: docs/AI_ENVIRONMENT_SECURITY.md
- AI Architecture: docs/adr/001-ai-architecture.md
- Security Lens: openagent-terminal/docs/security_lens.md
- Contributing: CONTRIBUTING.md
- Full Documentation Index: docs/
- Example configs: ./configs/
- Example env file: ./.env.example (copy to .env and adjust values)

## FAQ

**Q: How is this different from other AI terminals?**  
A: Complete terminal emulator with built-in AI, not a wrapper. Works with any shell, local AI option, based on proven Alacritty performance.

**Q: Is my data safe?**  
A: Yes. Local AI by default, no telemetry, commands never auto-execute, everything is opt-in.

**Q: Does AI slow down the terminal?**  
A: No. AI runs on-demand, asynchronously, with <5MB memory impact and zero rendering overhead.

**Q: Compatible with Alacritty configs?**  
A: Yes. Full backward compatibility with existing Alacritty configurations.

## License

Dual-licensed under Apache-2.0 OR MIT. See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT).

Based on [Alacritty](https://github.com/alacritty/alacritty). See [ATTRIBUTION.md](docs/guides/ATTRIBUTION.md) for details.

---

**Status:** Phase 4 development (Plugin system MVP kickoff, Security Lens polish, WGPU parity).
[Apache License, Version 2.0]: https://github.com/GeneticxCln/OpenAgent-Terminal/blob/main/LICENSE-APACHE
