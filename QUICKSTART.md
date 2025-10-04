# OpenAgent-Terminal Quick Start Guide

## ⚡ Get Started in 5 Minutes

### Prerequisites
- Rust (latest stable)
- Python 3.8+
- Linux or macOS

### 1. Clone & Setup

```bash
cd ~/projects/openagent-terminal
```

### 2. Install Python Dependencies

```bash
cd backend
pip install -e .
cd ..
```

### 3. Run Phase 1 Test

```bash
./test_ipc.sh
```

That's it! If you see "✅ Phase 1 IPC Test PASSED!" you're ready to go.

---

## 🔨 Development Workflow

### Running the Backend Only

```bash
cd backend
python -m openagent_terminal.bridge

# Or with debug logging:
python -m openagent_terminal.bridge --debug

# Or with custom socket:
python -m openagent_terminal.bridge --socket /tmp/my-socket.sock
```

### Running the Frontend Only

```bash
# Make sure backend is running first!
cargo run

# Or with custom socket:
export OPENAGENT_SOCKET=/tmp/my-socket.sock
cargo run
```

### Building Release Version

```bash
cargo build --release
./target/release/openagent-terminal
```

### Running Tests

```bash
# Rust tests
cargo test

# Python tests (when added)
cd backend && pytest

# Integration test
./test_ipc.sh
```

---

## 📁 Project Structure

```
openagent-terminal/
├── src/                      # Rust source code
│   ├── main.rs              # Entry point
│   └── ipc/                 # IPC client module
│       ├── mod.rs
│       ├── client.rs        # Unix socket client
│       ├── message.rs       # JSON-RPC messages
│       └── error.rs         # Error types
├── backend/                  # Python backend
│   ├── openagent_terminal/
│   │   ├── __init__.py
│   │   └── bridge.py        # IPC server
│   └── setup.py
├── docs/                     # Documentation
│   └── IPC_PROTOCOL.md      # Protocol specification
├── Cargo.toml               # Rust dependencies
├── DESIGN.md                # Architecture doc
├── ROADMAP.md               # Implementation plan
└── test_ipc.sh              # Integration test
```

---

## 🐛 Troubleshooting

### "Connection failed" error

**Problem:** Rust can't connect to Python backend.

**Solution:**
1. Make sure the backend is running:
   ```bash
   cd backend && python -m openagent_terminal.bridge
   ```
2. Check if socket exists:
   ```bash
   ls -la /run/user/1000/openagent-terminal-test.sock
   # or
   ls -la /tmp/openagent-terminal-test.sock
   ```

### Socket permission denied

**Problem:** Can't access socket file.

**Solution:**
```bash
# Socket should have 600 permissions
chmod 600 /path/to/socket.sock
```

### Port already in use / Socket already exists

**Problem:** Old socket file exists.

**Solution:**
```bash
rm /run/user/1000/openagent-terminal-test.sock
# or
rm /tmp/openagent-terminal-test.sock
```

### Build errors

**Problem:** Cargo build fails.

**Solution:**
```bash
# Update Rust
rustup update

# Clean and rebuild
cargo clean
cargo build
```

---

## 🎯 Current Status

✅ **Phase 1 Complete** - IPC Foundation  
🔴 **Phase 2 Next** - Agent Integration  

### What Works Now
- ✅ Unix socket IPC between Rust and Python
- ✅ JSON-RPC 2.0 message protocol
- ✅ Initialize handshake
- ✅ Connection lifecycle management
- ✅ Integration testing

### What's Next (Phase 2)
- 🔴 Agent query/response
- 🔴 Token streaming
- 🔴 Terminal UI rendering
- 🔴 OpenAgent integration

---

## 📚 Key Documentation

1. **[DESIGN.md](DESIGN.md)** - Full technical architecture
2. **[ROADMAP.md](ROADMAP.md)** - Implementation phases (12 weeks)
3. **[docs/IPC_PROTOCOL.md](docs/IPC_PROTOCOL.md)** - Protocol specification
4. **[PHASE1_COMPLETE.md](PHASE1_COMPLETE.md)** - Phase 1 results
5. **[README.md](README.md)** - Project overview

---

## 🤝 Contributing

### Adding New Features

1. **Check the roadmap:**
   ```bash
   cat ROADMAP.md
   ```

2. **Follow the architecture:**
   - Rust: Terminal UI, IPC client, rendering
   - Python: Agent logic, OpenAgent integration, IPC server

3. **Write tests:**
   - Unit tests in Rust: `#[cfg(test)]` modules
   - Unit tests in Python: `tests/` directory
   - Integration tests: Update `test_ipc.sh`

### Code Style

**Rust:**
```bash
cargo fmt        # Format code
cargo clippy     # Run linter
```

**Python:**
```bash
black .          # Format code
ruff check .     # Run linter
```

---

## 💡 Tips & Tricks

### Enable Debug Logging

**Rust:**
```bash
RUST_LOG=debug cargo run
```

**Python:**
```bash
python -m openagent_terminal.bridge --debug
```

### Custom Socket Path

```bash
export OPENAGENT_SOCKET=/tmp/my-custom-socket.sock
python -m openagent_terminal.bridge --socket /tmp/my-custom-socket.sock &
cargo run
```

### Watch Mode (Auto-rebuild)

```bash
# Install cargo-watch
cargo install cargo-watch

# Auto-rebuild on changes
cargo watch -x run
```

---

## 🆘 Getting Help

1. **Check logs:** Both Rust and Python emit detailed logs
2. **Read error messages:** They're designed to be helpful
3. **Review docs:** Especially `docs/IPC_PROTOCOL.md`
4. **Check GitHub issues:** See if others had similar problems

---

## ✨ Quick Commands Cheatsheet

```bash
# Full integration test
./test_ipc.sh

# Start backend
cd backend && python -m openagent_terminal.bridge

# Start frontend (in new terminal)
cargo run

# Build release
cargo build --release

# Run tests
cargo test                    # Rust tests
cd backend && pytest          # Python tests

# Format code
cargo fmt                     # Rust
black backend/                # Python

# Check for issues
cargo clippy                  # Rust linter
```

---

**Last Updated:** 2025-10-04  
**Current Phase:** ✅ Phase 1 Complete - Ready for Phase 2

🚀 **Happy coding!**
