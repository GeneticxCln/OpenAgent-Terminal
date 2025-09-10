#!/usr/bin/env bash
# Print ready-to-use environment snippets for configuring Ollama server performance
# and OpenAgent Terminal's Ollama client variables.
# Usage:
#   scripts/ollama_env_snippet.sh                  # print to stdout
#   scripts/ollama_env_snippet.sh --systemd > /tmp/ollama-override.conf
#   scripts/ollama_env_snippet.sh --shell > ~/.config/openagent-terminal/ollama.env

set -euo pipefail

mode="all"
if [[ ${1:-} == "--systemd" ]]; then
  mode="systemd"
elif [[ ${1:-} == "--shell" ]]; then
  mode="shell"
fi

if [[ "$mode" == "all" || "$mode" == "shell" ]]; then
  cat << 'EOS'
# --- Ollama server performance (shell) ---
# Set these in the same shell that runs 'ollama serve' or in your shell profile.
export OLLAMA_FLASH_ATTENTION=true
export OLLAMA_KV_CACHE_TYPE=f16

# --- OpenAgent Terminal Ollama client ---
# Point the client to the local server and choose a quantized model tag.
export OPENAGENT_OLLAMA_ENDPOINT="http://localhost:11434"
export OPENAGENT_OLLAMA_MODEL="llama3.1:8b-instruct-q4_K_M"  # adjust as needed
EOS
fi

if [[ "$mode" == "all" || "$mode" == "systemd" ]]; then
  cat << 'EOS'
# --- systemd override snippet (put under /etc/systemd/system/ollama.service.d/override.conf) ---
[Service]
Environment="OLLAMA_FLASH_ATTENTION=true"
Environment="OLLAMA_KV_CACHE_TYPE=f16"
# After saving:
#   sudo systemctl daemon-reload
#   sudo systemctl restart ollama
EOS
fi

