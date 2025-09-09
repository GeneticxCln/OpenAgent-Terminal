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

**Cloud AI:**
```bash
# OpenAI
export OPENAI_API_KEY="your-key"

# Anthropic  
export ANTHROPIC_API_KEY="your-key"
```

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

Backend selection:

- Default (when built with `wgpu`): try WGPU first, auto‑fallback to OpenGL if needed.
- Force OpenGL only: `OPENAGENT_FORCE_GL=1`
- Disable fallback: `OPENAGENT_DISABLE_GL_FALLBACK=1`
- Prefer OpenGL via config:

```toml
[debug]
prefer_wgpu = false
```

Minimal AI config (`~/.config/openagent-terminal/openagent-terminal.toml`):

```toml
[ai]
enabled = true
provider = "ollama"  # or "openai", "anthropic"
trigger_key = "Ctrl+Shift+A"
never_auto_run = true  # Safety first
```

See `examples/openagent-terminal.example.toml` for a fuller starter configuration.

## Installation

**From Source:**
```bash
cargo install --git https://github.com/GeneticxCln/OpenAgent-Terminal.git
```

**Releases:**
Download prebuilt binaries from [GitHub Releases](https://github.com/GeneticxCln/OpenAgent-Terminal/releases).

## Privacy & Security

- **Local by default** - Ollama runs entirely on your machine
- **No telemetry** - Zero data collection
- **Command analysis** - Built-in Security Lens reviews AI suggestions
- **Never auto-executes** - You approve every command
- **Open source** - Audit the code yourself

## Documentation

- [Installation Guide](INSTALL.md)
- [Configuration Manual](docs/configuration.md)  
- [AI Architecture](docs/adr/001-ai-architecture.md)
- [Security Lens](docs/security_lens.md)
- [Contributing](CONTRIBUTING.md)
- [Full Documentation](docs/)

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

Apache 2.0 - Based on [Alacritty](https://github.com/alacritty/alacritty). See [ATTRIBUTION.md](ATTRIBUTION.md) for details.

---

**Status:** Phase 4 development (Plugin system, Security Lens, WGPU) - See [STATUS.md](STATUS.md)
[Apache License, Version 2.0]: https://github.com/GeneticxCln/OpenAgent-Terminal/blob/main/LICENSE-APACHE
