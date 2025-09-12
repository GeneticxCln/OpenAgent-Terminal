# ADR-001: AI Architecture

## Status
Accepted

## Date
2024-08-30

## Context

OpenAgent Terminal needs to integrate AI capabilities to assist users with command generation and shell automation. The key requirements are:

1. Privacy-first approach
2. Multiple provider support
3. No auto-execution of commands
4. Extensible architecture
5. Minimal performance impact

## Decision

We will implement a provider-based architecture with the following characteristics:

### 1. Provider Pattern

Use a trait-based provider pattern to support multiple AI backends:

```rust
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn propose(&self, req: AiRequest) -> Result<Vec<AiProposal>, String>;
}
```

### 2. Local-First Default

Ollama will be the default provider because:
- Runs entirely locally
- No data leaves the user's machine
- No API keys required
- Free to use

### 3. Opt-in Cloud Providers

Cloud providers (OpenAI, Anthropic) are available but require:
- Explicit configuration
- API keys in environment variables
- User consent

### 4. Safety Guarantees

Commands are NEVER auto-executed:
- AI only generates proposals
- User must review proposals
- User explicitly copies or executes commands
- Audit logging available

### 5. Async Architecture

AI calls are non-blocking:
- UI remains responsive during queries
- Loading states shown
- Timeouts configured (30s default)
- Error handling with fallbacks

## Consequences

### Positive

1. **Privacy Protected**: Users can use AI without sending data to cloud
2. **Flexibility**: Easy to add new providers
3. **Safety**: No risk of unintended command execution
4. **Performance**: Non-blocking UI ensures responsiveness
5. **Extensible**: Plugin system can be added later

### Negative

1. **Complexity**: Multiple providers increase maintenance
2. **Local Requirements**: Ollama requires local setup
3. **Network Dependency**: Cloud providers need internet
4. **Resource Usage**: Local models use significant RAM/CPU

### Neutral

1. **Configuration**: Users must configure providers
2. **API Keys**: Cloud providers need secure key management
3. **Model Selection**: Users must choose appropriate models

## Implementation Details

### Module Structure

```
openagent-terminal-ai/
├── src/
│   ├── lib.rs           # Trait definitions
│   ├── providers/
│   │   ├── mod.rs       # Provider factory
│   │   ├── ollama.rs    # Ollama implementation
│   │   ├── openai.rs    # OpenAI implementation
│   │   └── anthropic.rs # Anthropic implementation
│   └── runtime.rs       # AI runtime manager
```

### Configuration

```toml
[ai]
enabled = true
provider = "ollama"  # Default

[ai.ollama]
endpoint = "http://localhost:11434"
model = "codellama"

[ai.privacy]
strip_sensitive = true
mask_patterns = ["password", "token", "key"]
audit_log = true
never_auto_run = true
```

### Error Handling

1. Provider unavailable → Show setup instructions
2. Network timeout → Return error with retry option
3. Invalid response → Log and show generic error
4. Rate limiting → Implement exponential backoff

### Conversation History (Preview)

Purpose
- Maintain optional, privacy-respecting conversation history across sessions to improve continuity and user experience.

Data model (persisted locally; maps to default SQLite schema)
```sql
CREATE TABLE IF NOT EXISTS ai_conversations (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    message_type TEXT NOT NULL, -- user | assistant | system
    content TEXT NOT NULL,
    metadata JSON,
    timestamp INTEGER NOT NULL
);
```

Runtime behavior
- Messages are appended as interactions occur (user prompts, assistant responses, system events).
- When building provider requests, the runtime can include a bounded window of prior messages, constrained by:
  - [ai.context].max_bytes budget (see docs/ai.md)
  - Provider-specific token limits
  - Optional per-request limit (e.g., last N messages)

Privacy & security
- All outbound requests are sanitized before leaving the process (see docs/ai.md Privacy & sanitization).
- History is stored locally; no data leaves the machine unless a cloud provider is explicitly configured.
- Sensitive history may be encrypted at rest when storage.encrypt_sensitive_data = true.

Configuration (see also the storage configuration)
- Enable/disable: storage.enable_ai_history (default: true)
- Retention: storage.ai_history_days (default: 90)
- Export/delete: planned UI/CLI affordances; manual deletion supported via storage backend.

API sketch
```rust
pub struct ConversationManager {
    storage: Arc<StorageBackend>,
    current_session: String,
}

impl ConversationManager {
    pub fn store_message(&self, message_type: MessageType, content: &str, metadata: Option<serde_json::Value>) -> Result<()> {
        // Insert into ai_conversations with current_session and timestamp
        # /* see docs/github-issues/004-persistent-data-storage.md for full example */
        Ok(())
    }

    pub fn load_conversation_history(&self, limit: Option<u32>) -> Result<Vec<ConversationMessage>> {
        // Query by session_id ordered by timestamp, with optional limit
        Ok(vec![])
    }
}
```
## Alternatives Considered

### 1. Single Provider Only

**Rejected**: Would limit user choice and create vendor lock-in

### 2. Auto-execution with Confirmation

**Rejected**: Too risky, even with confirmation dialogs

### 3. Embedded Models

**Rejected**: Would make binary size too large (>1GB)

### 4. Server-Side Processing

**Rejected**: Violates privacy-first principle

## Future Considerations

1. **Streaming Responses**: Show partial results as they arrive
2. **Context Management**: Maintain conversation history
3. **Fine-tuning**: Support custom models for terminal commands
4. **Caching**: Cache common queries for faster response
5. **Offline Mode**: Basic command suggestions without AI

## References

- [Ollama Documentation](https://ollama.ai/docs)
- [OpenAI API Best Practices](https://platform.openai.com/docs/guides/best-practices)
- [Privacy by Design Framework](https://www.ipc.on.ca/wp-content/uploads/Resources/7foundationalprinciples.pdf)

## Sign-off

- Architecture Team: Approved
- Security Team: Approved
- Product Team: Approved

---

*This ADR documents the key architectural decisions for AI integration in OpenAgent Terminal.*
