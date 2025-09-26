# AI Terminal Integration Demo — Phase 1 Completion

This demo now integrates real AI providers, a full security risk analyzer, and a robust AI error analysis pipeline.

Features
- Providers: OpenAI, Anthropic, Ollama, OpenRouter
- Security: Command risk analysis with RiskLevel (LOW/MEDIUM/HIGH/CRITICAL), findings and suggestions
- AI Analysis: Context-rich prompts for both failures and successes
- Real FS checks: Detect project types by actual files (.git, package.json, Cargo.toml)

Environment variables
- OPENAI_API_KEY — for OpenAI
- OPENAI_MODEL (default: gpt-4o-mini)
- ANTHROPIC_API_KEY — for Anthropic
- ANTHROPIC_MODEL (default: claude-3-5-sonnet-latest)
- OLLAMA_BASE_URL (default: http://localhost:11434)
- OLLAMA_MODEL (default: llama3.1:8b-instruct)
- OPENROUTER_API_KEY — for OpenRouter
- OPENROUTER_BASE_URL (default: https://openrouter.ai)
- OPENROUTER_REFERER (optional, recommended)
- OPENROUTER_APP (optional, sent as X-Title)
- OPENROUTER_MODEL (default: openrouter/auto)

Provider selection
At startup, the demo selects a default provider in this priority order:
1) OpenRouter, 2) OpenAI, 3) Anthropic, 4) Ollama

Build and run
- Install Rust toolchain (stable)
- Build: cargo build --release
- Run: cargo run --release

Notes
- If no cloud API keys are set, it will use Ollama locally (ensure Ollama is running and the model is pulled).
- Outputs include security warnings when risky commands are detected.
