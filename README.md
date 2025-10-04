# OpenAgent-Terminal

**The First AI-Native Terminal Emulator**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Status: Alpha](https://img.shields.io/badge/Status-Alpha-orange.svg)]()
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![Python](https://img.shields.io/badge/Python-3.9+-blue.svg)](https://www.python.org/)

> **⚠️ Project Status:** This project is in early development (Phase 1). Not ready for production use.

## Vision

OpenAgent-Terminal combines the GPU-accelerated rendering power of modern terminal emulators with the intelligence of AI agents, creating a seamless developer experience where AI assistance is built directly into your terminal workflow.

### What Makes It Different?

Unlike traditional terminals with bolted-on AI features, OpenAgent-Terminal is **AI-native from the ground up**:

- 🎨 **GPU-Accelerated UI** - Smooth 60 FPS rendering with WGPU
- 🤖 **Integrated AI Agent** - OpenAgent intelligence built-in, not add-on
- 📦 **Rich Block Rendering** - Code, diffs, and tool outputs beautifully formatted
- 🛡️ **Safety First** - Visual approval for all tool executions
- ⚡ **Real-Time Streaming** - See AI responses as they're generated
- 🎯 **Context-Aware** - AI knows your shell state, recent commands, and errors

## Screenshots

> Coming soon! We're in Phase 1 of development.

## Quick Start

### Prerequisites

```bash
# Rust 1.70+ (for frontend)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Python 3.9+ (for backend)
python --version

# OpenAgent (clone alongside this project)
git clone https://github.com/yourusername/OpenAgent.git
```

### Installation (Development)

```bash
# Clone this repository
git clone https://github.com/yourusername/openagent-terminal.git
cd openagent-terminal

# Build Rust frontend
cargo build --release

# Install Python backend
cd backend
pip install -e .

# Copy Portal assets (font)
mkdir -p assets
cp ../Portal/assets/DejaVuSansMono.ttf assets/
```

### Running

```bash
# Terminal 1: Start Python backend
cd backend
python -m openagent_terminal.bridge --debug

# Terminal 2: Run Rust frontend
cargo run --release
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  OpenAgent-Terminal                      │
│                                                           │
│  ┌──────────────────┐         ┌──────────────────────┐ │
│  │  Rust Frontend   │◄───────►│  Python Backend      │ │
│  │  (Portal-based)  │  IPC    │  (OpenAgent Core)    │ │
│  │                  │         │                       │ │
│  │ • WGPU Render    │         │ • Agent System       │ │
│  │ • PTY Manager    │         │ • LLM Integration    │ │
│  │ • Block UI       │         │ • Tool Execution     │ │
│  └──────────────────┘         └──────────────────────┘ │
│           │                            │                 │
│           └────────────────────────────┘                 │
│              JSON-RPC over Unix Socket                   │
└─────────────────────────────────────────────────────────┘
```

**Components:**
- **Rust Frontend:** GPU-accelerated terminal based on Portal (fredg-wgpu-terminal)
- **Python Backend:** AI intelligence powered by OpenAgent framework
- **IPC:** High-performance JSON-RPC over Unix domain sockets

## Features

### Current (Phase 1 - In Progress)
- [ ] Basic IPC communication
- [ ] Unix socket connection
- [ ] JSON-RPC protocol
- [ ] Handshake and initialization

### Planned

#### Phase 2: Core Integration (Weeks 3-4)
- [ ] Agent query/response cycle
- [ ] Real-time token streaming
- [ ] Loading indicators
- [ ] Error handling

#### Phase 3: Block Rendering (Weeks 5-6)
- [ ] Syntax-highlighted code blocks
- [ ] Diff visualization
- [ ] Block folding/unfolding
- [ ] Export blocks to files

#### Phase 4: Tool Integration (Weeks 7-8)
- [ ] Tool approval UI
- [ ] Progress visualization
- [ ] Diff previews
- [ ] Rollback capability

#### Phase 5: Advanced Features (Weeks 9-12)
- [ ] Multi-pane layouts
- [ ] Session persistence
- [ ] Inline command suggestions
- [ ] Command explanation on hover

See [ROADMAP.md](ROADMAP.md) for detailed timeline.

## Documentation

- **[Technical Design](DESIGN.md)** - Architecture and integration strategy
- **[IPC Protocol](docs/IPC_PROTOCOL.md)** - Communication protocol specification
- **[Roadmap](ROADMAP.md)** - Implementation timeline and milestones
- **[Contributing](CONTRIBUTING.md)** - How to contribute (coming soon)

## Development

### Project Structure

```
openagent-terminal/
├── src/                    # Rust source code
│   ├── main.rs            # Entry point
│   ├── ipc/               # IPC client implementation
│   ├── agent/             # Agent integration
│   ├── ui/                # UI rendering
│   └── terminal/          # Terminal emulation (from Portal)
├── backend/               # Python backend
│   └── openagent_terminal/
│       ├── bridge.py      # IPC server
│       ├── agent_handler.py
│       └── stream_adapter.py
├── docs/                  # Documentation
├── examples/              # Example usage
├── assets/                # Fonts and resources
└── tests/                 # Integration tests
```

### Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run
```

### Testing

```bash
# Rust unit tests
cargo test

# Python tests
cd backend && pytest

# Integration tests (requires both frontend and backend running)
./scripts/test_integration.sh
```

## Roadmap

**Current Phase:** Phase 1 - Foundation (Weeks 1-2)
**Next Milestone:** v0.1.0 - MVP (End of Phase 2)

| Version | Target | Status | Features |
|---------|--------|--------|----------|
| v0.1.0 | Week 4 | 🔨 In Progress | IPC, Agent Query/Response |
| v0.2.0 | Week 6 | 📋 Planned | Block Rendering, Syntax Highlighting |
| v0.3.0 | Week 8 | 📋 Planned | Tool Approval, Progress UI |
| v0.4.0 | Week 12 | 📋 Planned | Multi-pane, Sessions, Suggestions |
| v1.0.0 | Q2 2026 | 🎯 Target | Production Release |

## Contributing

We welcome contributions! However, note that this project is in very early development (Phase 1).

### How to Help

**Right now (Phase 1):**
- 🐛 Report bugs in IPC implementation
- 📝 Improve documentation
- 💡 Provide feedback on design documents
- ⭐ Star the project to show support

**Soon (Phase 2+):**
- 💻 Code contributions
- 🎨 UI/UX improvements
- 🧪 Testing and QA
- 📖 Writing guides and tutorials

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines (coming soon).

## Technology Stack

### Rust (Frontend)
- **winit + wgpu** - Window and GPU rendering
- **wgpu_glyph** - Text rendering
- **tokio** - Async runtime
- **serde_json** - JSON serialization
- **portable-pty** - PTY management
- **vte** - Terminal parser
- **syntect** - Syntax highlighting

### Python (Backend)
- **OpenAgent** - AI agent framework
- **asyncio** - Async I/O
- **jsonrpcserver** - JSON-RPC handling
- **Transformers / Ollama** - LLM backends

## Performance Targets

- ⚡ **Startup:** < 2 seconds
- 🔄 **IPC Latency:** < 10ms
- 🖼️ **Rendering:** 60 FPS constant
- 💾 **Memory:** < 500MB with agent loaded
- 📊 **Token Streaming:** < 50ms per token

## Comparison

| Feature | OpenAgent-Terminal | Warp | Fig | GitHub Copilot CLI |
|---------|-------------------|------|-----|-------------------|
| Open Source | ✅ | ❌ | ❌ | ❌ |
| Local LLMs | ✅ | ❌ | ❌ | ❌ |
| GPU Rendering | ✅ | ✅ | ❌ | ❌ |
| Block UI | ✅ | ✅ | Limited | ❌ |
| Tool Approval | ✅ | ❌ | ❌ | ❌ |
| Customizable | ✅ | Limited | Limited | ❌ |
| Self-Hosted | ✅ | ❌ | ❌ | ❌ |
| Privacy | ✅ | ❌ | ❌ | ❌ |

## FAQ

### Why not just use Warp?
Warp is excellent but closed-source and cloud-dependent. OpenAgent-Terminal gives you full control and privacy with local LLMs.

### Can I use this with GPT-4/Claude?
Future versions will support remote LLMs, but the focus is on local-first AI with OpenAgent.

### What about Windows support?
Phase 1 focuses on Linux. macOS and Windows support planned for later phases.

### How is this different from just running OpenAgent in a terminal?
OpenAgent-Terminal integrates AI deeply into the terminal UI with block rendering, visual tool approval, and seamless streaming - not just text responses.

### Will this replace my current terminal?
Eventually, yes! But we're not there yet. This is an early-stage project.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

This project combines two awesome projects:

- **[OpenAgent](../OpenAgent/)** - Powerful AI agent framework by GeneticxCln
- **[Portal (fredg-wgpu-terminal)](../Portal/)** - Modern GPU-accelerated terminal

Special thanks to:
- The Rust community for excellent tooling
- The OpenAgent contributors
- The terminal emulator community for inspiration

## Contact

- **Issues:** [GitHub Issues](https://github.com/yourusername/openagent-terminal/issues)
- **Discussions:** [GitHub Discussions](https://github.com/yourusername/openagent-terminal/discussions)
- **Documentation:** [docs/](docs/)

---

**Status:** 🚧 Early Development - Phase 1 in progress  
**Last Updated:** 2025-10-04  
**Next Update:** End of Phase 1 (Week 2)

⭐ **Star this repo to follow our progress!**
