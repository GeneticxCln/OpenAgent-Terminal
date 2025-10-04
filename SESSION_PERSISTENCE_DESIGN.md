# Session Persistence Design

**Feature:** Session Persistence & History Management  
**Phase:** 5 Week 3  
**Priority:** High  
**Estimated Time:** 12 hours

---

## Overview

Session persistence allows users to save and restore conversation history across terminal sessions. This enables:
- Continuing previous conversations
- Reviewing past interactions
- Exporting conversations to markdown
- Analyzing token usage over time
- Building context across sessions

---

## Architecture

### Components

```
┌─────────────────────────────────────────────────────────────┐
│                      User Interface (TUI)                    │
│  - Display session list                                      │
│  - Show "Restored session" indicator                         │
│  - Session switching UI                                      │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       │ IPC Messages
                       │
┌──────────────────────▼──────────────────────────────────────┐
│                   Rust Frontend (main.rs)                    │
│  - Session state tracking                                    │
│  - Current session ID                                        │
│  - Session metadata cache                                    │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       │ IPC: session_save, session_load, etc.
                       │
┌──────────────────────▼──────────────────────────────────────┐
│                Python Backend (bridge.py)                    │
│  - Session manager                                           │
│  - Auto-save on message exchange                             │
│  - Session restoration                                       │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       │
┌──────────────────────▼──────────────────────────────────────┐
│                  Session Storage Layer                       │
│  - File system operations                                    │
│  - JSON serialization                                        │
│  - Session indexing                                          │
└─────────────────────────────────────────────────────────────┘
                       │
                       ▼
            ~/.config/openagent-terminal/
                sessions/
                    index.json
                    2025-10-04_114203.json
                    2025-10-04_153045.json
```

---

## Data Structures

### 1. Session Model (Python)

```python
from dataclasses import dataclass, field
from datetime import datetime
from typing import List, Optional, Dict, Any
from enum import Enum

class MessageRole(Enum):
    """Message role in conversation"""
    USER = "user"
    ASSISTANT = "assistant"
    SYSTEM = "system"
    TOOL = "tool"

@dataclass
class Message:
    """Single message in conversation"""
    role: MessageRole
    content: str
    timestamp: datetime
    token_count: Optional[int] = None
    tool_calls: Optional[List[Dict[str, Any]]] = None
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        """Serialize to dictionary"""
        return {
            "role": self.role.value,
            "content": self.content,
            "timestamp": self.timestamp.isoformat(),
            "token_count": self.token_count,
            "tool_calls": self.tool_calls,
            "metadata": self.metadata
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "Message":
        """Deserialize from dictionary"""
        return cls(
            role=MessageRole(data["role"]),
            content=data["content"],
            timestamp=datetime.fromisoformat(data["timestamp"]),
            token_count=data.get("token_count"),
            tool_calls=data.get("tool_calls"),
            metadata=data.get("metadata", {})
        )

@dataclass
class SessionMetadata:
    """Session metadata"""
    session_id: str
    created_at: datetime
    updated_at: datetime
    message_count: int
    total_tokens: int
    title: Optional[str] = None
    tags: List[str] = field(default_factory=list)
    
    def to_dict(self) -> Dict[str, Any]:
        """Serialize to dictionary"""
        return {
            "session_id": self.session_id,
            "created_at": self.created_at.isoformat(),
            "updated_at": self.updated_at.isoformat(),
            "message_count": self.message_count,
            "total_tokens": self.total_tokens,
            "title": self.title,
            "tags": self.tags
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "SessionMetadata":
        """Deserialize from dictionary"""
        return cls(
            session_id=data["session_id"],
            created_at=datetime.fromisoformat(data["created_at"]),
            updated_at=datetime.fromisoformat(data["updated_at"]),
            message_count=data["message_count"],
            total_tokens=data["total_tokens"],
            title=data.get("title"),
            tags=data.get("tags", [])
        )

@dataclass
class Session:
    """Complete session with messages and metadata"""
    metadata: SessionMetadata
    messages: List[Message] = field(default_factory=list)
    
    def add_message(self, message: Message):
        """Add message and update metadata"""
        self.messages.append(message)
        self.metadata.message_count = len(self.messages)
        self.metadata.updated_at = datetime.now()
        if message.token_count:
            self.metadata.total_tokens += message.token_count
    
    def to_dict(self) -> Dict[str, Any]:
        """Serialize to dictionary"""
        return {
            "metadata": self.metadata.to_dict(),
            "messages": [msg.to_dict() for msg in self.messages]
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "Session":
        """Deserialize from dictionary"""
        return cls(
            metadata=SessionMetadata.from_dict(data["metadata"]),
            messages=[Message.from_dict(msg) for msg in data["messages"]]
        )
```

### 2. Session Manager (Python)

```python
import json
from pathlib import Path
from typing import Optional, List
from datetime import datetime

class SessionManager:
    """Manages session persistence"""
    
    def __init__(self, sessions_dir: Optional[Path] = None):
        """Initialize session manager"""
        if sessions_dir is None:
            sessions_dir = Path.home() / ".config" / "openagent-terminal" / "sessions"
        
        self.sessions_dir = sessions_dir
        self.sessions_dir.mkdir(parents=True, exist_ok=True)
        self.index_file = self.sessions_dir / "index.json"
        self.current_session: Optional[Session] = None
        
        # Load or create index
        self._load_index()
    
    def _load_index(self):
        """Load session index"""
        if self.index_file.exists():
            with open(self.index_file, 'r') as f:
                self.index = json.load(f)
        else:
            self.index = {"sessions": [], "version": "1.0"}
            self._save_index()
    
    def _save_index(self):
        """Save session index"""
        with open(self.index_file, 'w') as f:
            json.dump(self.index, f, indent=2)
    
    def create_session(self, title: Optional[str] = None) -> Session:
        """Create new session"""
        session_id = datetime.now().strftime("%Y-%m-%d_%H%M%S")
        metadata = SessionMetadata(
            session_id=session_id,
            created_at=datetime.now(),
            updated_at=datetime.now(),
            message_count=0,
            total_tokens=0,
            title=title
        )
        session = Session(metadata=metadata)
        self.current_session = session
        
        # Add to index
        self.index["sessions"].append(metadata.to_dict())
        self._save_index()
        
        return session
    
    def save_session(self, session: Optional[Session] = None) -> bool:
        """Save session to disk"""
        if session is None:
            session = self.current_session
        
        if session is None:
            return False
        
        session_file = self.sessions_dir / f"{session.metadata.session_id}.json"
        
        try:
            with open(session_file, 'w') as f:
                json.dump(session.to_dict(), f, indent=2)
            
            # Update index
            self._update_index_entry(session.metadata)
            return True
        except Exception as e:
            print(f"Error saving session: {e}")
            return False
    
    def load_session(self, session_id: str) -> Optional[Session]:
        """Load session from disk"""
        session_file = self.sessions_dir / f"{session_id}.json"
        
        if not session_file.exists():
            return None
        
        try:
            with open(session_file, 'r') as f:
                data = json.load(f)
            
            session = Session.from_dict(data)
            self.current_session = session
            return session
        except Exception as e:
            print(f"Error loading session: {e}")
            return None
    
    def list_sessions(self, limit: Optional[int] = None) -> List[SessionMetadata]:
        """List all sessions"""
        sessions = [SessionMetadata.from_dict(s) for s in self.index["sessions"]]
        sessions.sort(key=lambda s: s.updated_at, reverse=True)
        
        if limit:
            sessions = sessions[:limit]
        
        return sessions
    
    def delete_session(self, session_id: str) -> bool:
        """Delete session"""
        session_file = self.sessions_dir / f"{session_id}.json"
        
        try:
            if session_file.exists():
                session_file.unlink()
            
            # Remove from index
            self.index["sessions"] = [
                s for s in self.index["sessions"] 
                if s["session_id"] != session_id
            ]
            self._save_index()
            return True
        except Exception as e:
            print(f"Error deleting session: {e}")
            return False
    
    def export_to_markdown(self, session: Optional[Session] = None) -> str:
        """Export session to markdown"""
        if session is None:
            session = self.current_session
        
        if session is None:
            return ""
        
        lines = [
            f"# {session.metadata.title or 'Untitled Session'}",
            "",
            f"**Session ID:** {session.metadata.session_id}",
            f"**Created:** {session.metadata.created_at.strftime('%Y-%m-%d %H:%M:%S')}",
            f"**Updated:** {session.metadata.updated_at.strftime('%Y-%m-%d %H:%M:%S')}",
            f"**Messages:** {session.metadata.message_count}",
            f"**Total Tokens:** {session.metadata.total_tokens}",
            "",
            "---",
            ""
        ]
        
        for msg in session.messages:
            role_emoji = {
                MessageRole.USER: "👤",
                MessageRole.ASSISTANT: "🤖",
                MessageRole.SYSTEM: "⚙️",
                MessageRole.TOOL: "🔧"
            }
            
            emoji = role_emoji.get(msg.role, "")
            timestamp = msg.timestamp.strftime("%H:%M:%S")
            
            lines.extend([
                f"## {emoji} {msg.role.value.title()} [{timestamp}]",
                "",
                msg.content,
                ""
            ])
            
            if msg.tool_calls:
                lines.extend([
                    "**Tool Calls:**",
                    "```json",
                    json.dumps(msg.tool_calls, indent=2),
                    "```",
                    ""
                ])
        
        return "\n".join(lines)
    
    def _update_index_entry(self, metadata: SessionMetadata):
        """Update session in index"""
        for i, session in enumerate(self.index["sessions"]):
            if session["session_id"] == metadata.session_id:
                self.index["sessions"][i] = metadata.to_dict()
                break
        self._save_index()
```

### 3. Rust Session State (src/session.rs)

```rust
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub session_id: Option<String>,
    pub is_restored: bool,
    pub message_count: usize,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            session_id: None,
            is_restored: false,
            message_count: 0,
        }
    }
}

impl SessionState {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn start_session(&mut self, session_id: String) {
        self.session_id = Some(session_id);
        self.is_restored = false;
        self.message_count = 0;
    }
    
    pub fn restore_session(&mut self, session_id: String, message_count: usize) {
        self.session_id = Some(session_id);
        self.is_restored = true;
        self.message_count = message_count;
    }
    
    pub fn increment_message_count(&mut self) {
        self.message_count += 1;
    }
    
    pub fn clear(&mut self) {
        self.session_id = None;
        self.is_restored = false;
        self.message_count = 0;
    }
}
```

---

## IPC Messages

### New Message Types

```rust
// In src/ipc/message.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcMessage {
    // Existing messages...
    Initialize { demo_mode: bool },
    Query { query: String },
    ToolApprove { tool_id: String, approved: bool },
    // ... existing messages ...
    
    // New session messages
    SessionCreate { title: Option<String> },
    SessionSave,
    SessionLoad { session_id: String },
    SessionList { limit: Option<usize> },
    SessionDelete { session_id: String },
    SessionExport { session_id: Option<String>, format: String },
    
    // Session responses
    SessionCreated { session_id: String },
    SessionSaved { success: bool },
    SessionLoaded { session_id: String, message_count: usize },
    SessionListResult { sessions: Vec<SessionInfo> },
    SessionDeleted { success: bool },
    SessionExported { content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
    pub total_tokens: usize,
}
```

---

## File Structure

```
~/.config/openagent-terminal/
├── sessions/
│   ├── index.json              # Session index
│   ├── 2025-10-04_114203.json  # Session file
│   ├── 2025-10-04_153045.json
│   └── 2025-10-04_160312.json
├── history                     # Command history
└── config.toml                 # Configuration
```

### Session File Format (JSON)

```json
{
  "metadata": {
    "session_id": "2025-10-04_114203",
    "created_at": "2025-10-04T11:42:03Z",
    "updated_at": "2025-10-04T12:15:30Z",
    "message_count": 12,
    "total_tokens": 4567,
    "title": "Debug authentication issue",
    "tags": ["bug", "auth", "python"]
  },
  "messages": [
    {
      "role": "user",
      "content": "Help me debug this authentication error",
      "timestamp": "2025-10-04T11:42:03Z",
      "token_count": 12,
      "metadata": {}
    },
    {
      "role": "assistant",
      "content": "I'll help you debug the authentication error. Let me check the logs...",
      "timestamp": "2025-10-04T11:42:05Z",
      "token_count": 156,
      "tool_calls": [
        {
          "tool": "read_file",
          "args": {"path": "/var/log/auth.log"}
        }
      ],
      "metadata": {}
    }
  ]
}
```

### Index File Format (JSON)

```json
{
  "version": "1.0",
  "sessions": [
    {
      "session_id": "2025-10-04_160312",
      "title": "Implement session persistence",
      "created_at": "2025-10-04T16:03:12Z",
      "updated_at": "2025-10-04T16:45:20Z",
      "message_count": 8,
      "total_tokens": 3421,
      "tags": ["feature", "session"]
    },
    {
      "session_id": "2025-10-04_153045",
      "title": "Add test coverage",
      "created_at": "2025-10-04T15:30:45Z",
      "updated_at": "2025-10-04T15:55:10Z",
      "message_count": 15,
      "total_tokens": 5678,
      "tags": ["testing"]
    }
  ]
}
```

---

## Integration Points

### 1. Auto-save in bridge.py

```python
class Bridge:
    def __init__(self, demo_mode: bool = True):
        self.session_manager = SessionManager()
        self.current_session = self.session_manager.create_session()
        # ... existing init ...
    
    def handle_query(self, request: Dict[str, Any]) -> Dict[str, Any]:
        query = request.get("query", "")
        
        # Add user message to session
        user_msg = Message(
            role=MessageRole.USER,
            content=query,
            timestamp=datetime.now()
        )
        self.current_session.add_message(user_msg)
        
        # Process query...
        response = self.agent.process_query(query)
        
        # Add assistant response to session
        assistant_msg = Message(
            role=MessageRole.ASSISTANT,
            content=response,
            timestamp=datetime.now(),
            token_count=self._estimate_tokens(response)
        )
        self.current_session.add_message(assistant_msg)
        
        # Auto-save session
        self.session_manager.save_session(self.current_session)
        
        return {"response": response}
```

### 2. Session restoration in main.rs

```rust
// On startup, check for --restore flag
if let Some(session_id) = matches.value_of("restore") {
    let msg = IpcMessage::SessionLoad {
        session_id: session_id.to_string()
    };
    client.send(msg)?;
    
    // Wait for response
    if let Some(IpcMessage::SessionLoaded { session_id, message_count }) = client.receive()? {
        app_state.session.restore_session(session_id, message_count);
        println!("Restored session with {} messages", message_count);
    }
}
```

---

## Features

### Core Features (Week 3)

1. **Session Creation**
   - Auto-create on first message
   - Optional title
   - Unique session ID

2. **Auto-save**
   - Save after each message exchange
   - Non-blocking operation
   - Error handling

3. **Session Loading**
   - Load by session ID
   - Restore full conversation history
   - Display in TUI

4. **Session Listing**
   - List recent sessions
   - Show metadata (date, message count, tokens)
   - Sort by date

5. **Export to Markdown**
   - Full conversation export
   - Formatted with timestamps
   - Include tool calls

### Advanced Features (Future)

1. **Session Search**
   - Full-text search across sessions
   - Filter by date, tags
   - Search within conversation

2. **Session Tags**
   - Tag sessions with topics
   - Filter by tags
   - Auto-tagging based on content

3. **Session Analytics**
   - Token usage over time
   - Most active time periods
   - Common topics

4. **Session Branching**
   - Create branches from any point
   - Compare different approaches
   - Merge conversations

---

## Testing Strategy

### Unit Tests

1. **Session Model Tests** (test_session.py)
   - Message serialization/deserialization
   - Session metadata updates
   - Token counting
   - Timestamp handling

2. **Session Manager Tests**
   - Create/save/load sessions
   - Index management
   - Corrupted data handling
   - File permissions
   - Concurrent access

3. **Export Tests**
   - Markdown formatting
   - Special character handling
   - Large sessions
   - Empty sessions

### Integration Tests

1. **End-to-end session flow**
   - Create → Add messages → Save → Load
   - Verify data integrity

2. **IPC message handling**
   - Session commands through IPC
   - Response handling

3. **UI integration**
   - Session display in TUI
   - Session switching

---

## Performance Considerations

1. **Auto-save throttling**
   - Don't save on every token
   - Save at message completion
   - Batch writes

2. **Memory management**
   - Keep only current session in memory
   - Lazy-load old sessions
   - Pagination for large sessions

3. **File I/O optimization**
   - Async file operations
   - Index caching
   - Compression for old sessions

---

## Security Considerations

1. **File permissions**
   - Sessions directory: 700 (owner only)
   - Session files: 600 (owner read/write)
   - No sensitive data in filenames

2. **Data sanitization**
   - Escape special characters
   - Validate session IDs
   - Prevent path traversal

3. **Size limits**
   - Max session size: 100MB
   - Max message length: 1MB
   - Max sessions: 1000 (auto-cleanup)

---

## Implementation Phases

### Phase 1: Core Data Structures (2h)
- Implement Message, SessionMetadata, Session classes
- Add serialization/deserialization
- Write unit tests

### Phase 2: Session Manager (3h)
- Implement SessionManager class
- File I/O operations
- Index management
- Write unit tests

### Phase 3: Backend Integration (2h)
- Integrate into bridge.py
- Auto-save on message exchange
- IPC message handlers
- Write integration tests

### Phase 4: Rust Integration (3h)
- Create src/session.rs
- Add IPC message types
- Session state tracking
- Write Rust tests

### Phase 5: UI & Polish (2h)
- Session display in TUI
- Session list UI
- Error handling
- Documentation

**Total: 12 hours**

---

## Success Metrics

1. **Functionality**
   - ✅ Sessions saved automatically
   - ✅ Sessions restored correctly
   - ✅ Export to markdown works
   - ✅ 100% data integrity

2. **Performance**
   - ✅ Save time < 50ms
   - ✅ Load time < 100ms
   - ✅ No UI blocking

3. **Quality**
   - ✅ 90%+ test coverage
   - ✅ Zero data loss
   - ✅ Handles edge cases

4. **User Experience**
   - ✅ Transparent operation
   - ✅ Clear status indicators
   - ✅ Easy session switching

---

## Next Steps

1. ✅ Design complete
2. → Implement data structures
3. → Write tests
4. → Integrate into backend
5. → Add Rust support
6. → UI integration
7. → Documentation

**Ready to implement! 🚀**
