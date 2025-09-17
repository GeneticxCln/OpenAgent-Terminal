# OpenAgent Terminal AI Runtime & CLI Guide

## Table of Contents
1. [Overview](#overview)
2. [AI Runtime Architecture](#ai-runtime-architecture)
3. [Agent System](#agent-system)
4. [Multi-Provider Configuration](#multi-provider-configuration)
5. [CLI Commands](#cli-commands)
6. [Agent Performance Metrics](#agent-performance-metrics)
7. [Best Practices](#best-practices)
8. [Troubleshooting](#troubleshooting)

## Overview

OpenAgent Terminal features a sophisticated AI runtime system that supports multiple AI providers, intelligent agent routing, performance monitoring, and comprehensive CLI tooling. The system is designed for privacy-first operation with local AI providers (Ollama) while supporting cloud providers when needed.

### Key Features
- **Multi-Provider Support**: Ollama (local), OpenAI, Anthropic, OpenRouter
- **Intelligent Agent Routing**: Automatic selection of best agent for each task
- **Performance Metrics**: Real-time agent scoring and utilization tracking
- **Secure Configuration**: Environment-based credentials with provider isolation
- **Streaming Support**: Real-time AI responses with Retry-After handling
- **CLI Export**: History export in JSON/CSV/JSONL formats with SQLite fallback

## AI Runtime Architecture

### Core Components

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   AI Providers  │    │ Agent Manager   │    │ Performance     │
│                 │    │                 │    │ Metrics         │
│ • Ollama        │◄───┤ • Request       │────┤ • Success Rate  │
│ • OpenAI        │    │   Routing       │    │ • Response Time │
│ • Anthropic     │    │ • Load          │    │ • Satisfaction  │
│ • OpenRouter    │    │   Balancing     │    │ • Utilization   │
│ • Custom        │    │ • Collaboration │    │ • Load Tracking │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                        │                        │
         └────────────────────────┼────────────────────────┘
                                  │
                         ┌─────────────────┐
                         │   CLI Tools     │
                         │                 │
                         │ • ai validate   │
                         │ • ai migrate    │
                         │ • ai export     │
                         │ • ai purge      │
                         └─────────────────┘
```

### Agent Types

1. **Command Agent**: Terminal command generation and assistance
2. **Code Generation Agent**: Code writing, completion, refactoring
3. **Project Context Agent**: Repository analysis and understanding
4. **Quality Agent**: Code review, security scanning, style checks
5. **Collaboration Hub**: Multi-agent workflows and coordination

## Agent System

### Agent Performance Tracking

The system automatically tracks performance metrics for each agent:

```rust
pub struct AgentPerformanceStats {
    pub request_count: u64,
    pub success_count: u64,
    pub error_count: u64,
    pub avg_response_time: Duration,
    pub min_response_time: Duration,
    pub max_response_time: Duration,
    pub satisfaction_scores: Vec<f32>,
    pub avg_satisfaction: f32,
    pub current_load: u32,
    pub peak_concurrent_requests: u32,
}
```

### Intelligent Routing

Agents are automatically selected based on:
- **Success Rate** (40% weight): Historical success/failure ratio
- **Response Time** (30% weight): Average processing speed
- **User Satisfaction** (20% weight): User feedback scores
- **Current Load** (10% weight): Real-time utilization

### Agent Capabilities

```toml
# Example agent capability definition
[agent.code_generation]
supported_languages = ["rust", "python", "javascript", "go"]
supported_frameworks = ["tokio", "fastapi", "react", "gin"]
features = ["generation", "completion", "refactoring", "explanation"]
requires_internet = false  # for Ollama
privacy_level = "Local"
```

## Multi-Provider Configuration

### Secure Provider Setup

OpenAgent Terminal uses isolated environment variables to prevent credential leakage:

```toml
# ~/.config/openagent-terminal/openagent-terminal.toml
[ai]
enabled = true
provider = "ollama"  # Primary provider
trigger_key = "Ctrl+Shift+A"
never_auto_run = true

# Provider-specific configurations
[ai.providers.ollama]
endpoint_env = "OPENAGENT_OLLAMA_ENDPOINT"
model_env = "OPENAGENT_OLLAMA_MODEL"
default_endpoint = "http://localhost:11434"
default_model = "codellama"

[ai.providers.openai]
api_key_env = "OPENAGENT_OPENAI_API_KEY"
model_env = "OPENAGENT_OPENAI_MODEL"
endpoint_env = "OPENAGENT_OPENAI_ENDPOINT"
default_endpoint = "https://api.openai.com/v1"
default_model = "gpt-3.5-turbo"

[ai.providers.anthropic]
api_key_env = "OPENAGENT_ANTHROPIC_API_KEY"
model_env = "OPENAGENT_ANTHROPIC_MODEL"
endpoint_env = "OPENAGENT_ANTHROPIC_ENDPOINT"
default_endpoint = "https://api.anthropic.com/v1"
default_model = "claude-3-haiku-20240307"

[ai.providers.openrouter]
api_key_env = "OPENAGENT_OPENROUTER_API_KEY"
model_env = "OPENAGENT_OPENROUTER_MODEL"
endpoint_env = "OPENAGENT_OPENROUTER_ENDPOINT"
default_endpoint = "https://openrouter.ai/api/v1"
```

### Environment Variables

Set up your environment with provider-specific variables:

```bash
# For Ollama (local AI - no API key needed)
export OPENAGENT_OLLAMA_ENDPOINT="http://localhost:11434"
export OPENAGENT_OLLAMA_MODEL="codellama:7b"

# For OpenAI
export OPENAGENT_OPENAI_API_KEY="sk-your-openai-key-here"
export OPENAGENT_OPENAI_MODEL="gpt-4o-mini"

# For Anthropic
export OPENAGENT_ANTHROPIC_API_KEY="sk-ant-your-anthropic-key"
export OPENAGENT_ANTHROPIC_MODEL="claude-3-sonnet-20240229"

# For OpenRouter
export OPENAGENT_OPENROUTER_API_KEY="sk-or-your-openrouter-key"
export OPENAGENT_OPENROUTER_MODEL="anthropic/claude-3-haiku-20240307"
```

### Provider Switching

Runtime provider switching is supported through configuration:

```bash
# Switch to OpenAI
openagent-terminal --ai-provider openai

# Use specific model
openagent-terminal --ai-provider anthropic --ai-model claude-3-opus-20240229
```

## CLI Commands

### AI Validation

Validate provider configurations and credentials:

```bash
# Validate all configured providers
openagent-terminal ai validate

# Validate specific provider
openagent-terminal ai validate --provider ollama

# Include default providers in validation
openagent-terminal ai validate --include-defaults

# JSON output for automation
openagent-terminal ai validate --json
```

**Example Output:**
```
🔍 Looking for AI providers...
✓ ollama: OK
✗ openai: Missing API key (set OPENAGENT_OPENAI_API_KEY)
✓ anthropic: OK
⚠️  openrouter: Model not specified

Summary: 2/4 providers ready
```

### Migration Tools

Migrate from legacy environment variables to secure configuration:

```bash
# Detect legacy variables and show migration path
openagent-terminal ai migrate

# Generate config file
openagent-terminal ai migrate --config-out ~/.config/openagent-terminal/ai-providers.toml --apply

# Generate environment migration script
openagent-terminal ai migrate --write-env-snippet secure-ai-env.sh
```

### History Export

Export AI interaction history in multiple formats:

```bash
# Export to JSON (prettified)
openagent-terminal ai history-export --format json --to ai-history.json

# Export to CSV for analysis
openagent-terminal ai history-export --format csv --to ai-history.csv

# Export to JSONL for streaming processing
openagent-terminal ai history-export --format jsonl --to ai-history.jsonl
```

**Export Features:**
- **Automatic Fallback**: Falls back to JSONL if SQLite database unavailable
- **Progress Reporting**: Shows real-time export progress
- **Error Recovery**: Handles corrupted records gracefully
- **Format Validation**: Validates export format before processing

### History Management

```bash
# Purge old history, keep last 100 entries
openagent-terminal ai history-purge --keep-last 100

# Complete purge (keep 0 entries)
openagent-terminal ai history-purge --keep-last 0
```

## Agent Performance Metrics

### Real-Time Metrics

The system continuously tracks agent performance:

```bash
# View agent performance dashboard
openagent-terminal ai metrics

# Export metrics to file
openagent-terminal ai metrics --export metrics.json

# Reset performance metrics
openagent-terminal ai metrics --reset
```

### Metrics Available

1. **Response Metrics**
   - Average response time
   - Min/max response times
   - Success rate
   - Error rate

2. **Load Metrics**
   - Current concurrent requests
   - Peak concurrent requests
   - Total processing time
   - Utilization percentage

3. **Quality Metrics**
   - User satisfaction scores
   - Average satisfaction
   - Response quality trends

4. **Routing Efficiency**
   - Agent selection accuracy
   - Load balancing effectiveness
   - Routing decision time

### Performance API

Access metrics programmatically:

```rust
use openagent_terminal_ai::agents::manager::AgentManager;

let manager = AgentManager::new();
let metrics = manager.get_metrics().await;
let agent_stats = manager.get_agent_metrics("code_generation").await;

// Record user satisfaction
manager.record_satisfaction("command_agent", 0.85).await;

// Get routing recommendations
let recommendations = manager.get_routing_recommendations().await;
```

## Best Practices

### Security

1. **Use Local AI First**: Start with Ollama for maximum privacy
2. **Secure Credentials**: Use provider-specific environment variables
3. **Audit Trail**: Export history regularly for compliance
4. **Access Control**: Limit API keys to necessary scopes

### Performance

1. **Monitor Metrics**: Track agent performance regularly
2. **Load Balance**: Distribute requests across capable agents
3. **Cache Results**: Configure project context caching
4. **Timeout Management**: Set appropriate request timeouts

### Configuration

1. **Provider Redundancy**: Configure multiple providers for reliability
2. **Model Selection**: Choose appropriate models for tasks
3. **Rate Limiting**: Respect provider rate limits
4. **Cost Management**: Monitor usage for paid providers

### Maintenance

1. **Regular Exports**: Export history before purging
2. **Metric Analysis**: Review performance trends monthly
3. **Configuration Updates**: Keep provider configs current
4. **Security Updates**: Update API keys regularly

## Troubleshooting

### Common Issues

#### Provider Not Found
```bash
Error: Agent not found: openai
```
**Solution**: Validate provider configuration
```bash
openagent-terminal ai validate --provider openai --include-defaults
```

#### API Key Issues
```bash
Error: Missing API key (set OPENAGENT_OPENAI_API_KEY)
```
**Solution**: Set the correct environment variable
```bash
export OPENAGENT_OPENAI_API_KEY="your-key-here"
```

#### Export Failures
```bash
Warning: SQLite database not available. Attempting JSONL fallback...
```
**Solution**: This is normal behavior; the system automatically falls back to JSONL

#### Performance Issues
```bash
Warning: Agent 'command_agent' has high error rate (45%)
```
**Solution**: Check agent configuration and provider status
```bash
openagent-terminal ai validate
openagent-terminal ai metrics
```

### Debug Mode

Enable debug logging for detailed troubleshooting:

```bash
export RUST_LOG=openagent_terminal_ai=debug
openagent-terminal
```

### Health Checks

Verify system health:

```bash
# Check all components
openagent-terminal ai validate --include-defaults --json

# Test specific agent
openagent-terminal ai test-agent --agent command --prompt "list files"

# Verify metrics collection
openagent-terminal ai metrics --verify
```

### Log Analysis

Monitor AI system logs:

```bash
# View AI-specific logs
tail -f ~/.local/share/openagent-terminal/logs/ai.log

# Monitor streaming events  
export OPENAGENT_AI_LOG_VERBOSITY=verbose
```

## Advanced Usage

### Custom Providers

Add custom OpenAI-compatible providers:

```toml
[ai.providers.custom]
api_key_env = "OPENAGENT_CUSTOM_API_KEY"
model_env = "OPENAGENT_CUSTOM_MODEL"
endpoint_env = "OPENAGENT_CUSTOM_ENDPOINT"
default_endpoint = "https://your-custom-api.com/v1"
default_model = "your-custom-model"
```

### Workflow Integration

Integrate with external workflows:

```bash
# Export for analysis pipeline
openagent-terminal ai history-export --format jsonl | \
  jq '.[] | select(.mode == "streaming")' | \
  your-analysis-tool

# Automated metric collection
openagent-terminal ai metrics --export /tmp/metrics.json
python your-dashboard.py /tmp/metrics.json
```

### Performance Tuning

Optimize for your usage patterns:

```toml
[ai]
# Increase timeout for complex requests
request_timeout_ms = 60000

# Enable more concurrent agents
max_concurrent_agents = 5

# Tune streaming performance
stream_redraw_ms = 8  # Lower = more responsive, higher = less CPU
```

---

For more information, see:
- [AI Environment Security](AI_ENVIRONMENT_SECURITY.md)
- [Agent Architecture](AGENT_ARCHITECTURE.md)
- [Configuration Manual](configuration.md)