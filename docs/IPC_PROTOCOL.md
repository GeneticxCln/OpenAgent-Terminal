# OpenAgent-Terminal IPC Protocol Specification

**Version:** 1.0.0  
**Protocol:** JSON-RPC 2.0 over Unix Domain Socket  
**Status:** Draft

## Overview

This document specifies the complete IPC protocol for communication between the Rust frontend (terminal UI) and Python backend (OpenAgent intelligence).

## Transport Layer

### Connection
- **Type:** Unix Domain Socket
- **Path:** `$XDG_RUNTIME_DIR/openagent-terminal-{pid}.sock`
  - Falls back to `/tmp/openagent-terminal-{pid}.sock` if `$XDG_RUNTIME_DIR` not set
- **Permissions:** 0600 (owner read/write only)
- **Encoding:** UTF-8
- **Framing:** Newline-delimited JSON (one message per line)

### Lifecycle
1. Python backend starts and creates socket
2. Rust frontend connects to socket
3. Handshake via `initialize` method
4. Bidirectional message exchange
5. Clean shutdown with socket removal

## Message Format

All messages follow JSON-RPC 2.0 specification.

### Request (Client → Server)
```json
{
  "jsonrpc": "2.0",
  "id": <number|string>,
  "method": "<method_name>",
  "params": <object|array>
}
```

### Response (Server → Client)
```json
{
  "jsonrpc": "2.0",
  "id": <number|string>,
  "result": <any>
}
```

### Error Response
```json
{
  "jsonrpc": "2.0",
  "id": <number|string>,
  "error": {
    "code": <number>,
    "message": "<string>",
    "data": <any>
  }
}
```

### Notification (No response expected)
```json
{
  "jsonrpc": "2.0",
  "method": "<method_name>",
  "params": <object|array>
}
```

## Error Codes

Standard JSON-RPC error codes plus custom codes:

| Code | Message | Description |
|------|---------|-------------|
| -32700 | Parse error | Invalid JSON |
| -32600 | Invalid Request | Invalid JSON-RPC |
| -32601 | Method not found | Method doesn't exist |
| -32602 | Invalid params | Invalid method parameters |
| -32603 | Internal error | Internal server error |
| -32000 | Agent error | Agent processing failed |
| -32001 | Model error | LLM inference failed |
| -32002 | Tool error | Tool execution failed |
| -32003 | Timeout | Operation timed out |
| -32004 | Cancelled | Operation was cancelled |
| -32005 | Permission denied | Operation not permitted |

## Methods

### 1. initialize

**Direction:** Client → Server  
**Type:** Request  
**Description:** Initialize the connection and exchange capabilities

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocol_version": "1.0.0",
    "client_info": {
      "name": "openagent-terminal",
      "version": "0.1.0"
    },
    "terminal_size": {
      "cols": 80,
      "rows": 24,
      "pixel_width": 800,
      "pixel_height": 600
    },
    "capabilities": [
      "streaming",
      "blocks",
      "syntax_highlighting",
      "tool_approval",
      "session_persistence"
    ],
    "config": {
      "theme": "monokai",
      "font_size": 14
    }
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "status": "ready",
    "server_info": {
      "name": "openagent",
      "version": "0.1.3"
    },
    "capabilities": [
      "streaming",
      "blocks",
      "tool_execution",
      "context_aware"
    ],
    "models": [
      {
        "name": "codellama-7b",
        "type": "code",
        "loaded": true
      },
      {
        "name": "mistral-7b",
        "type": "chat",
        "loaded": false
      }
    ]
  }
}
```

### 2. agent.query

**Direction:** Client → Server  
**Type:** Request  
**Description:** Send a query to the AI agent

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "agent.query",
  "params": {
    "message": "How do I optimize this Rust code?",
    "context": {
      "cwd": "/home/user/project",
      "shell": "bash",
      "env": {
        "USER": "user",
        "PWD": "/home/user/project"
      },
      "recent_commands": [
        {
          "command": "cargo build",
          "exit_code": 0,
          "timestamp": 1696435200
        },
        {
          "command": "cargo test",
          "exit_code": 1,
          "timestamp": 1696435210
        }
      ],
      "recent_output": "thread 'main' panicked at...",
      "open_files": [
        "src/main.rs",
        "Cargo.toml"
      ]
    },
    "options": {
      "stream": true,
      "max_tokens": 2048,
      "temperature": 0.7,
      "tools_enabled": true,
      "require_approval": true
    }
  }
}
```

**Response (immediate):**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "query_id": "q-123abc",
    "status": "processing"
  }
}
```

**Followed by streaming notifications (see stream.* methods below)**

### 3. agent.cancel

**Direction:** Client → Server  
**Type:** Request  
**Description:** Cancel an in-progress query

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "agent.cancel",
  "params": {
    "query_id": "q-123abc"
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "cancelled": true,
    "query_id": "q-123abc"
  }
}
```

### 4. tool.approve

**Direction:** Client → Server  
**Type:** Request  
**Description:** Approve or reject a tool execution

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tool.approve",
  "params": {
    "execution_id": "exec-456def",
    "approved": true,
    "options": {
      "remember_choice": false,
      "apply_to_similar": false
    }
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "status": "approved",
    "execution_id": "exec-456def"
  }
}
```

### 5. context.update

**Direction:** Client → Server  
**Type:** Notification  
**Description:** Update terminal context (CWD, env vars, etc.)

**Notification:**
```json
{
  "jsonrpc": "2.0",
  "method": "context.update",
  "params": {
    "cwd": "/home/user/new-project",
    "env": {
      "PWD": "/home/user/new-project"
    },
    "terminal_size": {
      "cols": 100,
      "rows": 30
    }
  }
}
```

### 6. session.save

**Direction:** Client → Server  
**Type:** Request  
**Description:** Save current session state

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "session.save",
  "params": {
    "path": "/home/user/.openagent/sessions/session-123.json",
    "include_history": true,
    "include_blocks": true
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "saved": true,
    "path": "/home/user/.openagent/sessions/session-123.json",
    "size": 45632
  }
}
```

### 7. session.load

**Direction:** Client → Server  
**Type:** Request  
**Description:** Load a saved session

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "session.load",
  "params": {
    "path": "/home/user/.openagent/sessions/session-123.json"
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "result": {
    "loaded": true,
    "session": {
      "id": "session-123",
      "created": 1696435000,
      "blocks_count": 42,
      "messages_count": 15
    }
  }
}
```

## Notifications (Server → Client)

### 1. stream.token

**Description:** Stream individual tokens as they're generated

**Notification:**
```json
{
  "jsonrpc": "2.0",
  "method": "stream.token",
  "params": {
    "query_id": "q-123abc",
    "token": "To optimize",
    "index": 0,
    "metadata": {
      "type": "text",
      "confidence": 0.95
    }
  }
}
```

### 2. stream.block

**Description:** Stream a complete block (code, diff, etc.)

**Notification:**
```json
{
  "jsonrpc": "2.0",
  "method": "stream.block",
  "params": {
    "query_id": "q-123abc",
    "block_id": "block-789",
    "block": {
      "type": "code",
      "language": "rust",
      "content": "fn optimized() -> Result<()> {\n    // More efficient implementation\n    Ok(())\n}",
      "metadata": {
        "file": "src/main.rs",
        "lines": "10-15",
        "diff": false,
        "executable": true
      },
      "annotations": [
        {
          "line": 1,
          "type": "suggestion",
          "message": "This uses iterator adaptors"
        }
      ]
    }
  }
}
```

**Block Types:**
- `text` - Plain text
- `code` - Syntax-highlighted code
- `diff` - Code diff with changes
- `error` - Error message
- `tool_output` - Tool execution output
- `interactive` - Requires user interaction

### 3. stream.complete

**Description:** Signal that streaming is complete

**Notification:**
```json
{
  "jsonrpc": "2.0",
  "method": "stream.complete",
  "params": {
    "query_id": "q-123abc",
    "status": "success",
    "metadata": {
      "total_tokens": 150,
      "tokens_per_second": 25.5,
      "tools_executed": ["file_read", "code_analyze"],
      "model_used": "codellama-7b",
      "duration_ms": 5880
    }
  }
}
```

**Status values:**
- `success` - Completed successfully
- `error` - Failed with error
- `cancelled` - Cancelled by user
- `timeout` - Timed out

### 4. stream.error

**Description:** Signal an error during streaming

**Notification:**
```json
{
  "jsonrpc": "2.0",
  "method": "stream.error",
  "params": {
    "query_id": "q-123abc",
    "error": {
      "code": -32000,
      "message": "Agent processing failed",
      "details": "Model inference failed: CUDA out of memory"
    }
  }
}
```

### 5. tool.request_approval

**Description:** Request user approval for tool execution

**Notification:**
```json
{
  "jsonrpc": "2.0",
  "method": "tool.request_approval",
  "params": {
    "execution_id": "exec-456def",
    "query_id": "q-123abc",
    "tool": {
      "name": "file_write",
      "description": "Write optimized code to src/main.rs",
      "category": "file_system"
    },
    "arguments": {
      "path": "src/main.rs",
      "content": "fn optimized() { ... }"
    },
    "preview": {
      "type": "diff",
      "content": "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -10,5 +10,8 @@\n-fn old() {}\n+fn optimized() { ... }"
    },
    "risk_assessment": {
      "level": "medium",
      "reasons": [
        "Modifies existing file",
        "File is in version control"
      ],
      "reversible": true
    },
    "timeout": 30
  }
}
```

**Risk Levels:**
- `low` - Safe operation (read-only, local)
- `medium` - Modifies files, reversible
- `high` - System changes, network access
- `critical` - Irreversible or dangerous

### 6. tool.progress

**Description:** Update on tool execution progress

**Notification:**
```json
{
  "jsonrpc": "2.0",
  "method": "tool.progress",
  "params": {
    "execution_id": "exec-456def",
    "progress": {
      "percentage": 45,
      "status": "processing",
      "message": "Analyzing 15 files..."
    }
  }
}
```

### 7. tool.complete

**Description:** Tool execution completed

**Notification:**
```json
{
  "jsonrpc": "2.0",
  "method": "tool.complete",
  "params": {
    "execution_id": "exec-456def",
    "status": "success",
    "result": {
      "output": "File written successfully",
      "files_modified": ["src/main.rs"],
      "duration_ms": 125
    }
  }
}
```

### 8. agent.thinking

**Description:** Agent is thinking/planning (show loading indicator)

**Notification:**
```json
{
  "jsonrpc": "2.0",
  "method": "agent.thinking",
  "params": {
    "query_id": "q-123abc",
    "phase": "analyzing",
    "message": "Analyzing your code structure..."
  }
}
```

**Phases:**
- `analyzing` - Understanding the query
- `planning` - Planning tool usage
- `generating` - Generating response
- `executing` - Executing tools

### 9. suggestion.inline

**Description:** Inline command suggestion

**Notification:**
```json
{
  "jsonrpc": "2.0",
  "method": "suggestion.inline",
  "params": {
    "suggestion": "cargo test -- --nocapture",
    "confidence": 0.87,
    "source": "command_history",
    "context": "User previously used this with similar error"
  }
}
```

## Message Flow Examples

### Example 1: Simple Query

```
Client → Server: initialize
Client ← Server: {status: "ready"}

Client → Server: agent.query "explain ls command"
Client ← Server: {query_id: "q-1", status: "processing"}

Client ← Server: stream.token "The ls"
Client ← Server: stream.token " command"
Client ← Server: stream.token " lists files..."
Client ← Server: stream.complete {status: "success"}
```

### Example 2: Query with Tool Execution

```
Client → Server: agent.query "optimize my code"
Client ← Server: {query_id: "q-2", status: "processing"}

Client ← Server: agent.thinking {phase: "analyzing"}
Client ← Server: tool.request_approval {tool: "file_read"}

Client → Server: tool.approve {approved: true}

Client ← Server: tool.progress {percentage: 50}
Client ← Server: tool.complete {status: "success"}

Client ← Server: stream.block {type: "code", ...}
Client ← Server: stream.complete {status: "success"}
```

### Example 3: Cancellation

```
Client → Server: agent.query "long running query"
Client ← Server: {query_id: "q-3"}

Client ← Server: stream.token "Processing..."

Client → Server: agent.cancel {query_id: "q-3"}
Client ← Server: {cancelled: true}

Client ← Server: stream.complete {status: "cancelled"}
```

## Performance Guidelines

### Latency Targets
- **Handshake:** < 50ms
- **Query submission:** < 10ms
- **Token streaming:** < 50ms per token
- **Tool approval request:** < 100ms

### Throughput
- **Max message size:** 10MB
- **Max tokens/second:** 1000
- **Max concurrent queries:** 3

### Batching
For high-frequency updates (e.g., token streaming), the server MAY batch multiple tokens into a single message:

```json
{
  "jsonrpc": "2.0",
  "method": "stream.tokens",
  "params": {
    "query_id": "q-123",
    "tokens": [
      {"token": "Hello", "index": 0},
      {"token": " ", "index": 1},
      {"token": "world", "index": 2}
    ]
  }
}
```

## Security Considerations

### Authentication
- Socket permissions (0600) provide process-level authentication
- Additional token-based auth can be added if needed

### Message Validation
- All messages must be valid JSON-RPC 2.0
- Parameter types must match specification
- Maximum message size enforced (10MB)

### Rate Limiting
- Max 100 requests/second per connection
- Max 3 concurrent queries
- Tool execution requires explicit approval

## Versioning

Protocol version follows semantic versioning:
- **Major:** Breaking changes
- **Minor:** New features, backward compatible
- **Patch:** Bug fixes

Clients and servers MUST negotiate protocol version during `initialize`.

## Future Extensions

Planned additions:
- Binary attachments (for large files)
- Voice input/output events
- Collaborative features (multiple users)
- Remote agent connections
- WebSocket transport option

---

**Revision History:**
- v1.0.0 (2025-10-04): Initial specification
