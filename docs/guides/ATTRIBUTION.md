# Attribution

## OpenAgent Terminal

OpenAgent Terminal is an AI-enhanced terminal emulator that builds upon the excellent foundation provided by Alacritty.

## Based on Alacritty

This project is a fork of [Alacritty](https://github.com/alacritty/alacritty), a fast, cross-platform, OpenGL terminal emulator.

### Original Alacritty Authors
- Christian Duerr ([@chrisduerr](https://github.com/chrisduerr))
- Joe Wilm ([@jwilm](https://github.com/jwilm))
- And all [Alacritty contributors](https://github.com/alacritty/alacritty/graphs/contributors)

### Alacritty License
Alacritty is licensed under the Apache License, Version 2.0. The original license can be found at:
https://github.com/alacritty/alacritty/blob/master/LICENSE-APACHE

## OpenAgent Terminal Additions

OpenAgent Terminal adds the following features on top of Alacritty:

### AI Integration (New)
- **Multiple AI Providers**: Support for Ollama (local), OpenAI, and Anthropic
- **Smart Command Assistance**: Natural language to shell commands
- **Privacy-First Design**: Local AI by default, all cloud features opt-in
- **Context-Aware Suggestions**: Shell, directory, and platform awareness

### Additional Features (New)
- **Command Block Folding**: Collapse/expand command outputs
- **Sync System**: Optional settings/history synchronization (privacy-first)
- **Enhanced Configuration**: Extended TOML configuration for AI features
- **Workflow System**: Reusable command templates

## Third-Party Dependencies

### AI Module Dependencies
- `reqwest` - HTTP client for AI providers
- `tokio` - Async runtime for network operations
- `serde` - Serialization framework

### Original Alacritty Dependencies
- `winit` - Window handling
- `glutin` - OpenGL context creation
- `crossfont` - Font rasterization
- And many others listed in Cargo.toml

## Acknowledgments

We are deeply grateful to:

1. **The Alacritty Team**: For creating an exceptional terminal emulator that serves as our foundation
2. **The Rust Community**: For the excellent ecosystem and tools
3. **AI Provider Communities**:
   - Ollama team for local AI infrastructure
   - OpenAI for GPT models
   - Anthropic for Claude models

## Contributing

When contributing to OpenAgent Terminal:
- Respect the original Alacritty architecture where possible
- Maintain backward compatibility with Alacritty configurations
- Follow the established code style and conventions
- Credit original authors when modifying existing code

## License

OpenAgent Terminal maintains the Apache License, Version 2.0, as required by the original Alacritty project.

---

*This file serves to acknowledge the significant contribution of Alacritty to this project and to clarify the relationship between OpenAgent Terminal and Alacritty.*
