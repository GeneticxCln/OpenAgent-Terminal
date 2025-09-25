<div align="center">

# 🚀 OpenAgent Terminal

**The Fast, Modern Terminal Emulator**

*Blazing fast, GPU-accelerated terminal with Warp-inspired features. Built for performance and simplicity.*

[![CI](https://img.shields.io/github/actions/workflow/status/GeneticxCln/OpenAgent-Terminal/ci.yml?branch=main&style=for-the-badge&logo=github)](https://github.com/GeneticxCln/OpenAgent-Terminal/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/GeneticxCln/OpenAgent-Terminal?include_prereleases&sort=semver&style=for-the-badge&logo=github)](https://github.com/GeneticxCln/OpenAgent-Terminal/releases)
[![License](https://img.shields.io/badge/License-Apache%202.0%20%7C%20MIT-blue?style=for-the-badge)](https://github.com/GeneticxCln/OpenAgent-Terminal/blob/main/LICENSE-APACHE)
[![Stars](https://img.shields.io/github/stars/GeneticxCln/OpenAgent-Terminal?style=for-the-badge)](https://github.com/GeneticxCln/OpenAgent-Terminal/stargazers)

[🎯 Features](#-key-features) • [⚡ Quick Start](#-quick-start) • [📚 Documentation](#-documentation) • [💬 Community](#-community)

---

</div>

## 🌟 What Makes OpenAgent Terminal Special?

<table>
<tr>
<td width="33%" align="center">

### 🖥️ **Modern Interface**
Clean, Warp-inspired UI with command blocks, tabs, and intelligent completions

</td>
<td width="33%" align="center">

### ⚡ **Blazing Performance** 
GPU-accelerated rendering with <50MB memory usage and <100ms startup time

</td>
<td width="33%" align="center">

### 🔒 **Privacy-First Design**
No telemetry, no data collection, complete transparency and control

</td>
</tr>
</table>

## 🎯 Key Features

### ⚡ **Performance Excellence**
- **GPU-Accelerated Rendering**: Powered by WGPU for smooth, responsive UI
- **Memory Efficient**: <50MB RAM usage, minimal resource footprint
- **Lightning Fast**: <100ms startup time, instant command response
- **Based on Alacritty**: Built on proven, battle-tested terminal core

### 🎨 **Modern Terminal Experience**
- **Command Blocks**: Visual command history with collapsible output
- **Smart Completions**: Intelligent command and argument completion
- **Warp-Style Interface**: Modern tabs, panes, and workspace management

### 🔒 **Security & Privacy**
- **Zero Telemetry**: No data collection or tracking
- **Open Source**: Fully auditable codebase
- **Privacy-First**: No external dependencies for core functionality

## ⚡ Quick Start

### 🛠️ Installation

#### **Option 1: From Releases (Recommended)**
```bash
# Download the latest release for your platform
wget https://github.com/GeneticxCln/OpenAgent-Terminal/releases/latest/download/openagent-terminal-linux.tar.gz
tar -xzf openagent-terminal-linux.tar.gz
sudo mv openagent-terminal /usr/local/bin/
```

#### **Option 2: Build from Source**
```bash
# Clone and build
git clone https://github.com/GeneticxCln/OpenAgent-Terminal.git
cd OpenAgent-Terminal

# Basic build
cargo build --release

# Build with completions
cargo build --release --features "completions"
```

#### **Option 3: Cargo Install**
```bash
cargo install --git https://github.com/GeneticxCln/OpenAgent-Terminal.git
```

### 🎮 Usage & Keyboard Shortcuts

<table>
<tr>
<td width="50%">

#### **📋 Command Management**
| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+P` | Command Palette |
| `Ctrl+Shift+S` | Search Command History |
| `Alt+F` | Toggle Command Folding |
| `Alt+J/K` | Navigate Command Blocks |

</td>
<td width="50%">

#### **🚀 Workspace Controls**
| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+T` | New Tab |
| `Ctrl+Shift+D` | Split Horizontal |
| `Ctrl+Shift+Shift+D` | Split Vertical |
| `Ctrl+Shift+W` | Close Pane |

#### **⚙️ General**
| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+P` | Command Palette |
| `F5` | Refresh/Restart |

</td>
</tr>
</table>


## ⚙️ Configuration & Customization

### 🎨 **Themes & Appearance**
```toml
# ~/.config/openagent-terminal/openagent-terminal.toml
[theme]
name = "dark"  # "dark", "light", or custom theme name

[ui]
font_family = "JetBrains Mono"
font_size = 14
opacity = 0.95

[colors]
background = "#1e1e2e"
foreground = "#cdd6f4"
```


### 🚀 **Performance Tuning**
```toml
[performance]
# WGPU backend settings
[debug]
subpixel_text = "Enabled"
subpixel_orientation = "RGB"
subpixel_gamma = 2.2

# General optimization
# stream_redraw_ms = 16  # Lower = more responsive, higher = less CPU
```


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

```

See `examples/openagent-terminal.example.toml` for a fuller starter configuration.

## 🛠️ Build Variants & Features

### 🎯 **Feature-Gated Builds**

Choose the build that matches your needs:

<table>
<tr>
<td width="50%">

#### **📱 Standard Terminal**
```bash
# Standard build
cargo build --release
```
*Full-featured terminal emulator*

#### **🔧 With Completions**
```bash
# With completions support
cargo build --release --features "completions"
```
*Enhanced command completion*

</td>
<td width="50%">

#### **🎆 Advanced Text**
```bash
# With advanced text shaping
cargo build --release --features "harfbuzz"
```
*Enhanced font rendering*

#### **🎯 Custom Build**
```bash
# Pick specific features
cargo build --release --features \
  "completions,harfbuzz"
```
*Tailor to your exact needs*

</td>
</tr>
</table>

### 📝 **Available Features**
- `completions` - Smart command completion
- `harfbuzz` - Advanced text shaping
- `wayland` - Wayland display server support
- `x11` - X11 display server support
- `wgpu` - GPU-accelerated rendering

## 💬 Community

<div align="center">

### **Join Our Growing Community!**

[![Discord](https://img.shields.io/badge/Discord-Join%20Server-5865F2?style=for-the-badge&logo=discord&logoColor=white)](https://discord.gg/PP8cRXAz3V)
[![GitHub Discussions](https://img.shields.io/badge/GitHub-Discussions-181717?style=for-the-badge&logo=github)](https://github.com/GeneticxCln/OpenAgent-Terminal/discussions)
[![Reddit](https://img.shields.io/badge/Reddit-r%2FOpenAgentTerminal-FF4500?style=for-the-badge&logo=reddit&logoColor=white)](https://reddit.com/r/OpenAgentTerminal)

</div>

### 🌟 **Get Involved**
- **🗨️ [Discord](https://discord.gg/PP8cRXAz3V)** - Real-time chat, support, and discussions
- **📝 [GitHub Discussions](https://github.com/GeneticxCln/OpenAgent-Terminal/discussions)** - Feature requests and long-form discussions
- **🐛 [Issues](https://github.com/GeneticxCln/OpenAgent-Terminal/issues)** - Bug reports and feature requests
- **🔄 [Pull Requests](https://github.com/GeneticxCln/OpenAgent-Terminal/pulls)** - Contribute code and improvements

### 🎆 **Show Your Support**
If you find OpenAgent Terminal useful:
- ⭐ **Star the repo** to show your appreciation
- 🐦 **Share on social media** to spread the word  
- 📝 **Write a blog post** about your experience
- 💰 **[Sponsor the project](https://github.com/sponsors/GeneticxCln)** to support development

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

- **No telemetry** - Zero data collection
- **Privacy-first** - Local-only operation
- **Open source** - Audit the code yourself

## 📚 Documentation

<table>
<tr>
<td width="33%" align="center">

### 🚀 **Getting Started**
[![Install Guide](https://img.shields.io/badge/Install-Guide-blue?style=for-the-badge)](INSTALL.md)
[![Quick Start](https://img.shields.io/badge/Quick-Start-green?style=for-the-badge)](docs/quick-start.md)
[![Examples](https://img.shields.io/badge/Examples-yellow?style=for-the-badge)](examples/)

</td>
<td width="33%" align="center">

### ⚙️ **Configuration**
[![Config Manual](https://img.shields.io/badge/Config-Manual-orange?style=for-the-badge)](docs/configuration.md)
[![Themes](https://img.shields.io/badge/Themes-purple?style=for-the-badge)](docs/themes.md)
[![Keybindings](https://img.shields.io/badge/Keybindings-red?style=for-the-badge)](docs/keybindings.md)

</td>
<td width="33%" align="center">

### 🕰️ **Development**
[![Contributing](https://img.shields.io/badge/Contributing-Guide-lightblue?style=for-the-badge)](CONTRIBUTING.md)
[![Architecture](https://img.shields.io/badge/Architecture-lightgreen?style=for-the-badge)](docs/architecture.md)
[![Testing](https://img.shields.io/badge/Testing-lightyellow?style=for-the-badge)](docs/testing.md)

</td>
</tr>
</table>

### 📎 **Quick Links**
- 📦 **[Installation Guide](INSTALL.md)** - Platform-specific installation instructions
- ⚙️ **[Configuration Manual](docs/configuration.md)** - Complete configuration reference
- ⌨️ **[Keybindings Guide](docs/keybindings.md)** - Keyboard shortcuts and customization
- 🔒 **[Privacy & Security](docs/privacy-security.md)** - Data handling and security practices
- 🎨 **[Themes & Customization](docs/themes.md)** - Create and share custom themes
- 🔧 **[Contributing Guide](CONTRIBUTING.md)** - How to contribute to the project
- 📊 **[Testing Guide](docs/testing.md)** - Running tests and validation
- 🏗️ **[Architecture Overview](docs/architecture.md)** - Technical architecture and design decisions

## ❓ Frequently Asked Questions

<details>
<summary><strong>How is OpenAgent Terminal different from other AI terminals?</strong></summary>
<br>

OpenAgent Terminal is a **modern terminal emulator** with Warp-inspired features. Key differences:

- 🎨 **Full Terminal**: Complete VT100/xterm compatibility, works with any shell
- ⚡ **Performance**: Based on Alacritty's proven architecture for maximum speed
- 🔒 **Privacy**: No telemetry, privacy-first design
- 🎯 **Modern UI**: Command blocks, tabs, and intelligent completions

</details>

<details>
<summary><strong>Is my data safe and private?</strong></summary>
<br>

**Absolutely.** Privacy and security are core principles:

- 🏠 **Local-First**: Default AI (Ollama) runs entirely on your machine
- 🚷 **Zero Telemetry**: No analytics, tracking, or data collection
- 🔐 **No Auto-Execute**: Commands never run without your explicit approval  
- 🔓 **Open Source**: Fully auditable codebase, no hidden behavior
- 🛡️ **Optional Cloud**: Cloud AI providers are opt-in only

</details>

<details>
<summary><strong>Does AI impact terminal performance?</strong></summary>
<br>

**Not at all.** The AI system is designed for zero performance impact:

- 😴 **Lazy Loading**: AI only activates when you request it
- 🔄 **Async Processing**: AI runs in background, never blocks the UI
- 📊 **Minimal Memory**: <5MB additional memory usage when active
- ⚡ **Fast Startup**: <100ms startup time, same as without AI
- 🔥 **GPU Accelerated**: WGPU rendering keeps everything smooth

</details>

<details>
<summary><strong>Can I use my existing Alacritty configuration?</strong></summary>
<br>

**Yes!** OpenAgent Terminal maintains **full backward compatibility**:

- 📋 **Import Existing**: Your current `alacritty.yml` works as-is
- ➕ **Additive Features**: New AI and workflow features are purely additive
- 🔄 **Easy Migration**: Built-in tools to migrate configurations
- 🎆 **Enhanced**: All Alacritty features plus new AI capabilities

</details>

<details>
<summary><strong>What AI providers are supported?</strong></summary>
<br>

Multiple options to fit your preferences:

- 🏠 **Ollama** (Local) - Privacy-first, runs on your hardware
- 🤖 **OpenAI** - GPT-3.5, GPT-4, and latest models
- 🎆 **Anthropic** - Claude models for advanced reasoning
- 🌐 **OpenRouter** - Access to multiple models through one API
- 🕰️ **More Coming**: Additional providers based on community feedback

</details>

---

## 📜 License

**OpenAgent Terminal** is dual-licensed under:

[![Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg?style=for-the-badge)](LICENSE-APACHE) 
[![MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=for-the-badge)](LICENSE-MIT)

You may choose either license for your use case.

### 🙏 **Acknowledgments**

- Built on the foundation of **[Alacritty](https://github.com/alacritty/alacritty)** - The blazing fast, GPU-accelerated terminal emulator
- Powered by **[WGPU](https://wgpu.rs/)** for modern graphics rendering
- AI integration via **[Ollama](https://ollama.ai/)**, **OpenAI**, **Anthropic**, and **OpenRouter**
- Thanks to all our **[contributors](https://github.com/GeneticxCln/OpenAgent-Terminal/graphs/contributors)** who make this project possible

---

<div align="center">

## 🚀 **Ready to Upgrade Your Terminal Experience?**

### **[Download OpenAgent Terminal](https://github.com/GeneticxCln/OpenAgent-Terminal/releases/latest)** • **[Join Discord](https://discord.gg/PP8cRXAz3V)** • **[Read Docs](docs/)**

**Made with ❤️ by the OpenAgent Terminal team and community**

*Experience the future of command-line interfaces today.*

---

⭐ **Don't forget to star the repo if you find it useful!** ⭐

</div>
