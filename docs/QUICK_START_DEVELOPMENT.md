# Quick Start Development Guide

## Immediate Fixes (Day 1)

The previous instructions for manually changing Rust edition/toolchain with sed are obsolete and conflict with the current workspace configuration.

Single source of truth (do not override per-crate):
- rust-toolchain.toml → channel = 1.79.0 (with clippy, rustfmt)
- workspace Cargo.toml → [workspace.package] edition = "2021", rust-version = "1.79.0"

Notes:
- rustup/cargo automatically pick up rust-toolchain.toml; no manual edits are required.
- Member crates inherit edition/rust-version via `edition.workspace = true` and `rust-version.workspace = true`.

Verify your environment:
```bash
rustc --version   # should be 1.79.0
cargo --version   # should use the 1.79.0 toolchain
```

If rustup failed to select the toolchain, ensure it is installed:
```bash
rustup toolchain install 1.79.0
```
(Advanced) You can force it locally if necessary:
```bash
rustup override set 1.79.0
```

### 2. Fix Compiler Warnings

Create `fix_warnings.sh`:
```bash
#!/bin/bash

# Fix unused imports
cargo fix --allow-dirty --allow-staged

# Fix irrefutable let patterns
# Manual fixes needed in openagent-terminal/src/display/mod.rs
```

Key files to fix:
- `openagent-terminal/src/display/mod.rs` (lines 725, 758, 793)
- `openagent-terminal/src/renderer/mod.rs` (line 34)

### 3. Create Initial AI Implementation

Create `openagent-terminal-ai/src/providers/ollama.rs`:
```rust
use crate::{AiProvider, AiProposal, AiRequest};
use serde::{Deserialize, Serialize};

pub struct OllamaProvider {
    endpoint: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(endpoint: String, model: String) -> Self {
        Self { endpoint, model }
    }
    
    pub fn from_env() -> Result<Self, String> {
        let endpoint = std::env::var("OLLAMA_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let model = std::env::var("OLLAMA_MODEL")
            .unwrap_or_else(|_| "codellama".to_string());
        Ok(Self { endpoint, model })
    }
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

impl AiProvider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }
    
    fn propose(&self, req: AiRequest) -> Result<Vec<AiProposal>, String> {
        // For now, return a mock response
        // TODO: Implement actual HTTP client
        let proposal = AiProposal {
            title: format!("Suggestion for: {}", req.scratch_text),
            description: Some("AI-generated command".to_string()),
            proposed_commands: vec![
                "echo 'This is a placeholder command'".to_string()
            ],
        };
        
        Ok(vec![proposal])
    }
}
```

### 4. Update AI Module lib.rs

```rust
// openagent-terminal-ai/src/lib.rs
// Add at the end of the file:

#[cfg(feature = "ollama")]
pub mod providers {
    pub mod ollama;
    pub use ollama::OllamaProvider;
}

// Factory function for creating providers
pub fn create_provider(name: &str) -> Result<Box<dyn AiProvider>, String> {
    match name {
        "null" => Ok(Box::new(NullProvider)),
        #[cfg(feature = "ollama")]
        "ollama" => Ok(Box::new(providers::OllamaProvider::from_env()?)),
        _ => Err(format!("Unknown provider: {}", name)),
    }
}
```

### 5. Add HTTP Client Support

Update `openagent-terminal-ai/Cargo.toml`:
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.11", features = ["json", "blocking"], optional = true }
tokio = { version = "1", features = ["rt", "macros"], optional = true }

[features]
default = []
ollama = ["reqwest", "tokio"]
```

---

## Week 1 Implementation Plan

### Day 1-2: Foundation Fixes
- [ ] Confirm toolchain (1.79.0) and edition (2021) via rust-toolchain.toml and workspace Cargo.toml
- [ ] Resolve compiler warnings
- [ ] Update README.md with correct information
- [ ] Create ATTRIBUTION.md

### Day 3-4: Basic AI Implementation
- [ ] Implement Ollama provider
- [ ] Add command parsing logic
- [ ] Create simple context manager
- [ ] Add configuration support

### Day 5-6: Integration with Terminal
- [ ] Add AI keybindings
- [ ] Create scratch buffer UI
- [ ] Implement suggestion display
- [ ] Add copy-to-clipboard for suggestions

### Day 7: Testing & Documentation
- [ ] Write unit tests for AI module
- [ ] Create integration test
- [ ] Document AI API
- [ ] Create user guide

---

## Development Commands

### Build with AI features:
```bash
cargo build --features "ai ollama"
```

### Run tests:
```bash
cargo test --all-features
```

### Check code quality:
```bash
cargo clippy --all-features -- -D warnings
cargo fmt --check
```

### Run with debug logging:
```bash
RUST_LOG=openagent_terminal=debug cargo run --features "ai"
```

---

## Testing the AI Integration

### 1. Install Ollama locally:
```bash
# Linux/Mac
curl -fsSL https://ollama.ai/install.sh | sh

# Pull a model
ollama pull codellama
```

### 2. Test configuration:
Create `~/.config/openagent-terminal/openagent-terminal.toml`:
```toml
[ai]
enabled = true
provider = "ollama"

[ai.ollama]
endpoint = "http://localhost:11434"
model = "codellama"
```

### 3. Test AI functionality:
```bash
# Run the terminal
cargo run --features "ai ollama"

# In the terminal, trigger AI (e.g., Ctrl+Shift+A)
# Type a command request in natural language
# View suggestions
```

---

## Code Organization Best Practices

### Module Structure:
```
openagent-terminal-ai/
├── src/
│   ├── lib.rs           # Public API
│   ├── providers/       # AI provider implementations
│   │   ├── mod.rs
│   │   ├── ollama.rs
│   │   └── openai.rs
│   ├── context/         # Context management
│   │   ├── mod.rs
│   │   └── manager.rs
│   └── security/        # Privacy/security features
│       ├── mod.rs
│       └── sanitizer.rs
```

### Error Handling Pattern:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AiError {
    #[error("Provider error: {0}")]
    Provider(String),
    
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("Configuration error: {0}")]
    Config(String),
}

pub type AiResult<T> = Result<T, AiError>;
```

---

## Git Workflow

### Create feature branch:
```bash
git checkout -b feature/ai-implementation
```

### Commit message format:
```
feat(ai): implement Ollama provider

- Add basic Ollama HTTP client
- Implement AiProvider trait
- Add configuration support
- Include unit tests

Closes #123
```

### Pre-push checklist:
- [ ] All tests pass
- [ ] No compiler warnings
- [ ] Code formatted with rustfmt
- [ ] Documentation updated
- [ ] CHANGELOG.md updated

---

## Debugging Tips

### Enable verbose logging:
```bash
RUST_LOG=trace cargo run 2>&1 | tee debug.log
```

### Use conditional compilation for debug features:
```rust
#[cfg(debug_assertions)]
eprintln!("Debug: AI request: {:?}", request);
```

### Performance profiling:
```bash
cargo build --release --features "ai"
perf record --call-graph=dwarf ./target/release/openagent-terminal
perf report
```

---

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

---

## Resources

- [Rust Async Book](https://rust-lang.github.io/async-book/)
- [Ollama API Docs](https://github.com/ollama/ollama/blob/main/docs/api.md)
- [Terminal Emulator Specs](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
- [Alacritty Architecture](https://github.com/alacritty/alacritty/blob/master/docs/architecture.md)

---

*For questions or help, open an issue on GitHub or join our Discord.*
