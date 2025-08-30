# Quick Start Development Guide

## Immediate Fixes (Day 1)

### 1. Fix Cargo.toml Version Issues

```bash
# Run this script to fix all Cargo.toml files
#!/bin/bash

# Fix workspace Cargo.toml
sed -i 's/edition = "2024"/edition = "2021"/' Cargo.toml
sed -i 's/rust-version = "1.85.0"/rust-version = "1.74.0"/' Cargo.toml

# Fix all member Cargo.toml files
find . -name "Cargo.toml" -exec sed -i 's/edition.workspace = true/edition = "2021"/' {} \;

# Verify changes
cargo check
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
- [ ] Fix Cargo.toml versions
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

## Resources

- [Rust Async Book](https://rust-lang.github.io/async-book/)
- [Ollama API Docs](https://github.com/ollama/ollama/blob/main/docs/api.md)
- [Terminal Emulator Specs](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
- [Alacritty Architecture](https://github.com/alacritty/alacritty/blob/master/docs/architecture.md)

---

*For questions or help, open an issue on GitHub or join our Discord.*
