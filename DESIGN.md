# OpenAgent-Terminal: Technical Design Document

**Version:** 0.1.0  
**Date:** 2025-10-04  
**Status:** Draft

## Executive Summary

OpenAgent-Terminal is an AI-native terminal emulator that combines the GPU-accelerated rendering capabilities of Portal (fredg-wgpu-terminal) with the intelligent agent framework of OpenAgent. This document outlines the technical architecture, integration strategy, and implementation roadmap.

## Vision

Create the **first AI-native terminal emulator** that:
- Provides seamless AI agent interaction within the terminal workflow
- Offers GPU-accelerated rendering of AI outputs with rich formatting
- Visualizes tool executions and agent reasoning in real-time
- Maintains the performance and responsiveness expected of modern terminals

## Architecture Overview

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    OpenAgent-Terminal                        │
│                                                               │
│  ┌────────────────────┐         ┌──────────────────────┐   │
│  │   Rust Frontend    │◄───────►│  Python Backend      │   │
│  │   (Portal-based)   │  IPC    │  (OpenAgent Core)    │   │
│  │                    │         │                       │   │
│  │ • WGPU Renderer    │         │ • Agent System       │   │
│  │ • PTY Manager      │         │ • LLM Integration    │   │
│  │ • Input Handler    │         │ • Tool Execution     │   │
│  │ • Block Renderer   │         │ • Policy Engine      │   │
│  │ • Agent Pane       │         │ • Streaming API      │   │
│  └────────────────────┘         └──────────────────────┘   │
│           │                              │                   │
│           └──────────────────────────────┘                   │
│                   JSON-RPC over IPC                          │
└─────────────────────────────────────────────────────────────┘
```

### Component Breakdown

#### 1. Rust Frontend (Terminal Core)
**Base:** Portal (fredg-wgpu-terminal)

**New Components:**
- `agent_pane.rs` - Dedicated pane for AI agent interaction
- `block_renderer.rs` - GPU-accelerated block rendering with syntax highlighting
- `ipc_client.rs` - JSON-RPC client for Python backend communication
- `stream_buffer.rs` - Efficient buffer for streaming LLM tokens
- `split_manager.rs` - Layout manager for multi-pane views

**Responsibilities:**
- All rendering (shell + agent UI)
- Input capture and routing
- PTY management for shell
- Real-time display of streaming AI responses
- Session state persistence

#### 2. Python Backend (Intelligence Core)
**Base:** OpenAgent

**New Components:**
- `terminal_bridge.py` - IPC server for terminal frontend
- `terminal_backend.py` - Terminal-specific backend interface
- `block_formatter.py` - Format agent outputs for terminal blocks
- `stream_adapter.py` - Adapt LLM streaming for terminal consumption

**Responsibilities:**
- Agent orchestration
- LLM inference
- Tool execution
- Context management
- Security policy enforcement

## Inter-Process Communication (IPC)

### Protocol: JSON-RPC 2.0 over Unix Domain Socket

**Rationale:**
- Language-agnostic
- Well-defined specification
- Efficient for local communication
- Easy to debug and extend

### IPC Architecture

```
Rust Frontend                          Python Backend
     │                                      │
     │  1. Connect to Unix Socket           │
     │─────────────────────────────────────►│
     │                                      │
     │  2. Initialize Request               │
     │  {"method": "initialize"}            │
     │─────────────────────────────────────►│
     │                                      │
     │  3. Ready Response                   │
     │  {"result": {"status": "ready"}}     │
     │◄─────────────────────────────────────│
     │                                      │
     │  4. User types message               │
     │  {"method": "agent.query"}           │
     │─────────────────────────────────────►│
     │                                      │
     │  5. Stream tokens back               │
     │  {"method": "stream.token"}          │
     │◄─────────────────────────────────────│
     │  (multiple notifications)            │
     │◄─────────────────────────────────────│
     │                                      │
     │  6. Complete response                │
     │  {"method": "stream.complete"}       │
     │◄─────────────────────────────────────│
```

### Socket Location
- **Path:** `$XDG_RUNTIME_DIR/openagent-terminal-{pid}.sock`
- **Permissions:** 0600 (user-only access)
- **Cleanup:** Automatic on process exit

### Message Types

#### Client → Server (Rust → Python)

1. **initialize**
   ```json
   {
     "jsonrpc": "2.0",
     "id": 1,
     "method": "initialize",
     "params": {
       "terminal_size": {"cols": 80, "rows": 24},
       "capabilities": ["streaming", "blocks", "syntax_highlighting"]
     }
   }
   ```

2. **agent.query**
   ```json
   {
     "jsonrpc": "2.0",
     "id": 2,
     "method": "agent.query",
     "params": {
       "message": "How do I optimize this Rust code?",
       "context": {
         "cwd": "/home/user/project",
         "shell_state": "bash",
         "recent_commands": ["cargo build", "cargo test"]
       }
     }
   }
   ```

3. **agent.cancel**
   ```json
   {
     "jsonrpc": "2.0",
     "id": 3,
     "method": "agent.cancel",
     "params": {"query_id": 2}
   }
   ```

4. **tool.approve**
   ```json
   {
     "jsonrpc": "2.0",
     "id": 4,
     "method": "tool.approve",
     "params": {
       "tool_execution_id": "exec-123",
       "approved": true
     }
   }
   ```

#### Server → Client (Python → Rust)

1. **stream.token** (notification)
   ```json
   {
     "jsonrpc": "2.0",
     "method": "stream.token",
     "params": {
       "query_id": 2,
       "token": "To optimize",
       "metadata": {"type": "text"}
     }
   }
   ```

2. **stream.block** (notification)
   ```json
   {
     "jsonrpc": "2.0",
     "method": "stream.block",
     "params": {
       "query_id": 2,
       "block": {
         "type": "code",
         "language": "rust",
         "content": "fn optimized() -> Result<()> { ... }",
         "metadata": {"diff": true}
       }
     }
   }
   ```

3. **stream.complete** (notification)
   ```json
   {
     "jsonrpc": "2.0",
     "method": "stream.complete",
     "params": {
       "query_id": 2,
       "status": "success",
       "metadata": {
         "tokens_used": 150,
         "tools_executed": ["file_read", "code_analyze"]
       }
     }
   }
   ```

4. **tool.request_approval** (notification)
   ```json
   {
     "jsonrpc": "2.0",
     "method": "tool.request_approval",
     "params": {
       "execution_id": "exec-123",
       "tool_name": "file_write",
       "description": "Write optimized code to src/main.rs",
       "preview": "--- a/src/main.rs\n+++ b/src/main.rs\n...",
       "risk_level": "medium"
     }
   }
   ```

## UI Layout & Rendering

### Layout Modes

#### 1. Classic Mode (Default)
```
┌─────────────────────────────────────────────┐
│  Shell Output                               │
│  $ cargo build                              │
│     Compiling...                            │
│                                             │
│  [AI Assistant available - Press Ctrl+A]    │
└─────────────────────────────────────────────┘
```

#### 2. Split Mode (AI Active)
```
┌──────────────────────┬──────────────────────┐
│  Shell (60%)         │  AI Assistant (40%)  │
│                      │                      │
│  $ cargo test        │  > Analyzing test... │
│    test foo ... ok   │                      │
│                      │  I can see your test │
│                      │  is failing because: │
│                      │                      │
│                      │  ```rust             │
│                      │  fn fixed() { ... }  │
│                      │  ```                 │
└──────────────────────┴──────────────────────┘
```

#### 3. Overlay Mode (Quick Query)
```
┌─────────────────────────────────────────────┐
│  Shell Output                               │
│  $ ./run_server                             │
│  Server listening on :8080                  │
│  ┌────────────────────────────────────────┐ │
│  │ 🤖 AI Query                            │ │
│  │ > explain this error                   │ │
│  │                                        │ │
│  │ This error occurs because...           │ │
│  │ [Esc to close]                         │ │
│  └────────────────────────────────────────┘ │
└─────────────────────────────────────────────┘
```

### Block Rendering System

#### Block Types

1. **Text Block** - Standard text output
2. **Code Block** - Syntax-highlighted code with language detection
3. **Diff Block** - Color-coded diffs with +/- indicators
4. **Tool Block** - Tool execution status and output
5. **Error Block** - Error messages with stack traces
6. **Interactive Block** - Blocks requiring user input (approvals)

#### Block Features

- **Folding:** Collapse/expand large blocks
- **Syntax Highlighting:** Language-aware with themes
- **Search:** Highlight search matches across blocks
- **Copy:** Easy copying of block contents
- **Export:** Save blocks to files
- **Links:** Clickable file paths and URLs

### WGPU Rendering Pipeline

```
User Input → Event Loop
                ↓
    ┌───────────────────────┐
    │  Layout Manager       │
    │  • Calculate panes    │
    │  • Manage focus       │
    └───────────────────────┘
                ↓
    ┌───────────────────────┐
    │  Block Renderer       │
    │  • Format blocks      │
    │  • Syntax highlight   │
    │  • Apply themes       │
    └───────────────────────┘
                ↓
    ┌───────────────────────┐
    │  WGPU Compositor      │
    │  • Generate glyphs    │
    │  • Apply shaders      │
    │  • Render to texture  │
    └───────────────────────┘
                ↓
         Window Surface
```

## Integration Strategy

### Phase 1: Foundation (Weeks 1-2)

**Goal:** Get basic IPC communication working

**Tasks:**
1. Set up project structure
2. Implement JSON-RPC over Unix socket in Rust
3. Create Python IPC server in OpenAgent
4. Test basic message passing
5. Implement initialization handshake

**Deliverables:**
- Rust can send messages to Python
- Python can send responses back
- Simple echo test working

### Phase 2: Core Integration (Weeks 3-4)

**Goal:** Basic agent query/response cycle

**Tasks:**
1. Implement `agent.query` in Rust
2. Connect to OpenAgent's chat API
3. Add streaming token support
4. Display AI responses in terminal
5. Handle errors and timeouts

**Deliverables:**
- User can ask AI questions
- Responses stream back in real-time
- Basic text formatting works

### Phase 3: Block Rendering (Weeks 5-6)

**Goal:** Rich block-based UI

**Tasks:**
1. Port OpenAgent block system to Rust
2. Implement WGPU block renderer
3. Add syntax highlighting
4. Implement folding/unfolding
5. Add copy/export functionality

**Deliverables:**
- Code blocks render with syntax highlighting
- Diffs show with colors
- Users can fold/unfold blocks
- Export blocks to files

### Phase 4: Tool Integration (Weeks 7-8)

**Goal:** Visualize tool executions

**Tasks:**
1. Add tool approval UI
2. Show tool execution progress
3. Display tool outputs inline
4. Implement safety previews
5. Add rollback capability

**Deliverables:**
- Tool requests show approval dialog
- Tool execution visualized
- Safe/unsafe operations clearly marked
- Rollback works for file operations

### Phase 5: Advanced Features (Weeks 9-12)

**Goal:** Polish and unique features

**Tasks:**
1. Multi-pane layout system
2. Session persistence
3. Command explanation mode
4. Inline suggestions
5. Performance optimization
6. Configuration system
7. Plugin support
8. Documentation

**Deliverables:**
- Split pane views work smoothly
- Sessions save/restore
- Hover explanations
- Command completions from AI
- Comprehensive docs

## Technology Stack

### Rust (Frontend)
- **GUI:** winit + wgpu (from Portal)
- **Text Rendering:** wgpu_glyph
- **IPC:** Custom JSON-RPC implementation
- **Async:** tokio
- **Serialization:** serde_json
- **PTY:** portable-pty
- **Parser:** vte

### Python (Backend)
- **Framework:** OpenAgent core
- **IPC Server:** asyncio + Unix sockets
- **JSON-RPC:** jsonrpcserver library
- **LLM:** Transformers / Ollama
- **Async:** asyncio

### Communication
- **Protocol:** JSON-RPC 2.0
- **Transport:** Unix Domain Sockets
- **Serialization:** JSON

## Performance Considerations

### Latency Targets
- **Input to response:** < 100ms (IPC overhead)
- **Token streaming:** < 50ms per token
- **Rendering:** 60 FPS minimum
- **Memory usage:** < 200MB baseline (excluding LLM)

### Optimization Strategies

1. **Double Buffering:** Keep shell and agent panes independent
2. **Incremental Rendering:** Only redraw changed regions
3. **GPU Acceleration:** Use WGPU for all text rendering
4. **Efficient IPC:** Batch notifications when possible
5. **Smart Throttling:** Limit token stream rate to rendering capacity
6. **Lazy Loading:** Load syntax highlighters on-demand
7. **Memory Pooling:** Reuse buffers for streaming data

## Security Considerations

### IPC Security
- Unix socket with 0600 permissions
- Process isolation (Rust can't execute Python code directly)
- Message validation on both sides
- Rate limiting on IPC messages

### Agent Security
- All tool executions go through approval (unless explicitly trusted)
- Sandbox tool execution when possible
- Command validation via OpenAgent's policy engine
- File operation restrictions (safe paths only)

### Resource Limits
- Max message size: 10MB
- Max concurrent queries: 3
- Token rate limit: 1000/second
- Memory limit: 4GB for Python process

## Configuration

### Terminal Config (`~/.config/openagent-terminal/config.toml`)

```toml
[terminal]
font_family = "DejaVu Sans Mono"
font_size = 14
theme = "monokai"
scrollback_lines = 10000

[agent]
model = "codellama-7b"
auto_suggest = true
require_approval = true  # For tool executions

[layout]
default_mode = "classic"  # classic, split, overlay
split_ratio = 0.6  # Shell takes 60%, agent 40%
ai_pane_position = "right"  # right, bottom, left

[keybindings]
toggle_ai = "Ctrl+A"
send_query = "Ctrl+Enter"
cancel_query = "Ctrl+C"
approve_tool = "Ctrl+Y"
reject_tool = "Ctrl+N"

[ipc]
socket_path = "$XDG_RUNTIME_DIR/openagent-terminal-{pid}.sock"
timeout = 30  # seconds
```

### OpenAgent Config (`.openagent.toml`)

Standard OpenAgent configuration, plus:

```toml
[terminal_backend]
enabled = true
socket_path = "$XDG_RUNTIME_DIR/openagent-terminal-{pid}.sock"
max_concurrent_queries = 3
streaming = true

[terminal_features]
inline_suggestions = true
command_explanation = true
tool_preview = true
```

## Testing Strategy

### Unit Tests
- Rust: IPC client, block parser, layout manager
- Python: IPC server, message handlers, block formatter

### Integration Tests
- Full IPC cycle (Rust → Python → Rust)
- Agent query/response with streaming
- Tool approval flow
- Session save/restore

### End-to-End Tests
- Spawn full terminal + agent backend
- Send queries, verify rendering
- Test all keybindings
- Verify cleanup on exit

### Performance Tests
- IPC latency benchmarks
- Rendering FPS under load
- Memory usage profiling
- Token streaming throughput

## Development Workflow

### Setup
```bash
# Clone both projects
git clone <openagent-terminal>
cd openagent-terminal

# Initialize submodules (if using git submodules)
git submodule update --init

# Build Rust frontend
cargo build --release

# Install Python backend
cd backend
pip install -e .
```

### Run Development Build
```bash
# Terminal 1: Start OpenAgent backend in debug mode
cd backend
python -m openagent_terminal.bridge --debug

# Terminal 2: Run Rust frontend
cargo run
```

### Run Tests
```bash
# Rust tests
cargo test

# Python tests
cd backend && pytest

# Integration tests
./scripts/test_integration.sh
```

## Deployment

### Binary Distribution
1. **Rust frontend:** Static binary via `cargo build --release`
2. **Python backend:** PyInstaller bundle or system Python
3. **Combined package:** AppImage (Linux) or DMG (macOS)

### Package Options

**Option 1: Separate packages**
- `openagent-terminal` (Rust binary)
- `openagent-terminal-backend` (Python package)
- User installs both

**Option 2: Bundled (Recommended)**
- Single AppImage/DMG with both components
- Python runtime embedded
- Models downloaded on first run

## Future Enhancements

### Short Term
- [ ] Inline command suggestions
- [ ] Multi-agent support
- [ ] Custom themes
- [ ] Plugin system

### Medium Term
- [ ] Remote agent connections
- [ ] Collaborative features
- [ ] Agent marketplace
- [ ] Mobile companion app

### Long Term
- [ ] Voice input/output
- [ ] Visual programming interface
- [ ] Agent training from terminal history
- [ ] Cloud sync for sessions

## Success Metrics

### Technical Metrics
- Render at 60 FPS consistently
- IPC latency < 50ms p95
- Memory usage < 500MB with agent loaded
- Startup time < 2 seconds

### User Experience Metrics
- Agent response feels "instant" for simple queries
- No perceptible lag in terminal input
- Smooth streaming of AI responses
- Zero crashes in normal operation

### Adoption Metrics
- 1000+ stars on GitHub in 6 months
- Active community contributions
- Featured in terminal emulator comparisons
- Used in production by early adopters

## Conclusion

OpenAgent-Terminal represents a new category of developer tools: AI-native terminals that make working with intelligent agents as natural as using a shell. By combining Portal's GPU-accelerated rendering with OpenAgent's powerful agent framework, we create a unique and valuable tool that advances the state of the art in terminal emulators.

The phased approach ensures we build incrementally, validating each component before moving forward. The IPC architecture keeps concerns separated while maintaining high performance. The rich UI provides the visualization needed to understand AI agent behavior.

This is an ambitious project, but the component pieces are solid, the architecture is sound, and the market need is clear. Let's build the future of terminals together.

---

**Document Status:** Living document - will be updated as implementation progresses  
**Next Review:** After Phase 1 completion  
**Contributors:** Please submit feedback via GitHub issues
