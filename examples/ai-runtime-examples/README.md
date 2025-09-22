# OpenAgent Terminal AI Runtime Examples

This directory contains comprehensive end-to-end examples demonstrating secure AI provider configuration and real-world usage patterns for OpenAgent Terminal.

## Table of Contents
1. [Basic Setup Examples](#basic-setup-examples)
2. [Multi-Provider Configurations](#multi-provider-configurations)
3. [Production Deployment Examples](#production-deployment-examples)
4. [Development Workflows](#development-workflows)
5. [Security Hardening Examples](#security-hardening-examples)

## Basic Setup Examples

### Example 1: Local-Only AI with Ollama

**Scenario**: Privacy-focused developer who wants AI assistance without cloud dependencies.

**Configuration**: `examples/local-only-ollama/`
- Ollama-only setup
- No internet required after initial model download
- Full privacy and data control

**Use Cases**:
- Code completion and generation
- Command assistance
- Project analysis
- Offline development

### Example 2: Hybrid Cloud Setup

**Scenario**: Team environment with both local and cloud AI capabilities.

**Configuration**: `examples/hybrid-cloud/`
- Primary: Ollama (local)
- Fallback: OpenAI (cloud)
- Smart routing based on task complexity

**Use Cases**:
- Fast local responses for simple tasks
- Cloud AI for complex reasoning
- Automatic failover and load balancing

### Example 3: Enterprise Multi-Provider

**Scenario**: Large organization with multiple AI provider contracts.

**Configuration**: `examples/enterprise-multi-provider/`
- Multiple providers configured
- Provider selection by team/project
- Cost optimization and redundancy

**Use Cases**:
- Team-specific AI preferences
- Provider redundancy and failover
- Cost allocation and tracking

## Multi-Provider Configurations

### Ollama + OpenAI Configuration

```toml
# ~/.config/openagent-terminal/openagent-terminal.toml
[ai]
enabled = true
provider = "ollama"  # Primary provider
fallback_providers = ["openai"]  # Fallback chain
never_auto_run = true
trigger_key = "Ctrl+Shift+A"

# Local AI (primary)
[ai.providers.ollama]
endpoint_env = "OPENAGENT_OLLAMA_ENDPOINT"
model_env = "OPENAGENT_OLLAMA_MODEL"
default_endpoint = "http://localhost:11434"
default_model = "codellama:7b"
priority = 1

# Cloud AI (fallback)
[ai.providers.openai]
api_key_env = "OPENAGENT_OPENAI_API_KEY"
model_env = "OPENAGENT_OPENAI_MODEL"
endpoint_env = "OPENAGENT_OPENAI_ENDPOINT"
default_endpoint = "https://api.openai.com/v1"
default_model = "gpt-4o-mini"
priority = 2
```

**Environment Setup**:
```bash
#!/bin/bash
# secure-ai-env.sh

# Ollama (local) - no API key needed
export OPENAGENT_OLLAMA_ENDPOINT="http://localhost:11434"
export OPENAGENT_OLLAMA_MODEL="codellama:7b"

# OpenAI (fallback) - requires API key
export OPENAGENT_OPENAI_API_KEY="sk-your-openai-key-here"
export OPENAGENT_OPENAI_MODEL="gpt-4o-mini"
```

### Full Multi-Provider Configuration

```toml
# Complete multi-provider setup

## Provider management

- List all known providers and show the active one:

  openagent-terminal ai provider list --include-defaults

- Switch active provider (persists to your config):

  openagent-terminal ai provider set anthropic

- JSON for scripting:

  openagent-terminal ai provider list --json
[ai]
enabled = true
provider = "ollama"
fallback_providers = ["openai", "anthropic", "openrouter"]
max_concurrent_agents = 3
request_timeout_ms = 30000

# Provider configurations
[ai.providers.ollama]
endpoint_env = "OPENAGENT_OLLAMA_ENDPOINT"
model_env = "OPENAGENT_OLLAMA_MODEL"
default_endpoint = "http://localhost:11434"
default_model = "codellama:7b"
priority = 1
use_for = ["code", "command", "quick_questions"]

[ai.providers.openai]
api_key_env = "OPENAGENT_OPENAI_API_KEY"
model_env = "OPENAGENT_OPENAI_MODEL"
endpoint_env = "OPENAGENT_OPENAI_ENDPOINT"
default_endpoint = "https://api.openai.com/v1"
default_model = "gpt-4o-mini"
priority = 2
use_for = ["complex_reasoning", "analysis"]

[ai.providers.anthropic]
api_key_env = "OPENAGENT_ANTHROPIC_API_KEY"
model_env = "OPENAGENT_ANTHROPIC_MODEL"
endpoint_env = "OPENAGENT_ANTHROPIC_ENDPOINT"
default_endpoint = "https://api.anthropic.com/v1"
default_model = "claude-3-haiku-20240307"
priority = 3
use_for = ["writing", "documentation"]

[ai.providers.openrouter]
api_key_env = "OPENAGENT_OPENROUTER_API_KEY"
model_env = "OPENAGENT_OPENROUTER_MODEL"
endpoint_env = "OPENAGENT_OPENROUTER_ENDPOINT"
default_endpoint = "https://openrouter.ai/api/v1"
priority = 4
use_for = ["specialized_models"]
```

## Production Deployment Examples

### Docker Compose with Ollama

```yaml
# examples/production-docker/docker-compose.yml
version: '3.8'

services:
  ollama:
    image: ollama/ollama:latest
    ports:
      - "11434:11434"
    volumes:
      - ollama_data:/root/.ollama
    environment:
      - OLLAMA_HOST=0.0.0.0
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]

  openagent-terminal:
    build: 
      context: .
      dockerfile: Dockerfile
    depends_on:
      - ollama
    environment:
      - OPENAGENT_OLLAMA_ENDPOINT=http://ollama:11434
      - OPENAGENT_OLLAMA_MODEL=codellama:7b
      - OPENAGENT_OPENAI_API_KEY=${OPENAI_API_KEY}
      - OPENAGENT_ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
    volumes:
      - ./config:/home/user/.config/openagent-terminal
      - ai_history:/home/user/.local/share/openagent-terminal

volumes:
  ollama_data:
  ai_history:
```

> Note: OLLAMA_HOST configures the Ollama server container itself. To configure OpenAgent Terminal as a client, set OPENAGENT_OLLAMA_ENDPOINT and OPENAGENT_OLLAMA_MODEL in the terminal’s environment.
>
> Example (client-side):
>
> ```bash
> export OPENAGENT_OLLAMA_ENDPOINT="http://ollama:11434"   # or http://localhost:11434 when running locally
> export OPENAGENT_OLLAMA_MODEL="codellama:7b"
> ```

### Kubernetes Deployment

```yaml
# examples/production-k8s/openagent-deployment.yml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: openagent-terminal
spec:
  replicas: 3
  selector:
    matchLabels:
      app: openagent-terminal
  template:
    metadata:
      labels:
        app: openagent-terminal
    spec:
      containers:
      - name: openagent-terminal
        image: openagent-terminal:latest
        env:
        - name: OPENAGENT_OLLAMA_ENDPOINT
          value: "http://ollama-service:11434"
        - name: OPENAGENT_OPENAI_API_KEY
          valueFrom:
            secretKeyRef:
              name: ai-secrets
              key: openai-api-key
        - name: OPENAGENT_ANTHROPIC_API_KEY
          valueFrom:
            secretKeyRef:
              name: ai-secrets
              key: anthropic-api-key
        volumeMounts:
        - name: config
          mountPath: /home/user/.config/openagent-terminal
        - name: history
          mountPath: /home/user/.local/share/openagent-terminal
      volumes:
      - name: config
        configMap:
          name: openagent-config
      - name: history
        persistentVolumeClaim:
          claimName: ai-history-pvc

---
apiVersion: v1
kind: Secret
metadata:
  name: ai-secrets
type: Opaque
data:
  openai-api-key: <base64-encoded-key>
  anthropic-api-key: <base64-encoded-key>
```

## Development Workflows

### Workflow 1: Full-Stack Development

**Use Case**: Web developer working on a full-stack application

**Setup**:
```bash
# Development environment setup
export OPENAGENT_OLLAMA_ENDPOINT="http://localhost:11434"
export OPENAGENT_OLLAMA_MODEL="codellama:13b"  # Larger model for complex code
export OPENAGENT_OPENAI_API_KEY="sk-your-key"
export OPENAGENT_OPENAI_MODEL="gpt-4"  # Premium model for architecture decisions

# Start the terminal with development profile
openagent-terminal --profile development
```

**Configuration**:
```toml
# ~/.config/openagent-terminal/profiles/development.toml
[ai]
enabled = true
provider = "ollama"
fallback_providers = ["openai"]
inline_suggestions = true
animated_typing = true

# Enhanced context for development
[ai.context]
enabled = true
max_bytes = 65536  # Larger context for complex codebases
providers = ["env", "git", "file_tree", "package_json", "cargo_toml"]

[ai.context.file_tree]
max_entries = 1000
exclude_patterns = ["node_modules", "target", "dist", ".git"]
```

**Example Usage**:
1. **Code Generation**: "Create a React component for user authentication"
2. **Debugging**: "Why is this async function hanging?"
3. **Architecture**: "Design a microservices structure for this e-commerce app"
4. **Testing**: "Generate unit tests for this API endpoint"

### Workflow 2: DevOps and Infrastructure

**Use Case**: DevOps engineer managing cloud infrastructure

**Setup**:
```bash
# DevOps-focused AI setup
export OPENAGENT_OLLAMA_MODEL="deepseek-coder:6.7b"  # Code-focused model
export OPENAGENT_OPENAI_MODEL="gpt-4"  # For complex infrastructure design

# Enable security lens for dangerous commands
export OPENAGENT_SECURITY_ENABLED=true
export OPENAGENT_SECURITY_LEVEL=strict
```

**Configuration**:
```toml
# ~/.config/openagent-terminal/profiles/devops.toml
[ai]
enabled = true
provider = "ollama"
fallback_providers = ["openai"]

# DevOps-specific agents
[ai.agents.infrastructure]
enabled = true
supported_tools = ["terraform", "kubernetes", "docker", "ansible"]
safety_checks = true

[security]
enabled = true
block_critical = true  # Block potentially destructive commands

# Custom DevOps security patterns
[[security.custom_patterns]]
pattern = "(?i)(terraform|pulumi)\\s+destroy"
risk_level = "Critical"
message = "Infrastructure destruction command"

[[security.custom_patterns]]
pattern = "(?i)kubectl\\s+delete\\s+.*prod"
risk_level = "Critical"
message = "Deleting production Kubernetes resources"
```

**Example Usage**:
1. **Infrastructure as Code**: "Generate Terraform for AWS EKS cluster"
2. **Troubleshooting**: "Debug this Kubernetes pod crash loop"
3. **Security**: "Review this Dockerfile for security issues"
4. **Monitoring**: "Create Prometheus alerts for this service"

### Workflow 3: Data Science and ML

**Use Case**: Data scientist working with ML models and data analysis

**Setup**:
```bash
# ML-focused setup with specialized models
export OPENAGENT_OLLAMA_MODEL="codellama:34b"  # Large model for complex analysis
export OPENAGENT_OPENAI_MODEL="gpt-4"
export OPENAGENT_ANTHROPIC_MODEL="claude-3-opus-20240229"  # For data interpretation
```

**Configuration**:
```toml
# ~/.config/openagent-terminal/profiles/datascience.toml
[ai]
enabled = true
provider = "ollama"
fallback_providers = ["anthropic", "openai"]
request_timeout_ms = 60000  # Longer timeout for complex analysis

[ai.context]
enabled = true
providers = ["env", "git", "file_tree", "jupyter", "requirements_txt"]

[ai.agents.data_analysis]
enabled = true
supported_languages = ["python", "r", "sql", "julia"]
supported_frameworks = ["pandas", "numpy", "scikit-learn", "tensorflow", "pytorch"]
```

**Example Usage**:
1. **Data Analysis**: "Analyze this CSV for patterns and anomalies"
2. **ML Code**: "Create a neural network for image classification"
3. **Visualization**: "Generate matplotlib code for this dataset"
4. **Statistics**: "Explain the statistical significance of these results"

## Security Hardening Examples

### Enterprise Security Configuration

```toml
# ~/.config/openagent-terminal/security-hardened.toml
[ai]
enabled = true
provider = "ollama"  # Local-only for maximum security
never_auto_run = true
require_confirmation_for_all = true

# Strict security settings
[security]
enabled = true
block_critical = true
gate_paste_events = true
log_all_interactions = true

[security.require_confirmation]
Safe = false
Caution = true
Warning = true
Critical = true

# Enhanced security patterns
[[security.custom_patterns]]
pattern = "(?i)(rm|del|delete).*-r.*/"
risk_level = "Critical"
message = "Recursive deletion detected"

[[security.custom_patterns]]
pattern = "(?i)(curl|wget).*\\|.*sh"
risk_level = "Critical"
message = "Downloading and executing scripts"

[[security.custom_patterns]]
pattern = "(?i)sudo.*passwd"
risk_level = "Warning"
message = "Password modification"

# Rate limiting
[security.rate_limit]
enabled = true
max_detections = 5
window_seconds = 300

# Audit logging
[audit]
enabled = true
log_file = "/var/log/openagent-terminal/audit.log"
include_context = false  # Don't log sensitive context
include_responses = false  # Don't log AI responses
```

### GDPR-Compliant Configuration

```toml
# ~/.config/openagent-terminal/gdpr-compliant.toml
[ai]
enabled = true
provider = "ollama"  # Local processing for GDPR compliance
never_send_personal_data = true

# Privacy settings
[privacy]
enabled = true
data_retention_days = 30  # Automatic cleanup
anonymize_logs = true
redact_sensitive_patterns = true

# Context filtering
[ai.context]
enabled = true
privacy_filter = true
exclude_patterns = [
    "(?i)(email|mail).*@.*\\.(com|org|net)",  # Email addresses
    "(?i)\\b\\d{4}[\\s-]?\\d{4}[\\s-]?\\d{4}[\\s-]?\\d{4}\\b",  # Credit cards
    "(?i)\\b\\d{3}-\\d{2}-\\d{4}\\b",  # SSNs
    "(?i)password[\\s:=]+\\S+",  # Passwords
]

# Local history only
[blocks.export]
allow_external = false
encryption_required = true
```

### Air-Gapped Environment

```toml
# ~/.config/openagent-terminal/air-gapped.toml
[ai]
enabled = true
provider = "ollama"  # Only local AI allowed
cloud_providers_disabled = true
internet_access_required = false

[ai.providers.ollama]
endpoint_env = "OPENAGENT_OLLAMA_ENDPOINT"
model_env = "OPENAGENT_OLLAMA_MODEL"
default_endpoint = "http://localhost:11434"
default_model = "codellama:7b"
offline_mode = true

# Disable all cloud features
[ai.context]
enabled = true
providers = ["env", "git", "file_tree"]  # Only local providers
network_providers_disabled = true

# Enhanced local security
[security]
enabled = true
network_commands_blocked = true
file_transfer_blocked = true
external_tool_execution_blocked = true
```

## Usage Examples

### Example Session 1: Setting Up a New Project

```bash
# Terminal session with AI assistance

# 1. Project initialization
$ mkdir my-rust-project && cd my-rust-project

# 2. Ask AI for help (Ctrl+Shift+A)
AI> "Help me set up a new Rust project with web API and database"

# AI suggests:
# cargo init --name my-rust-project
# Add these dependencies to Cargo.toml: tokio, axum, sqlx...

# 3. Follow AI suggestions
$ cargo init --name my-rust-project
$ # Edit Cargo.toml based on AI suggestions

# 4. Generate boilerplate code
AI> "Create a basic axum web server with health check endpoint"

# AI generates complete main.rs with proper structure

# 5. Database setup
AI> "How do I set up PostgreSQL with SQLx migrations?"

# AI provides step-by-step database setup instructions
```

### Example Session 2: Debugging Complex Issues

```bash
# Debugging session

# 1. Error occurs
$ cargo test
error[E0308]: mismatched types
  expected `Result<(), Box<dyn Error>>`, found `()`

# 2. Get AI help with error context
AI> "Fix this Rust compilation error" 
# (AI automatically sees the error from terminal context)

# AI explains the error and provides fixes

# 3. Performance issue
$ cargo bench
# Benchmark shows poor performance

AI> "This benchmark is slow, how can I optimize it?"

# AI analyzes the code and suggests optimization strategies
```

### Example Session 3: Learning New Technologies

```bash
# Learning session

# 1. Explore new tech
AI> "Explain Docker containers and show me a practical example"

# AI provides explanation and generates Dockerfile

# 2. Hands-on practice
AI> "Create a Docker setup for my Rust web app with PostgreSQL"

# AI generates docker-compose.yml with best practices

# 3. Advanced concepts
AI> "How do I implement health checks and graceful shutdown?"

# AI shows implementation patterns with code examples
```

## Monitoring and Observability

### Metrics Collection Example

```bash
# Set up automated metrics collection
#!/bin/bash
# collect-ai-metrics.sh

# Export current metrics
openagent-terminal ai metrics --export /tmp/ai-metrics.json

# Process with jq for dashboard
cat /tmp/ai-metrics.json | jq '.agent_stats | to_entries[] | {
  agent: .key,
  success_rate: (.value.success_count / .value.request_count),
  avg_response_time: .value.avg_response_time,
  current_load: .value.current_load
}'

# Send to monitoring system (example)
curl -X POST https://your-monitoring.com/metrics \
  -H "Content-Type: application/json" \
  -d @/tmp/ai-metrics.json
```

### Health Checks

```bash
# health-check.sh - Monitor AI system health
#!/bin/bash

set -e

echo "🏥 OpenAgent Terminal AI Health Check"
echo "===================================="

# Check provider configurations
echo "📋 Checking provider configurations..."
openagent-terminal ai validate --include-defaults --json > /tmp/provider-health.json

if jq -e '.[] | select(.ok == false)' /tmp/provider-health.json > /dev/null; then
    echo "❌ Some providers have issues:"
    jq -r '.[] | select(.ok == false) | "  - \(.name): \(.error)"' /tmp/provider-health.json
    exit 1
fi

echo "✅ All configured providers are healthy"

# Check agent performance
echo "📊 Checking agent performance..."
openagent-terminal ai metrics --verify

echo "✅ AI system is healthy and operational"
```

## Getting Started

1. **Choose your setup**: Select an example that matches your use case
2. **Copy configuration**: Use the provided TOML files as a starting point
3. **Set environment variables**: Configure your AI provider credentials
4. **Test the setup**: Run validation and basic tests
5. **Customize**: Adjust settings based on your specific needs

Each example includes:
- Complete configuration files
- Environment setup scripts
- Usage demonstrations
- Security considerations
- Monitoring setup

For more detailed information, see the [AI Runtime & CLI Guide](../../docs/AI_RUNTIME_CLI_GUIDE.md).
