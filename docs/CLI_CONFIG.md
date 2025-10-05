# CLI and Configuration Guide

## Overview

OpenAgent-Terminal provides flexible configuration through a combination of CLI arguments, environment variables, and configuration files, with clear precedence rules for predictable behavior.

## Configuration Precedence

The application follows this precedence order (highest to lowest):

1. **CLI Arguments** (highest priority)
2. **Environment Variables**  
3. **Configuration File**
4. **Built-in Defaults** (lowest priority)

This ensures you can override any setting temporarily via CLI while maintaining persistent configuration in files.

## Command-Line Interface

### Basic Usage

```bash
# Run with defaults
openagent-terminal

# Specify custom socket
openagent-terminal --socket /tmp/my-socket.sock

# Use custom config file
openagent-terminal --config /path/to/config.toml

# Override model
openagent-terminal --model gpt-4

# Enable verbose logging
openagent-terminal --verbose

# Combine options
openagent-terminal --socket /tmp/socket.sock --model claude-3-opus --verbose
```

### Available Options

#### `-s, --socket <PATH>`
Path to Unix socket for IPC with Python backend.

**Precedence:** CLI > `OPENAGENT_SOCKET` env > Default  
**Default:** `$XDG_RUNTIME_DIR/openagent-terminal-test.sock`

**Examples:**
```bash
openagent-terminal --socket /tmp/custom.sock
openagent-terminal -s ~/.openagent/socket.sock
```

#### `-c, --config <PATH>`
Path to configuration file.

**Default:** `$XDG_CONFIG_HOME/openagent-terminal/config.toml`

**Examples:**
```bash
openagent-terminal --config /etc/openagent/config.toml
openagent-terminal -c ~/my-config.toml
```

#### `-l, --log-level <LEVEL>`
Set logging verbosity.

**Levels:** `trace`, `debug`, `info`, `warn`, `error`, `off`  
**Default:** `info`

**Examples:**
```bash
openagent-terminal --log-level debug
openagent-terminal -l trace
```

#### `-m, --model <MODEL>`
Override AI model from config file.

**Examples:** `mock`, `gpt-4`, `claude-3-opus`, `claude-3-sonnet`

**Examples:**
```bash
openagent-terminal --model gpt-4
openagent-terminal -m claude-3-opus
```

#### `-v, --verbose`
Enable verbose output (equivalent to `--log-level debug`).

**Examples:**
```bash
openagent-terminal --verbose
openagent-terminal -v
```

#### `-q, --quiet`
Suppress all output except errors (equivalent to `--log-level error`).

**Examples:**
```bash
openagent-terminal --quiet
openagent-terminal -q
```

#### `--generate-config`
Generate default configuration file and exit.

Creates `config.toml` at standard location with default settings. Prompts for confirmation if file already exists.

**Examples:**
```bash
openagent-terminal --generate-config
```

### Help and Version

```bash
# Show help
openagent-terminal --help
openagent-terminal -h

# Show version
openagent-terminal --version
openagent-terminal -V
```

## Environment Variables

### `OPENAGENT_SOCKET`
Socket path for backend connection.

**Precedence:** CLI `--socket` > This variable > Default

**Examples:**
```bash
export OPENAGENT_SOCKET=/tmp/my-socket.sock
openagent-terminal

# Temporarily override
OPENAGENT_SOCKET=/tmp/other.sock openagent-terminal
```

### `XDG_RUNTIME_DIR`
Used for default socket path.

**Default:** `/tmp` (if not set)

### `XDG_CONFIG_HOME`
Used for default config file location.

**Default:** `~/.config` (if not set)

## Configuration File

### Location

**Default path:**
```
Linux:   ~/.config/openagent-terminal/config.toml
macOS:   ~/Library/Application Support/openagent-terminal/config.toml
Windows: %APPDATA%\openagent-terminal\config.toml
```

### Generate Default Config

```bash
openagent-terminal --generate-config
```

This creates a config file with all available settings and their defaults.

### File Format

The configuration file uses TOML format:

```toml
[terminal]
font_family = "DejaVu Sans Mono"
font_size = 14
theme = "monokai"
scrollback_lines = 10000
syntax_highlighting = true

[agent]
model = "mock"
auto_suggest = true
require_approval = true
max_tokens = 2000
temperature = 0.7

[keybindings]
toggle_ai = "Ctrl+A"
send_query = "Enter"
cancel = "Ctrl+C"
clear_screen = "Ctrl+K"
show_history = "Ctrl+L"

[tools]
enable_real_execution = false
safe_directories = ["~", "."]
command_timeout = 10
```

### Sections

#### `[terminal]`
Terminal display and rendering settings.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `font_family` | string | "DejaVu Sans Mono" | Font family name |
| `font_size` | integer | 14 | Font size in points |
| `theme` | string | "monokai" | Color theme |
| `scrollback_lines` | integer | 10000 | Scrollback buffer size |
| `syntax_highlighting` | boolean | true | Enable syntax highlighting |

#### `[agent]`
AI agent configuration.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `model` | string | "mock" | AI model to use |
| `auto_suggest` | boolean | true | Auto command suggestions |
| `require_approval` | boolean | true | Require approval for tools |
| `max_tokens` | integer | 2000 | Max tokens per query |
| `temperature` | float | 0.7 | LLM sampling temperature |

#### `[keybindings]`
Keyboard shortcuts.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `toggle_ai` | string | "Ctrl+A" | Toggle AI pane |
| `send_query` | string | "Enter" | Send query |
| `cancel` | string | "Ctrl+C" | Cancel operation |
| `clear_screen` | string | "Ctrl+K" | Clear screen |
| `show_history` | string | "Ctrl+L" | Show command history |

#### `[tools]`
Tool execution settings.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `enable_real_execution` | boolean | false | Enable real file operations |
| `safe_directories` | array | ["~", "."] | Allowed directories |
| `command_timeout` | integer | 10 | Command timeout in seconds |

## Precedence Examples

### Example 1: Socket Path

**Scenario:** Multiple socket path sources

```bash
# Config file: socket = "/etc/socket.sock"
# Environment: OPENAGENT_SOCKET=/tmp/env.sock
# CLI: --socket /tmp/cli.sock

openagent-terminal --socket /tmp/cli.sock
# Uses: /tmp/cli.sock (CLI wins)

export OPENAGENT_SOCKET=/tmp/env.sock
openagent-terminal
# Uses: /tmp/env.sock (env wins over file)

unset OPENAGENT_SOCKET
openagent-terminal
# Uses: $XDG_RUNTIME_DIR/openagent-terminal-test.sock (default)
```

### Example 2: Model Selection

**Config file:**
```toml
[agent]
model = "mock"
```

**CLI override:**
```bash
openagent-terminal --model gpt-4
# Uses: gpt-4 (CLI wins)

openagent-terminal
# Uses: mock (from config file)
```

### Example 3: Log Levels

**Priority:** `--quiet` > `--verbose` > `--log-level` > Default

```bash
# Quiet takes precedence
openagent-terminal --verbose --quiet
# Uses: error level (quiet wins)

# Verbose
openagent-terminal --verbose
# Uses: debug level

# Explicit level
openagent-terminal --log-level trace
# Uses: trace level

# Default
openagent-terminal
# Uses: info level
```

## Common Use Cases

### Development Setup

```bash
# Generate config
openagent-terminal --generate-config

# Edit config
$EDITOR ~/.config/openagent-terminal/config.toml

# Run with debug logging
openagent-terminal --verbose
```

### Custom Socket for Testing

```bash
# Start backend with custom socket
cd backend
python -m openagent_terminal.bridge --socket /tmp/test.sock

# Connect frontend
openagent-terminal --socket /tmp/test.sock
```

### Different Environments

```bash
# Development
openagent-terminal --config ~/dev-config.toml --model mock --verbose

# Staging
openagent-terminal --config ~/staging-config.toml --model gpt-4

# Production
openagent-terminal --config /etc/openagent/prod.toml
```

### CI/CD Integration

```bash
# Run in quiet mode with specific config
openagent-terminal \
    --quiet \
    --config /ci/config.toml \
    --socket /tmp/ci-socket.sock
```

## Configuration Best Practices

### 1. Use Configuration File for Persistent Settings

Store preferences that rarely change in the config file:
```toml
[terminal]
font_family = "Fira Code"
font_size = 12
theme = "dracula"
```

### 2. Use Environment Variables for Environment-Specific Settings

```bash
# In .bashrc or .zshrc
export OPENAGENT_SOCKET=/home/user/.openagent/socket.sock
```

### 3. Use CLI Arguments for One-Off Overrides

```bash
# Temporarily use different model
openagent-terminal --model gpt-4

# Debug a specific session
openagent-terminal --verbose
```

### 4. Version Control Your Config

```bash
# Keep config in dotfiles repo
git add ~/.config/openagent-terminal/config.toml
git commit -m "Update openagent config"
```

### 5. Use Multiple Configs for Different Profiles

```bash
# Work profile
openagent-terminal --config ~/work-config.toml

# Personal profile  
openagent-terminal --config ~/personal-config.toml
```

## Troubleshooting

### Config File Not Found

```bash
# Generate default config
openagent-terminal --generate-config

# Verify location
ls -la ~/.config/openagent-terminal/config.toml
```

### Socket Connection Failed

```bash
# Check socket path
openagent-terminal --verbose 2>&1 | grep "Socket path"

# Verify backend is running
ps aux | grep openagent

# Try custom socket
openagent-terminal --socket /tmp/custom.sock
```

### Log Level Not Working

```bash
# Ensure no conflicting flags
openagent-terminal --log-level debug  # Good
openagent-terminal --quiet --log-level debug  # quiet wins
```

### Model Override Not Applied

```bash
# Check logs
openagent-terminal --model gpt-4 --verbose 2>&1 | grep "model"
# Should show: CLI override: model = gpt-4
```

## Advanced Usage

### Custom Config in Non-Standard Location

```bash
openagent-terminal --config /opt/configs/openagent.toml
```

### Multiple Instances with Different Sockets

```bash
# Terminal 1
openagent-terminal --socket /tmp/instance1.sock

# Terminal 2
openagent-terminal --socket /tmp/instance2.sock
```

### Scripted Usage

```bash
#!/bin/bash
# launch-openagent.sh

SOCKET_PATH="/tmp/openagent-$$.sock"
LOG_FILE="/tmp/openagent-$$.log"

# Start backend
python -m openagent_terminal.bridge --socket "$SOCKET_PATH" &
BACKEND_PID=$!

# Wait for socket
while [ ! -S "$SOCKET_PATH" ]; do sleep 0.1; done

# Start frontend
openagent-terminal \
    --socket "$SOCKET_PATH" \
    --log-level debug \
    2>&1 | tee "$LOG_FILE"

# Cleanup
kill $BACKEND_PID
rm -f "$SOCKET_PATH"
```

## Summary

OpenAgent-Terminal's configuration system provides:

✅ **Flexible Configuration** - Multiple ways to configure  
✅ **Clear Precedence** - CLI > Env > File > Default  
✅ **Easy Setup** - `--generate-config` for quick start  
✅ **Override Friendly** - CLI args for temporary changes  
✅ **Well Documented** - `--help` for all options

The precedence system ensures predictable behavior while supporting various use cases from development to production deployment.
