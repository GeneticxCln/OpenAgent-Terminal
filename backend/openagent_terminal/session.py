"""Session persistence data structures and manager.

This module provides classes for managing conversation sessions, including
saving and loading session history, metadata tracking, and export functionality.
"""

import json
import os
import shutil
import tempfile
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from threading import Lock
from typing import Any, Dict, List, Optional


class MessageRole(Enum):
    """Message role in conversation."""
    USER = "user"
    ASSISTANT = "assistant"
    SYSTEM = "system"
    TOOL = "tool"


@dataclass
class Message:
    """Single message in conversation."""
    role: MessageRole
    content: str
    timestamp: datetime
    token_count: Optional[int] = None
    tool_calls: Optional[List[Dict[str, Any]]] = None
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        """Serialize to dictionary."""
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
        """Deserialize from dictionary."""
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
    """Session metadata."""
    session_id: str
    created_at: datetime
    updated_at: datetime
    message_count: int
    total_tokens: int
    title: Optional[str] = None
    tags: List[str] = field(default_factory=list)
    
    def to_dict(self) -> Dict[str, Any]:
        """Serialize to dictionary."""
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
        """Deserialize from dictionary."""
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
    """Complete session with messages and metadata."""
    metadata: SessionMetadata
    messages: List[Message] = field(default_factory=list)
    _lock: Lock = field(default_factory=Lock, init=False, repr=False)
    
    def add_message(self, message: Message) -> None:
        """Add message and update metadata.
        
        Thread-safe: Uses internal lock to prevent race conditions.
        """
        with self._lock:
            self.messages.append(message)
            self.metadata.message_count = len(self.messages)
            self.metadata.updated_at = datetime.now()
            if message.token_count:
                self.metadata.total_tokens += message.token_count
            
            # Auto-generate title from first user message
            if self.metadata.title is None and message.role == MessageRole.USER and len(self.messages) == 1:
                self.metadata.title = self._generate_title(message.content)
    
    def to_dict(self) -> Dict[str, Any]:
        """Serialize to dictionary."""
        return {
            "metadata": self.metadata.to_dict(),
            "messages": [msg.to_dict() for msg in self.messages]
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "Session":
        """Deserialize from dictionary."""
        return cls(
            metadata=SessionMetadata.from_dict(data["metadata"]),
            messages=[Message.from_dict(msg) for msg in data["messages"]]
        )
    
    def _generate_title(self, content: str) -> str:
        """Generate a session title from message content.
        
        Args:
            content: Message content to generate title from
            
        Returns:
            Generated title (max 50 chars)
        """
        # Remove extra whitespace
        title = ' '.join(content.split())
        
        # Truncate to 50 chars, add ellipsis if needed
        max_length = 50
        if len(title) > max_length:
            title = title[:max_length-3] + "..."
        
        return title
    
    def _escape_markdown_content(self, content: str) -> str:
        """Escape markdown special characters in content.
        
        Args:
            content: Content to escape
            
        Returns:
            Escaped content
        """
        # Don't escape code blocks
        if content.strip().startswith("```"):
            return content
        
        # Escape markdown headers at line start
        lines = []
        for line in content.split('\n'):
            if line.startswith('#'):
                line = '\\' + line
            lines.append(line)
        
        return '\n'.join(lines)


class SessionManager:
    """Manages session persistence."""
    
    def __init__(self, sessions_dir: Optional[Path] = None):
        """Initialize session manager.
        
        Args:
            sessions_dir: Directory for storing sessions. Defaults to
                ~/.config/openagent-terminal/sessions
        """
        if sessions_dir is None:
            sessions_dir = Path.home() / ".config" / "openagent-terminal" / "sessions"
        
        self.sessions_dir = sessions_dir
        self.sessions_dir.mkdir(parents=True, exist_ok=True)
        
        # Set proper permissions (owner only)
        try:
            os.chmod(self.sessions_dir, 0o700)
        except (OSError, AttributeError):
            # Windows doesn't support chmod
            pass
        
        self.index_file = self.sessions_dir / "index.json"
        self.current_session: Optional[Session] = None
        
        # Load or create index
        self._load_index()
    
    def _load_index(self) -> None:
        """Load session index."""
        if self.index_file.exists():
            try:
                with open(self.index_file, 'r', encoding='utf-8') as f:
                    self.index = json.load(f)
            except (json.JSONDecodeError, IOError):
                # Corrupted index, create new one
                self.index = {"sessions": [], "version": "1.0"}
                self._save_index()
        else:
            self.index = {"sessions": [], "version": "1.0"}
            self._save_index()
    
    def _save_index(self) -> None:
        """Save session index atomically using temp file."""
        tmp_path = None
        try:
            # Write to temp file first (atomic operation)
            with tempfile.NamedTemporaryFile(
                mode='w',
                encoding='utf-8',
                dir=self.sessions_dir,
                delete=False,
                prefix='.index_',
                suffix='.tmp'
            ) as tmp:
                json.dump(self.index, tmp, indent=2)
                tmp_path = tmp.name
            
            # Set proper permissions before moving
            try:
                os.chmod(tmp_path, 0o600)
            except (OSError, AttributeError):
                pass
            
            # Atomic rename (replaces old file)
            shutil.move(tmp_path, self.index_file)
            tmp_path = None  # Successfully moved
            
        except IOError as e:
            print(f"Error saving index: {e}")
            # Clean up temp file if it exists
            if tmp_path and os.path.exists(tmp_path):
                try:
                    os.unlink(tmp_path)
                except:
                    pass
    
    def create_session(self, title: Optional[str] = None) -> Session:
        """Create new session.
        
        Args:
            title: Optional session title
            
        Returns:
            New session instance
        """
        session_id = datetime.now().strftime("%Y-%m-%d_%H%M%S")
        
        # Handle duplicate session IDs (unlikely but possible)
        counter = 1
        while (self.sessions_dir / f"{session_id}.json").exists():
            session_id = f"{datetime.now().strftime('%Y-%m-%d_%H%M%S')}_{counter}"
            counter += 1
        
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
        
        # Auto-cleanup if too many sessions (prevent unbounded growth)
        if len(self.index["sessions"]) > 1000:
            import logging
            logger = logging.getLogger(__name__)
            logger.warning(f"Session limit exceeded ({len(self.index['sessions'])}), cleaning up old sessions")
            self.cleanup_old_sessions(max_sessions=800)
        
        self._save_index()
        
        return session
    
    def save_session(self, session: Optional[Session] = None) -> bool:
        """Save session to disk.
        
        Args:
            session: Session to save. If None, saves current session.
            
        Returns:
            True if save successful, False otherwise
        """
        if session is None:
            session = self.current_session
        
        if session is None:
            return False
        
        session_file = self.sessions_dir / f"{session.metadata.session_id}.json"
        
        try:
            with open(session_file, 'w', encoding='utf-8') as f:
                json.dump(session.to_dict(), f, indent=2)
            
            # Set proper permissions (owner only)
            try:
                os.chmod(session_file, 0o600)
            except (OSError, AttributeError):
                pass
            
            # Update index
            self._update_index_entry(session.metadata)
            return True
        except (IOError, TypeError) as e:
            print(f"Error saving session: {e}")
            return False
    
    def load_session(self, session_id: str) -> Optional[Session]:
        """Load session from disk.
        
        Args:
            session_id: Session ID to load
            
        Returns:
            Loaded session or None if not found/error
        """
        # Validate session_id to prevent path traversal
        if ".." in session_id or "/" in session_id or "\\" in session_id:
            return None
        
        session_file = self.sessions_dir / f"{session_id}.json"
        
        if not session_file.exists():
            return None
        
        try:
            with open(session_file, 'r', encoding='utf-8') as f:
                data = json.load(f)
            
            session = Session.from_dict(data)
            self.current_session = session
            return session
        except (json.JSONDecodeError, IOError, KeyError, ValueError) as e:
            print(f"Error loading session: {e}")
            return None
    
    def list_sessions(self, limit: Optional[int] = None) -> List[SessionMetadata]:
        """List all sessions.
        
        Args:
            limit: Maximum number of sessions to return
            
        Returns:
            List of session metadata, sorted by update time (newest first)
        """
        try:
            sessions = [SessionMetadata.from_dict(s) for s in self.index["sessions"]]
            sessions.sort(key=lambda s: s.updated_at, reverse=True)
            
            if limit is not None and limit > 0:
                sessions = sessions[:limit]
            
            return sessions
        except (KeyError, ValueError) as e:
            print(f"Error listing sessions: {e}")
            return []
    
    def delete_session(self, session_id: str) -> bool:
        """Delete session.
        
        Args:
            session_id: Session ID to delete
            
        Returns:
            True if deletion successful, False otherwise
        """
        # Validate session_id to prevent path traversal
        if ".." in session_id or "/" in session_id or "\\" in session_id:
            return False
        
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
        except (IOError, KeyError) as e:
            print(f"Error deleting session: {e}")
            return False
    
    def export_to_markdown(self, session: Optional[Session] = None) -> str:
        """Export session to markdown.
        
        Args:
            session: Session to export. If None, exports current session.
            
        Returns:
            Markdown formatted string
        """
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
                self._escape_markdown_content(msg.content),
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
    
    def _update_index_entry(self, metadata: SessionMetadata) -> None:
        """Update session in index.
        
        Args:
            metadata: Updated session metadata
        """
        for i, session in enumerate(self.index["sessions"]):
            if session["session_id"] == metadata.session_id:
                self.index["sessions"][i] = metadata.to_dict()
                break
        self._save_index()
    
    def cleanup_old_sessions(self, max_sessions: int = 1000) -> int:
        """Clean up old sessions if limit exceeded.
        
        Args:
            max_sessions: Maximum number of sessions to keep
            
        Returns:
            Number of sessions deleted
        """
        sessions = self.list_sessions()
        
        if len(sessions) <= max_sessions:
            return 0
        
        # Delete oldest sessions
        sessions_to_delete = sessions[max_sessions:]
        deleted_count = 0
        
        for session_meta in sessions_to_delete:
            if self.delete_session(session_meta.session_id):
                deleted_count += 1
        
        return deleted_count
