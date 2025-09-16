#009 — AI streaming reliability (Retry-After + backpressure)

Status: Open
Priority: High

Scope
- Ensure Retry-After and rate-limit reset headers are honored for all cloud providers
- Confirm micro-batching/backpressure behavior across providers
- Improve streaming UX and logging state transitions

References
- openagent-terminal-ai/src/streaming.rs
- openagent-terminal-ai/src/providers/*
- openagent-terminal/tests/ai_stream_mid_cancel.rs
- .github/workflows/ci.yml (ai-tests)

Acceptance criteria
- Verified provider behavior in tests (OpenAI/Anthropic/OpenRouter)
- Streaming UX shows clear states; logs reflect chunking/backpressure
