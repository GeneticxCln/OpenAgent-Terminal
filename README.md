<p align="center">
    <img width="400" alt="OpenAgent Terminal Logo" src="extra/logo/openagent-terminal.png" style="max-width: 100%; height: auto;">
</p>

<h1 align="center">OpenAgent Terminal</h1>
<h3 align="center">The AI-Enhanced Terminal Emulator</h3>
<p align="center">A fast, cross-platform terminal with built-in AI assistance for command generation and shell automation</p>

<p align="center">
  <a href="https://github.com/GeneticxCln/OpenAgent-Terminal/actions/workflows/ci.yml">
    <img alt="CI" src="https://github.com/GeneticxCln/OpenAgent-Terminal/actions/workflows/ci.yml/badge.svg?branch=main">
  </a>
  <a href="https://github.com/GeneticxCln/OpenAgent-Terminal/releases">
    <img alt="Latest release" src="https://img.shields.io/github/v/release/GeneticxCln/OpenAgent-Terminal?include_prereleases&sort=semver">
  </a>
  <a href="https://github.com/GeneticxCln/OpenAgent-Terminal/blob/main/LICENSE-APACHE">
    <img alt="License" src="https://img.shields.io/github/license/GeneticxCln/OpenAgent-Terminal">
  </a>
</p>

## About

OpenAgent Terminal is an **AI-enhanced terminal emulator** that combines the speed and reliability of [Alacritty](https://github.com/alacritty/alacritty) with powerful AI capabilities for command assistance and automation. It's designed for developers who want intelligent help without sacrificing performance or privacy.

> 🚀 **From broken fork to a fast, AI-enhanced terminal in 3 phases (continuing with Phase 4).**

### Key Features

🤖 **AI-Powered Command Assistance**
- Natural language to shell commands
- Multiple AI providers (Ollama, OpenAI, Anthropic)
- Context-aware suggestions based on your environment
- Never auto-executes commands for safety
- Smart command history navigation

🔒 **Privacy-First Design**
- **Local AI with Ollama by default** - no data leaves your machine
- All cloud features are opt-in
- No telemetry or data collection
- Your terminal history stays private
- API keys stored securely in environment variables
- **Security Lens integration** - AI-suggested commands are analyzed for risks

⚡ **High Performance**
- GPU-accelerated rendering (inherited from Alacritty)
- Minimal resource usage (<50MB base, <150MB with AI)
- Fast startup time (<100ms)
- Smooth scrolling and text rendering at 60fps
- Non-blocking AI operations

🎨 **Modern Features**
- Command block folding
- Settings synchronization (coming soon)
- Extensive configuration options
- Cross-platform support (Linux, macOS, Windows, BSD)
- Provider-agnostic AI architecture

The software is actively developed and used in production by developers who value both performance and intelligent assistance.

Precompiled binaries are available from the [GitHub releases page](https://github.com/GeneticxCln/OpenAgent-Terminal/releases).


## What Makes OpenAgent Terminal Different?

Unlike traditional terminals, OpenAgent Terminal understands what you're trying to do:

```bash
# Press Ctrl+Shift+A and type:
"find all large files over 100MB"

# Get intelligent suggestions:
find / -type f -size +100M 2>/dev/null
du -h / 2>/dev/null | grep '[0-9]\{3\}M'
```

### Core Features
- **AI Command Generation**: Convert natural language to shell commands
- **Multi-Provider Support**: Choose between local (Ollama) or cloud AI (OpenAI, Anthropic)
- **Smart Context**: AI understands your shell, directory, and platform
- **Command Safety**: Reviews commands before execution, never auto-runs
- **Fast & Lightweight**: Built on Alacritty's proven performance

For a complete feature list, see [docs/features.md](docs/features.md)

## Quick Start with AI

### 1. Install OpenAgent Terminal
```bash
git clone https://github.com/GeneticxCln/OpenAgent-Terminal.git
cd OpenAgent-Terminal
cargo build --release --features "ai ai-ollama"
```

### 2. Set up AI Provider

#### Option A: Ollama (Recommended - Local & Private)
```bash
# Install Ollama
curl -fsSL https://ollama.ai/install.sh | sh

# Start Ollama service
ollama serve

# Pull a model
ollama pull codellama
```

#### Option B: OpenAI
```bash
export OPENAI_API_KEY="your-api-key"
```

#### Option C: Anthropic
```bash
export ANTHROPIC_API_KEY="your-api-key"
```

### 3. Configure AI Assistant

Create or edit your config file with the AI section:

```toml
[ai]
# Basic settings
enabled = true
provider = "ollama"  # or "openai", "anthropic"
trigger_key = "Ctrl+Shift+A"

# Panel appearance
panel_height_fraction = 0.40  # 40% of screen height
backdrop_alpha = 0.25         # dim background
rounded_corners = true
corner_radius_px = 12.0
shadow = true
shadow_alpha = 0.35
shadow_size_px = 8

# Behavior
auto_focus = true           # focus panel when opened
animated_typing = true      # animate AI responses
animation_speed = 1.0       # animation speed multiplier
propose_max_commands = 10   # max suggestions per query
never_auto_run = true       # safety: never auto-execute
inline_suggestions = false  # show suggestions as you type

# Logging
log_verbosity = "summary"    # "off", "summary", "verbose"
```

See `example_config.toml` for complete configuration options.

### 4. Use AI Assistant
- Press `Ctrl+Shift+A` to toggle AI panel
- Type your request in natural language
- Press `Enter` to get suggestions
- Use arrow keys to navigate proposals
- Press `Ctrl+I` to insert selected command
- Press `Ctrl+E` to apply dry-run analysis
- Press `Ctrl+C` to copy selected command
- Press `Ctrl+R` to regenerate suggestions
- Press `Esc` to close panel

### Example AI Queries
```
"find all python files modified in the last week"
"compress this directory into a tar.gz"
"show system resource usage"
"git commit with conventional commit message"
"setup a python virtual environment"
```

## Architecture & Documentation

### Security Lens (Command Safety)
OpenAgent Terminal ships with a built-in Security Lens that analyzes commands before they run. It classifies risk as Safe, Caution, Warning, or Critical and can require a confirmation overlay or block Critical commands outright. You can add your own org-specific regex patterns.

Quick example (TOML):
```toml path=null start=null
[security]
enabled = true
block_critical = true

[security.require_confirmation]
Safe = false
Caution = true
Warning = true
Critical = true

[[security.custom_patterns]]
pattern = "(?i)kubectl\s+delete\s+ns\s+prod"
risk_level = "Critical"
message = "Deleting the production namespace"
```
See full docs and UX flow: docs/security_lens.md

- **Architecture Overview**: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- **AI Design Decisions**: [docs/adr/001-ai-architecture.md](docs/adr/001-ai-architecture.md)
- **Sync Protocol Design**: [docs/adr/002-sync-protocol.md](docs/adr/002-sync-protocol.md)
- **Development Plan**: [DEVELOPMENT_PLAN.md](DEVELOPMENT_PLAN.md)
- **Quick Start Guide**: [docs/QUICK_START_DEVELOPMENT.md](docs/QUICK_START_DEVELOPMENT.md)
- **Security Lens & Confirmations**: [docs/security_lens.md](docs/security_lens.md)
- **Plugins & Workflows**: [docs/plugins.md](docs/plugins.md), [docs/workflows.md](docs/workflows.md)

## Further information

- Releases: https://github.com/GeneticxCln/OpenAgent-Terminal/releases
- Changelog: [CHANGELOG.md](CHANGELOG.md)
- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md)

## Installation

OpenAgent Terminal can be installed by using various package managers on Linux, BSD,
macOS and Windows.

Prebuilt binaries for macOS and Windows can also be downloaded from the
[GitHub releases page](https://github.com/GeneticxCln/OpenAgent-Terminal/releases).

For everyone else, the detailed instructions to install OpenAgent Terminal can be found
[here](INSTALL.md).

### Requirements

- At least OpenGL ES 2.0
- [Windows] ConPTY support (Windows 10 version 1809 or higher)

## Configuration

You can find the documentation for OpenAgent Terminal's configuration in `man 5
openagent-terminal`.

OpenAgent Terminal doesn't create the config file for you, but it looks for one in the
following locations:

1. `$XDG_CONFIG_HOME/openagent-terminal/openagent-terminal.toml`
2. `$XDG_CONFIG_HOME/openagent-terminal.toml`
3. `$HOME/.config/openagent-terminal/openagent-terminal.toml`
4. `$HOME/.openagent-terminal.toml`
5. `/etc/openagent-terminal/openagent-terminal.toml`

On Windows, the config file will be looked for in:

* %APPDATA%\\openagent-terminal\\openagent-terminal.toml

### Theming (new)

You can enable theming via the `[theme]` section in your config or by pointing to a custom
TOML theme file. Built-in themes: `dark`, `light`, `high-contrast-dark`.

Example config snippet:

```toml
[theme]
# choose a built-in theme or provide a custom path
name = "dark"           # or "light", "high-contrast-dark"
# path = "/path/to/custom_theme.toml"  # overrides `name` when set

# global preference (respected by UI animations)
reduce_motion = false

# optional visual overrides
rounded_corners = true
corner_radius_px = 12.0
shadow = true
shadow_alpha = 0.35
shadow_size_px = 8
```

Sample themes are available in `extra/themes/`.

## Troubleshooting

- Wayland vs X11 (Linux)
  - The window backend is auto-detected. To force one:
    - Wayland: `WINIT_UNIX_BACKEND=wayland`
    - X11: `WINIT_UNIX_BACKEND=x11`
  - If you experience a blank window, input focus issues, or decoration glitches (especially on NVIDIA + Wayland), try forcing the other backend.

- GPU drivers
  - Minimum requirement for the default renderer is OpenGL ES 2.0 (GL/GLES via your driver).
  - Ensure drivers are installed and up to date:
    - AMD/Intel: mesa (OpenGL), mesa-vulkan-drivers (for Vulkan)
    - NVIDIA: proprietary driver + nvidia-utils + Vulkan loader (libvulkan)
  - Diagnostics: `glxinfo -B` (OpenGL) and `vulkaninfo` (Vulkan) should succeed.

- wgpu vs OpenGL backends
  - Default build uses OpenGL. An optional wgpu renderer is available via the feature flag.
    - Build with wgpu: `cargo build -p openagent-terminal --features wgpu`
  - If using wgpu, you can force a specific backend:
    - Vulkan: `WGPU_BACKEND=vk`
    - OpenGL: `WGPU_BACKEND=gl` (useful when Vulkan is unavailable or unstable)
  - If you see errors like "No adapters found" or "device lost", try switching backend or updating drivers.

- Hybrid/discrete GPUs (Linux)
  - To run on the discrete GPU: `DRI_PRIME=1 openagent-terminal`

## Contributing

A guideline about contributing to OpenAgent Terminal can be found in the
[`CONTRIBUTING.md`](CONTRIBUTING.md) file.

## FAQ

**_How is this different from GitHub Copilot CLI or other AI terminals?_**

OpenAgent Terminal is a complete terminal emulator with AI built-in, not a wrapper or plugin. This means:
- No additional tools or subscriptions required
- Works with any shell or command-line tool
- Local AI option (Ollama) for complete privacy
- Integrated seamlessly into your terminal workflow
- Based on the proven Alacritty codebase for reliability

**_Is my data safe? What about privacy?_**

Privacy is a core design principle:
- **Local by default**: Ollama runs entirely on your machine
- **Opt-in cloud**: Cloud providers (OpenAI, Anthropic) require explicit configuration
- **No telemetry**: We don't collect any usage data
- **No auto-execution**: AI never runs commands without your approval
- **Open source**: You can audit the code yourself

**_Does AI slow down the terminal?_**

No! The AI features are:
- Activated on-demand (Ctrl+Shift+A)
- Run asynchronously without blocking the terminal
- Add less than 5MB to memory usage
- Have zero impact on terminal rendering performance

**_Can I use my existing Alacritty config?_**

Yes! OpenAgent Terminal maintains full compatibility with Alacritty configurations. Your existing `alacritty.yml` or `alacritty.toml` will work. AI features are added through new configuration sections that don't interfere with existing settings.

**_Is it really the fastest terminal emulator?_**

OpenAgent Terminal inherits Alacritty's exceptional performance. Benchmarks using [vtebench](https://github.com/alacritty/vtebench) show consistent top-tier performance. The AI features add negligible overhead since they run on-demand and don't affect the rendering pipeline.

**_Why isn't feature X implemented?_**

OpenAgent Terminal focuses on being a fast, AI-enhanced terminal. Features like tabs or splits are intentionally left to window managers or [terminal multiplexers][tmux]. This keeps the codebase lean and maintains performance.

[tmux]: https://github.com/tmux/tmux

## Project Status

This repository’s canonical, up-to-date status lives in [STATUS.md](STATUS.md).

Summary:
- ✅ Phase 1: Foundation & Identity — complete
- ✅ Phase 2: Core AI Implementation — complete
- ✅ Phase 3: AI UI and integration — complete
- 🚧 Phase 4: Plugin system, Security Lens, WGPU parity, and testing infrastructure — in progress

For detailed timelines and next milestones, see [STATUS.md](STATUS.md) and the [Development Plan](DEVELOPMENT_PLAN.md).

## Attribution

OpenAgent Terminal is built on the solid foundation of [Alacritty](https://github.com/alacritty/alacritty). We're grateful to the Alacritty team for creating such an excellent terminal emulator. See [ATTRIBUTION.md](ATTRIBUTION.md) for full details.

## License

OpenAgent Terminal is released under the [Apache License, Version 2.0].

[Apache License, Version 2.0]: https://github.com/GeneticxCln/OpenAgent-Terminal/blob/main/LICENSE-APACHE

