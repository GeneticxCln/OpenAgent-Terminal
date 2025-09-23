# OpenAgent Terminal Architecture

## Overview

OpenAgent Terminal is an AI-enhanced terminal emulator built on top of Alacritty's proven foundation. This document describes the high-level architecture and key design decisions.

## Core Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     OpenAgent Terminal                       │
├─────────────────────────────────────────────────────────────┤
│                         Frontend                             │
│  ┌──────────────┐  ┌─────────────┐  ┌──────────────┐       │
│  │   Renderer   │  │   Display   │  │   Window     │       │
│  │   (WGPU)     │  │   Manager   │  │   Manager    │       │
│  └──────────────┘  └─────────────┘  └──────────────┘       │
├─────────────────────────────────────────────────────────────┤
│                      Core Terminal                           │
│  ┌──────────────┐  ┌─────────────┐  ┌──────────────┐       │
│  │   Terminal   │  │     PTY     │  │    Shell     │       │
│  │   Emulator   │  │   Process   │  │  Interface   │       │
│  └──────────────┘  └─────────────┘  └──────────────┘       │
├─────────────────────────────────────────────────────────────┤
│                     AI Integration                           │
│  ┌──────────────┐  ┌─────────────┐  ┌──────────────┐       │
│  │ AI Providers │  │  AI Runtime │  │   AI Panel   │       │
│  │  (Ollama,    │  │   Manager   │  │     UI       │       │
│  │   OpenAI)    │  └─────────────┘  └──────────────┘       │
│  └──────────────┘                                           │
├─────────────────────────────────────────────────────────────┤
│                    Configuration                             │
│  ┌──────────────┐  ┌─────────────┐  ┌──────────────┐       │
│  │    Config    │  │   Settings  │  │   Keybinds   │       │
│  │    Parser    │  │   Manager   │  │   Manager    │       │
│  └──────────────┘  └─────────────┘  └──────────────┘       │
└─────────────────────────────────────────────────────────────┘
```

## Module Structure

### Core Modules

- **openagent-terminal**: Main application entry point and UI
- **openagent-terminal-core**: Terminal emulation logic (inherited from Alacritty)
- **openagent-terminal-config**: Configuration management
- **openagent-terminal-config-derive**: Derive macros for configuration
- **openagent-terminal-ai**: AI provider interfaces and implementations
- **openagent-terminal-sync**: Synchronization module (future)

### AI Module (`openagent-terminal-ai`)

The AI module follows a provider pattern for extensibility:

```rust
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn propose(&self, req: AiRequest) -> Result<Vec<AiProposal>, String>;
}
```

#### Providers

1. **Ollama Provider** (Local, Privacy-First)
   - Runs entirely on local machine
   - No data leaves the system
   - Default provider

2. **OpenAI Provider** (Cloud)
   - GPT-3.5/4 integration
   - Requires API key
   - Opt-in only

3. **Anthropic Provider** (Cloud)
   - Claude integration
   - Requires API key
   - Opt-in only

### Display Module

The display module handles rendering with multiple backends:

- **WGPU Backend**: Primary renderer using GPU acceleration
- OpenGL fallback has been removed.

#### AI Panel Rendering

The AI panel is rendered as an overlay:
- Triggered by keybinding (Ctrl+Shift+A)
- Renders in bottom third of terminal
- Semi-transparent background
- Keyboard navigation for proposals

## Data Flow

### AI Query Flow

```
User Input → Scratch Buffer → AI Runtime → Provider Selection
    ↓                                           ↓
Terminal UI ← Proposal Display ← Response ← AI Provider
```

### Command Execution Safety

**Important**: Commands are NEVER auto-executed
1. User requests command via natural language
2. AI generates proposals
3. User reviews proposals
4. User explicitly copies/executes chosen command

## Configuration

Configuration follows a hierarchical model:

1. Default configuration (built-in)
2. System configuration (`/etc/openagent-terminal/`)
3. User configuration (`~/.config/openagent-terminal/`)
4. Environment variables (for secrets)

### AI Configuration

```toml
[ai]
enabled = true
provider = "ollama"  # Default to local

[ai.ollama]
endpoint = "http://localhost:11434"
model = "codellama"

[ai.privacy]
strip_sensitive = true
never_auto_run = true  # Safety first!
```

## Security & Privacy

### Privacy Principles

1. **Local by Default**: Ollama runs entirely on user's machine
2. **Opt-in Cloud**: Cloud providers require explicit configuration
3. **No Telemetry**: No usage data collection
4. **Secure Secrets**: API keys only in environment variables

### Security Measures

- Commands never auto-execute
- Sensitive data stripping before API calls
- Audit logging for AI interactions
- Encrypted sync (when implemented)

## Performance Considerations

### Optimizations

1. **Async AI Calls**: Non-blocking UI during AI queries
2. **GPU Rendering**: Hardware acceleration for display
3. **Lazy Loading**: AI module only loaded when needed
4. **Efficient Updates**: Damage tracking for minimal redraws

### Benchmarks

Target metrics:
- Startup time: < 100ms
- AI response: < 500ms (local), < 2s (cloud)
- Rendering: 60fps minimum
- Memory: < 50MB base, < 150MB with AI

## Extension Points

### Adding New AI Providers

1. Implement the `AiProvider` trait
2. Add provider to factory function
3. Add configuration structure
4. Update documentation

Example:
```rust
pub struct CustomProvider {
    // provider fields
}

impl AiProvider for CustomProvider {
    fn name(&self) -> &'static str { "custom" }
    fn propose(&self, req: AiRequest) -> Result<Vec<AiProposal>, String> {
        // implementation
    }
}
```

### Plugin System (Future)

Planned plugin interface for:
- Custom AI providers
- Command preprocessors
- Output formatters
- Theme engines

## Testing Strategy

### Unit Tests
- Provider implementations
- Configuration parsing
- UI components

### Integration Tests
- End-to-end AI flow
- Provider switching
- Error handling

### Performance Tests
- Startup time
- Rendering performance
- AI response latency

## Future Enhancements

### Phase 4 Plans
- Streaming AI responses
- Multi-turn conversations
- Command explanation mode
- Custom prompt templates
- Fine-tuned models

### Long-term Vision
- Full plugin ecosystem
- Advanced sync capabilities
- Collaborative features
- Custom AI model training

## Dependencies

### Core Dependencies
- `winit`: Window management
- `wgpu`: GPU rendering (WGPU backend)
- `crossfont`: Font rendering

### AI Dependencies
- `reqwest`: HTTP client
- `tokio`: Async runtime
- `serde`: Serialization

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for development guidelines.

## References

- [Alacritty README](https://github.com/alacritty/alacritty/blob/master/README.md)
- [OpenAI API Documentation](https://platform.openai.com/docs)
- [Ollama Documentation](https://ollama.ai/docs)

---

*Last Updated: 2024-08-30*
*Version: 1.0.0*
