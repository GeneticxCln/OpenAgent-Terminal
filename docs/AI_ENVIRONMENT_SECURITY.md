# AI Provider Environment Security

## Problem Statement

The current AI runtime implementation has critical security and configuration issues:

1. **Cross-provider pollution**: Environment variables are mapped across providers
2. **Global environment mutation**: `std::env::set_var()` affects entire process
3. **Credential leakage**: Different providers access each other's credentials
4. **Configuration conflicts**: Cannot use multiple providers simultaneously

## Current Problematic Implementation

```rust
// PROBLEMATIC: Sets same values for different providers
if let Some(env_name) = api_key_env {
    if let Ok(value) = std::env::var(env_name) {
        std::env::set_var("OPENAI_API_KEY", value.clone());
        std::env::set_var("ANTHROPIC_API_KEY", value);
    }
}
```

## Secure Solution: Provider-Specific Namespacing

### 1. Environment Variable Naming Convention

Use provider-specific prefixes to avoid conflicts:

```bash
# Provider-specific environment variables
export OPENAGENT_OPENAI_API_KEY="sk-..."
export OPENAGENT_OPENAI_MODEL="gpt-4"
export OPENAGENT_OPENAI_ENDPOINT="https://api.openai.com/v1"

export OPENAGENT_ANTHROPIC_API_KEY="sk-ant-..."
export OPENAGENT_ANTHROPIC_MODEL="claude-3-sonnet-20240229"
export OPENAGENT_ANTHROPIC_ENDPOINT="https://api.anthropic.com"

export OPENAGENT_OLLAMA_ENDPOINT="http://localhost:11434"
export OPENAGENT_OLLAMA_MODEL="codellama"

# Generic fallbacks (optional)
export OPENAGENT_AI_PROVIDER="openai"  # Default provider
```

### 2. Configuration Structure

```toml
[ai]
enabled = true
provider = "openai"

[ai.providers.openai]
api_key_env = "OPENAGENT_OPENAI_API_KEY"
model_env = "OPENAGENT_OPENAI_MODEL"
endpoint_env = "OPENAGENT_OPENAI_ENDPOINT"

[ai.providers.anthropic]
api_key_env = "OPENAGENT_ANTHROPIC_API_KEY"
model_env = "OPENAGENT_ANTHROPIC_MODEL"
endpoint_env = "OPENAGENT_ANTHROPIC_ENDPOINT"

[ai.providers.ollama]
endpoint_env = "OPENAGENT_OLLAMA_ENDPOINT"
model_env = "OPENAGENT_OLLAMA_MODEL"
```

### 3. Runtime Isolation

- Use provider-specific configuration containers
- Pass credentials directly to provider constructors
- Never mutate global environment
- Support multiple provider instances simultaneously

### 4. Security Benefits

- **Isolation**: Providers only access their specific credentials
- **Auditability**: Clear mapping of which provider uses which credentials
- **Flexibility**: Different providers can use different endpoints/models
- **Safety**: No global environment pollution
- **Multi-tenancy**: Support for multiple provider configurations

### 5. Migration Strategy

1. Update existing configurations to use namespaced environment variables
2. Maintain backward compatibility for a transition period
3. Add deprecation warnings for old environment variable patterns
4. Provide migration scripts for common configurations

## Validate and Migrate with CLI

- Validate providers: `openagent-terminal ai validate --include-defaults`
- Validate a specific provider: `openagent-terminal ai validate --provider openai`
- Migrate legacy envs to secure OPENAGENT_* (dry-run): `openagent-terminal ai migrate`
- Apply and write snippets:
  - `openagent-terminal ai migrate --apply --config-out ~/.config/openagent-terminal/openagent-terminal.toml --write-env-snippet ~/.config/openagent-terminal/ai.env`
  - Then `source ~/.config/openagent-terminal/ai.env` in your shell rc

Notes
- The migration helper never prints secrets; it references existing variables like ${OPENAI_API_KEY} in the generated snippet.
- See configs/example_secure_ai_config.toml for a full example.

## Ollama performance (server-side environment)

These variables are consumed by the Ollama server (not the client) and must be set in the environment of the `ollama serve` process.

- Enable Flash Attention (when supported by your GPU/driver):
  - `OLLAMA_FLASH_ATTENTION=true`
- Use half-precision KV cache for improved quality/perf on capable GPUs:
  - `OLLAMA_KV_CACHE_TYPE=f16`

Examples:

- Shell session:
  ```bash
  export OLLAMA_FLASH_ATTENTION=true
  export OLLAMA_KV_CACHE_TYPE=f16
  ollama serve
  ```

- systemd override:
  ```ini
  [Service]
  Environment="OLLAMA_FLASH_ATTENTION=true"
  Environment="OLLAMA_KV_CACHE_TYPE=f16"
  ```

Tip: Keep these alongside your OPENAGENT_OLLAMA_* client variables in your shell profile or deploy them via systemd for persistence.

## Implementation Requirements

1. **ProviderConfig trait**: Define common configuration interface
2. **Environment isolation**: Remove global env var setting
3. **Configuration validation**: Ensure required variables are set
4. **Error handling**: Clear error messages for missing/invalid credentials
5. **Testing**: Verify isolation between providers
