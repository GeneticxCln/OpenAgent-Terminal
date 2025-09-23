# openagent-terminal-ai

Optional AI interfaces for OpenAgent Terminal (privacy-first, opt-in).

Features:
- ai-ollama
- ai-openai
- ai-anthropic
- ai-openrouter
- agents (multi-agent system)

Testing:
- Examples are gated by features. Common patterns:
  - cargo test -p openagent-terminal-ai
  - cargo test -p openagent-terminal-ai --features ai-openai
  - cargo test -p openagent-terminal-ai --features ai-ollama

Docs:
- ../docs/TESTING.md
- ../docs/features.md

License:
- Apache-2.0 OR MIT
