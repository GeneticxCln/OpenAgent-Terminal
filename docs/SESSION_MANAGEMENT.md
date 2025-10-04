# Session Management Guide

## Overview

OpenAgent-Terminal now includes full session persistence, allowing you to:
- Save conversation history automatically
- Resume previous sessions
- Export sessions to markdown
- Manage multiple concurrent projects
- Track token usage and message counts

## Quick Start

### Starting the Application

```bash
# Terminal 1: Start Python backend
cd backend
python -m openagent_terminal.bridge

# Terminal 2: Start Rust frontend
cargo run --release
```

### Your First Session

When you start the application, you'll see a prompt:
```
>
```

Just type your question:
```
> Help me write a Python function to parse JSON
```

The AI will respond, and your conversation is automatically saved to a new session.

## Session Commands

All session commands start with `/` to distinguish them from regular queries.

### List Sessions

Show all your saved sessions:
```
/list
```

Show only the 10 most recent sessions:
```
/list 10
```

**Example Output:**
```
╔═══════════════════════════════════════════════════════════════════╗
║                        Session History                           ║
╚═══════════════════════════════════════════════════════════════════╝

1. a1b2c3d4 Session about Python JSON parsing
   Created: 2025-10-04 10:30  Messages: 12  Tokens: 2,450

2. e5f6g7h8 Docker containerization help
   Created: 2025-10-03 15:22  Messages: 8   Tokens: 1,820

3. i9j0k1l2 Rust async programming
   Created: 2025-10-03 09:15  Messages: 15  Tokens: 3,200

Tip: Use /load <session-id> to continue a previous session
```

### Load a Session

Resume a previous conversation:
```
/load a1b2c3d4
```

When you load a session:
- The conversation history is restored
- The prompt shows the session ID: `[a1b2c3d4]>`
- All new messages are added to this session
- The backend has full context of previous messages

### Export a Session

Export the current session to markdown:
```
/export
```

Export a specific session:
```
/export a1b2c3d4
```

Export to a file:
```
/export --output=session.md
```

Export as JSON (future feature):
```
/export --format=json --output=session.json
```

**Example Markdown Export:**
```markdown
# Session: a1b2c3d4

**Created:** 2025-10-04 10:30:45 UTC
**Messages:** 12
**Total Tokens:** 2,450

---

## Message 1 (User)
*2025-10-04 10:30:45*

Help me write a Python function to parse JSON

---

## Message 2 (Assistant)
*2025-10-04 10:30:52*

I'll help you create a Python function to parse JSON...
```

### Delete a Session

Permanently delete a session:
```
/delete a1b2c3d4
```

⚠️ **Warning:** This action cannot be undone!

### View Current Session Info

See information about your active session:
```
/info
```

**Example Output:**
```
╔═══════════════════════════════════════════════════════════════════╗
║                      Current Session Info                        ║
╚═══════════════════════════════════════════════════════════════════╝

Session ID: a1b2c3d4e5f6g7h8
Title: Session about Python JSON parsing
Created: 2025-10-04 10:30:45
Updated: 2025-10-04 11:15:22
Messages: 12
Total Tokens: 2,450
```

### Get Help

Display the help menu:
```
/help
```

### Exit the Application

```
/exit
```

You can also use:
- `/quit`
- `/q`
- `Ctrl+D` (EOF)

## Command Aliases

Some commands have shorter aliases:

| Command | Aliases |
|---------|---------|
| `/list` | `/ls` |
| `/delete` | `/rm` |
| `/info` | `/current` |
| `/help` | `/?` |
| `/exit` | `/quit`, `/q` |

## Session Storage

Sessions are stored in your home directory:
```
~/.config/openagent-terminal/sessions/
```

Each session is saved as a JSON file:
```
sessions/
  ├── a1b2c3d4-e5f6-g7h8-i9j0-k1l2m3n4o5p6.json
  ├── e5f6g7h8-i9j0-k1l2-m3n4-o5p6q7r8s9t0.json
  └── ...
```

### Session File Format

```json
{
  "session_id": "a1b2c3d4-e5f6-g7h8-i9j0-k1l2m3n4o5p6",
  "created_at": "2025-10-04T10:30:45Z",
  "updated_at": "2025-10-04T11:15:22Z",
  "messages": [
    {
      "role": "user",
      "content": "Help me write a Python function to parse JSON",
      "timestamp": "2025-10-04T10:30:45Z",
      "token_count": 9
    },
    {
      "role": "assistant",
      "content": "I'll help you create a Python function...",
      "timestamp": "2025-10-04T10:30:52Z",
      "token_count": 245
    }
  ]
}
```

## Best Practices

### 1. **Organize by Project**
Create separate sessions for different projects or topics.

### 2. **Use Descriptive Queries**
Your first message in a session often becomes the title, so make it descriptive:
```
> Setup CI/CD pipeline for Node.js project with GitHub Actions
```

### 3. **Regular Exports**
Export important sessions to markdown for documentation:
```
/export --output=project-docs/ci-cd-setup.md
```

### 4. **Clean Up Old Sessions**
Periodically delete sessions you no longer need:
```
/list
/delete old-session-id
```

### 5. **Session Limits**
By default, sessions auto-save every message. For very long conversations:
- Consider exporting and starting a new session after ~50 messages
- Token limits may apply based on your AI model

## Troubleshooting

### "No sessions found"
- You haven't created any sessions yet
- Sessions directory may not exist yet
- Start a conversation to create your first session

### "Failed to load session"
- Check that the session ID is correct (use `/list` to see available IDs)
- The session file may be corrupted
- Check `~/.config/openagent-terminal/sessions/` exists and is readable

### "Session not saved"
- Ensure the backend is running
- Check write permissions on `~/.config/openagent-terminal/sessions/`
- Review backend logs for errors

### Sessions Not Persisting
- Make sure both backend and frontend are running
- Check that the session directory exists
- Verify the IPC connection is working (you should see "✅ Connected to Python backend")

## Advanced Usage

### Session Auto-Save
Sessions auto-save after every message exchange. No manual save needed!

### Session Context
When you load a session, the AI has access to the full conversation history, providing better context for follow-up questions.

### Concurrent Sessions
You can have multiple terminal windows open with different sessions. Each window maintains its own session state.

### Session Metadata
The system tracks:
- Message count
- Total token usage
- Creation and update timestamps
- Automatic title generation from first message

## API Reference (for Developers)

### IPC Messages

List sessions:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "session.list",
  "params": { "limit": 10 }
}
```

Load session:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "session.load",
  "params": { "session_id": "abc123..." }
}
```

Export session:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "session.export",
  "params": {
    "session_id": "abc123...",
    "format": "markdown"
  }
}
```

Delete session:
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "session.delete",
  "params": { "session_id": "abc123..." }
}
```

## Future Enhancements

Planned features:
- [ ] Session search by content
- [ ] Session tags and categories
- [ ] Session sharing and collaboration
- [ ] Cloud sync for sessions
- [ ] Session templates
- [ ] Advanced export formats (PDF, HTML)
- [ ] Session branching (fork conversations)
- [ ] Session merge capabilities

## Feedback

Found a bug or have a feature request? Please open an issue on GitHub:
https://github.com/GeneticxCln/openagent-terminal/issues
