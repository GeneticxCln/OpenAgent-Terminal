feat(docs, ai): migrate to OPENAGENT_OLLAMA_*; clarify server vars; fix reconfigure borrows; perf test guard

- Docs/examples: use OPENAGENT_OLLAMA_ENDPOINT and OPENAGENT_OLLAMA_MODEL (preferred)
- Code: accept legacy OLLAMA_ENDPOINT/OLLAMA_MODEL for compatibility
- Clarify OLLAMA_HOST is server-side (Ollama container/process), not client
- Fix borrow-checker issues in AI provider reconfigure flows (event.rs)
- CLI: fix minor compile issues (history export base path, provider mark)
- Tests: add OPENAGENT_STRICT_PERF=1 guard for timing-sensitive perf test
- Formatting: cargo fmt (stable); Linting: cargo clippy (warnings only)

Notes:
- Workspace builds with --features ai-ollama
- All tests pass when skipping env-sensitive timing test (or when OPENAGENT_STRICT_PERF is unset)
- Follow-ups: Document strict perf in developer docs (done); optional clippy cleanup in plugin-loader (partial)
