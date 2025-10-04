# Getting Started with OpenAgent-Terminal

**Welcome to OpenAgent-Terminal!** 🚀

This guide will help you set up your development environment and understand the project structure.

## ⚠️ Important Notice

**Current Status:** Phase 1 - Foundation (Week 1)  
**Development Stage:** Early Alpha - NOT ready for use

This project has just been created (2025-10-04). We're currently in the foundation phase, setting up basic IPC communication. If you're interested in contributing, please read on!

## Project Overview

OpenAgent-Terminal combines two existing projects:
- **[OpenAgent](../OpenAgent/)** - AI agent framework (Python)
- **[Portal](../Portal/)** - GPU-accelerated terminal (Rust)

The goal is to create the first truly AI-native terminal emulator.

## Prerequisites

### Required

- **Rust 1.70+**
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

- **Python 3.9+**
  ```bash
  python --version  # Should be 3.9 or higher
  ```

- **Git**
  ```bash
  git --version
  ```

### Recommended

- **VS Code** with extensions:
  - rust-analyzer
  - Python
  - Even Better TOML

- **Development Tools**
  ```bash
  # Rust formatter and linter
  rustup component add rustfmt clippy
  
  # Python formatter and linter
  pip install black mypy ruff
  ```

## Initial Setup

### 1. Clone the Projects

```bash
cd ~/projects  # Or your preferred location

# Clone OpenAgent (if you haven't already)
git clone https://github.com/yourusername/OpenAgent.git

# Clone Portal (if you haven't already)
git clone https://github.com/yourusername/Portal.git

# Clone this project
git clone https://github.com/yourusername/openagent-terminal.git
cd openagent-terminal
```

Your directory structure should look like:
```
~/projects/
├── OpenAgent/
├── Portal/
└── openagent-terminal/  ← You are here
```

### 2. Set Up Rust Frontend

```bash
cd ~/projects/openagent-terminal

# Build the project
cargo build

# Run tests
cargo test

# Try running it (will show placeholder message)
cargo run
```

**Expected Output:**
```
╔════════════════════════════════════════════╗
║      OpenAgent-Terminal (Alpha)           ║
║   AI-Native Terminal Emulator             ║
╚════════════════════════════════════════════╝

⚠️  Project Status: Phase 1 - Foundation
📦 Current Goal: IPC Communication Setup
...
```

### 3. Set Up Python Backend

```bash
cd ~/projects/openagent-terminal/backend

# Create virtual environment
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install in development mode
pip install -e .

# Install OpenAgent (from parent project)
pip install -e ../../OpenAgent

# Try running the bridge (will show placeholder message)
python -m openagent_terminal.bridge
```

**Expected Output:**
```
╔════════════════════════════════════════════╗
║  OpenAgent-Terminal Backend (Python)      ║
║  IPC Bridge Server                         ║
╚════════════════════════════════════════════╝

⚠️  Project Status: Phase 1 - Foundation
...
```

### 4. Copy Assets

```bash
cd ~/projects/openagent-terminal

# Create assets directory
mkdir -p assets

# Copy font from Portal
cp ../Portal/assets/DejaVuSansMono.ttf assets/

# Verify
ls -lh assets/
```

## Project Structure

```
openagent-terminal/
├── Cargo.toml              # Rust dependencies
├── src/                    # Rust source code
│   ├── main.rs            # Entry point
│   └── ipc/               # IPC client (Phase 1)
│       ├── mod.rs
│       ├── client.rs      # Unix socket client
│       ├── message.rs     # JSON-RPC messages
│       └── error.rs       # Error types
├── backend/               # Python backend
│   ├── setup.py          # Python package config
│   ├── openagent_terminal/
│   │   ├── __init__.py
│   │   └── bridge.py     # IPC server (Phase 1)
│   └── tests/
├── docs/                  # Documentation
│   ├── IPC_PROTOCOL.md   # Communication protocol
│   └── ...
├── assets/                # Fonts and resources
├── DESIGN.md             # Technical design
├── ROADMAP.md            # Implementation plan
└── README.md             # Project overview
```

## Current Phase: Foundation (Weeks 1-2)

### What's Working

✅ Project structure created  
✅ Cargo.toml configured  
✅ Basic Rust modules scaffolded  
✅ Python package structure created  
✅ Comprehensive documentation written  

### What's Next (Your Task!)

The following need to be implemented:

#### Rust Side
- [ ] Implement Unix socket client (src/ipc/client.rs)
- [ ] Add JSON-RPC message handling
- [ ] Write connection tests

#### Python Side
- [ ] Implement Unix socket server (backend/openagent_terminal/bridge.py)
- [ ] Add JSON-RPC request handler
- [ ] Connect to OpenAgent core

#### Testing
- [ ] Create integration test script
- [ ] Test initialize handshake
- [ ] Verify clean shutdown

## How to Contribute

### Phase 1 Contribution Opportunities

1. **Implement IPC Client (Rust)**
   - File: `src/ipc/client.rs`
   - Task: Implement Unix socket connection
   - Difficulty: Medium
   - See: `docs/IPC_PROTOCOL.md`

2. **Implement IPC Server (Python)**
   - File: `backend/openagent_terminal/bridge.py`
   - Task: Create async socket server
   - Difficulty: Medium
   - See: `docs/IPC_PROTOCOL.md`

3. **Write Tests**
   - Files: `tests/ipc_tests.rs`, `backend/tests/test_ipc.py`
   - Task: Integration tests for IPC
   - Difficulty: Easy-Medium

4. **Documentation**
   - Task: Add code examples, tutorials
   - Difficulty: Easy
   - Always needed!

### Development Workflow

1. **Pick a task** from ROADMAP.md Phase 1
2. **Create a branch**
   ```bash
   git checkout -b feature/ipc-client-implementation
   ```

3. **Implement the feature**
   - Write code
   - Add tests
   - Update documentation

4. **Test your changes**
   ```bash
   # Rust
   cargo test
   cargo clippy
   cargo fmt
   
   # Python
   pytest
   black .
   mypy .
   ```

5. **Commit and push**
   ```bash
   git add .
   git commit -m "feat: implement Unix socket client"
   git push origin feature/ipc-client-implementation
   ```

6. **Create Pull Request**
   - Describe what you implemented
   - Reference relevant issues
   - Link to protocol spec if applicable

## Useful Commands

### Rust Development

```bash
# Build
cargo build              # Debug build
cargo build --release   # Optimized build

# Run
cargo run               # Run with debug info
RUST_LOG=debug cargo run  # Verbose logging

# Test
cargo test              # Run all tests
cargo test ipc          # Run tests matching "ipc"

# Quality
cargo clippy            # Lint
cargo fmt               # Format code
```

### Python Development

```bash
# Activate venv
source backend/venv/bin/activate

# Install/update
pip install -e backend/

# Test
pytest backend/tests/
pytest -v               # Verbose
pytest --cov            # With coverage

# Quality
black backend/
mypy backend/
ruff check backend/
```

## Reading Material

Before diving into implementation, read these docs:

1. **[DESIGN.md](DESIGN.md)** - Understand the architecture
2. **[docs/IPC_PROTOCOL.md](docs/IPC_PROTOCOL.md)** - Learn the protocol
3. **[ROADMAP.md](ROADMAP.md)** - See the big picture

## Getting Help

- **Documentation:** Start with DESIGN.md
- **Issues:** Check existing GitHub issues
- **Discussions:** Use GitHub Discussions for questions
- **Code Examples:** Look at OpenAgent and Portal codebases

## Tips for Success

1. **Start Small:** Pick one small task to get familiar
2. **Read the Docs:** All protocols and designs are documented
3. **Ask Questions:** Use GitHub Discussions
4. **Test Early:** Write tests as you go
5. **Follow Conventions:** Match existing code style
6. **Commit Often:** Small, focused commits are better

## Next Steps

1. Read through the documentation
2. Build and run both frontend and backend
3. Pick a Phase 1 task from ROADMAP.md
4. Start coding!

## Need Help?

If you get stuck:
- Check the documentation in `docs/`
- Look at the TODO comments in source files
- Create a GitHub Discussion
- Open an issue with the `question` label

---

**Welcome aboard! Let's build something amazing together.** 🚀

Last Updated: 2025-10-04
