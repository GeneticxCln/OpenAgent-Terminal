#!/bin/bash

# Migration script for OpenAgent Terminal AI Provider Security
# Converts legacy environment variable setup to secure namespaced approach

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}==== OpenAgent Terminal AI Provider Migration ====${NC}"
echo
echo "This script helps migrate from legacy environment variables to the secure"
echo "namespaced approach that prevents credential leakage between providers."
echo

# Check if legacy environment variables are present
legacy_found=false

declare -A legacy_mappings=(
    ["OPENAI_API_KEY"]="OPENAGENT_OPENAI_API_KEY"
    ["OPENAI_API_BASE"]="OPENAGENT_OPENAI_ENDPOINT"
    ["OPENAI_MODEL"]="OPENAGENT_OPENAI_MODEL"
    ["ANTHROPIC_API_KEY"]="OPENAGENT_ANTHROPIC_API_KEY"
    ["ANTHROPIC_API_BASE"]="OPENAGENT_ANTHROPIC_ENDPOINT"
    ["ANTHROPIC_MODEL"]="OPENAGENT_ANTHROPIC_MODEL"
    ["OLLAMA_ENDPOINT"]="OPENAGENT_OLLAMA_ENDPOINT"
    ["OLLAMA_MODEL"]="OPENAGENT_OLLAMA_MODEL"
)

echo -e "${YELLOW}Scanning for legacy environment variables...${NC}"
echo

migration_commands=""

for legacy_var in "${!legacy_mappings[@]}"; do
    if [[ -n "${!legacy_var:-}" ]]; then
        legacy_found=true
        secure_var="${legacy_mappings[$legacy_var]}"
        value="${!legacy_var}"
        
        echo -e "${YELLOW}⚠${NC}  Found legacy variable: ${legacy_var}=\"${value}\""
        echo -e "    ${GREEN}→${NC} Should migrate to: ${secure_var}=\"${value}\""
        
        # Build migration command
        migration_commands+="export ${secure_var}=\"${value}\"\n"
        migration_commands+="unset ${legacy_var}  # Remove legacy variable\n"
        echo
    fi
done

if [[ "$legacy_found" == false ]]; then
    echo -e "${GREEN}✓${NC} No legacy environment variables found!"
    echo "Your environment is already secure or no AI credentials are configured."
    echo
else
    echo -e "${RED}Legacy environment variables detected!${NC}"
    echo "These variables can cause credential leakage between providers."
    echo
fi

# Generate secure environment setup
echo -e "${BLUE}=== Secure Environment Variable Setup ===${NC}"
echo
echo "Add these commands to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
echo

cat << 'EOF'
# OpenAgent Terminal - Secure AI Provider Configuration

# OpenAI Configuration
export OPENAGENT_OPENAI_API_KEY="sk-your-openai-api-key-here"
export OPENAGENT_OPENAI_MODEL="gpt-3.5-turbo"  # or gpt-4
export OPENAGENT_OPENAI_ENDPOINT="https://api.openai.com/v1"  # optional

# Anthropic Configuration
export OPENAGENT_ANTHROPIC_API_KEY="sk-ant-your-anthropic-key-here"
export OPENAGENT_ANTHROPIC_MODEL="claude-3-haiku-20240307"
export OPENAGENT_ANTHROPIC_ENDPOINT="https://api.anthropic.com/v1"  # optional

# Ollama Configuration (for local deployment)
export OPENAGENT_OLLAMA_ENDPOINT="http://localhost:11434"  # optional
export OPENAGENT_OLLAMA_MODEL="codellama"

# Remove legacy variables to prevent pollution
unset OPENAI_API_KEY OPENAI_API_BASE OPENAI_MODEL
unset ANTHROPIC_API_KEY ANTHROPIC_API_BASE ANTHROPIC_MODEL
unset OLLAMA_ENDPOINT OLLAMA_MODEL
EOF

if [[ "$legacy_found" == true ]]; then
    echo
    echo -e "${YELLOW}=== Migration Commands ===${NC}"
    echo "Run these commands to migrate your current configuration:"
    echo
    echo -e "${migration_commands}"
fi

# Generate secure configuration file
echo
echo -e "${BLUE}=== Configuration File Update ===${NC}"
echo "Update your ~/.config/openagent-terminal/openagent-terminal.toml:"
echo

cat << 'EOF'
[ai]
enabled = true
provider = "openai"  # or "anthropic", "ollama"

# Provider-specific configurations
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

[ai.providers.ollama]
endpoint_env = "OPENAGENT_OLLAMA_ENDPOINT"
model_env = "OPENAGENT_OLLAMA_MODEL"
default_endpoint = "http://localhost:11434"
default_model = "codellama"
EOF

echo
echo -e "${GREEN}=== Security Benefits ===${NC}"
echo "✓ No credential leakage between providers"
echo "✓ Clear separation of provider configurations"
echo "✓ Support for multiple providers simultaneously"
echo "✓ No global environment variable mutation"
echo "✓ Auditability and traceability of credential usage"
echo

echo -e "${BLUE}=== Next Steps ===${NC}"
echo "1. Update your shell profile with the secure environment variables"
echo "2. Update your OpenAgent Terminal configuration file"
echo "3. Restart your shell or run 'source ~/.bashrc' (or equivalent)"
echo "4. Test your AI provider functionality"
echo
echo "For more information, see: docs/AI_ENVIRONMENT_SECURITY.md"
echo

if [[ "$legacy_found" == true ]]; then
    echo -e "${YELLOW}IMPORTANT: Don't forget to remove the legacy environment variables!${NC}"
    echo "Keeping both legacy and secure variables active can still cause issues."
    echo
fi

echo -e "${GREEN}Migration guide complete!${NC}"
