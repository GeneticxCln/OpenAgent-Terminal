"""Tests for session persistence module."""

import json
import tempfile
from datetime import datetime
from pathlib import Path

import pytest

from openagent_terminal.session import (
    Message,
    MessageRole,
    Session,
    SessionManager,
    SessionMetadata,
)


class TestMessageRole:
    """Tests for MessageRole enum."""
    
    def test_message_roles(self):
        """Test all message role values."""
        assert MessageRole.USER.value == "user"
        assert MessageRole.ASSISTANT.value == "assistant"
        assert MessageRole.SYSTEM.value == "system"
        assert MessageRole.TOOL.value == "tool"


class TestMessage:
    """Tests for Message class."""
    
    def test_message_creation(self):
        """Test creating a message."""
        msg = Message(
            role=MessageRole.USER,
            content="Test message",
            timestamp=datetime.now()
        )
        
        assert msg.role == MessageRole.USER
        assert msg.content == "Test message"
        assert isinstance(msg.timestamp, datetime)
        assert msg.token_count is None
        assert msg.tool_calls is None
        assert msg.metadata == {}
    
    def test_message_with_token_count(self):
        """Test message with token count."""
        msg = Message(
            role=MessageRole.ASSISTANT,
            content="Response",
            timestamp=datetime.now(),
            token_count=42
        )
        
        assert msg.token_count == 42
    
    def test_message_with_tool_calls(self):
        """Test message with tool calls."""
        tool_calls = [{"tool": "test", "args": {"foo": "bar"}}]
        msg = Message(
            role=MessageRole.ASSISTANT,
            content="Using tool",
            timestamp=datetime.now(),
            tool_calls=tool_calls
        )
        
        assert msg.tool_calls == tool_calls
    
    def test_message_serialization(self):
        """Test message serialization to dict."""
        timestamp = datetime.now()
        msg = Message(
            role=MessageRole.USER,
            content="Test",
            timestamp=timestamp,
            token_count=10
        )
        
        data = msg.to_dict()
        
        assert data["role"] == "user"
        assert data["content"] == "Test"
        assert data["timestamp"] == timestamp.isoformat()
        assert data["token_count"] == 10
    
    def test_message_deserialization(self):
        """Test message deserialization from dict."""
        data = {
            "role": "assistant",
            "content": "Response",
            "timestamp": "2025-10-04T12:00:00",
            "token_count": 20,
            "tool_calls": None,
            "metadata": {}
        }
        
        msg = Message.from_dict(data)
        
        assert msg.role == MessageRole.ASSISTANT
        assert msg.content == "Response"
        assert isinstance(msg.timestamp, datetime)
        assert msg.token_count == 20
    
    def test_message_roundtrip(self):
        """Test message serialization roundtrip."""
        original = Message(
            role=MessageRole.USER,
            content="Test message",
            timestamp=datetime.now(),
            token_count=15,
            metadata={"foo": "bar"}
        )
        
        # Serialize and deserialize
        data = original.to_dict()
        restored = Message.from_dict(data)
        
        assert restored.role == original.role
        assert restored.content == original.content
        assert restored.token_count == original.token_count
        assert restored.metadata == original.metadata


class TestSessionMetadata:
    """Tests for SessionMetadata class."""
    
    def test_metadata_creation(self):
        """Test creating session metadata."""
        now = datetime.now()
        metadata = SessionMetadata(
            session_id="test-123",
            created_at=now,
            updated_at=now,
            message_count=0,
            total_tokens=0
        )
        
        assert metadata.session_id == "test-123"
        assert metadata.message_count == 0
        assert metadata.total_tokens == 0
        assert metadata.title is None
        assert metadata.tags == []
    
    def test_metadata_with_title_and_tags(self):
        """Test metadata with title and tags."""
        metadata = SessionMetadata(
            session_id="test-123",
            created_at=datetime.now(),
            updated_at=datetime.now(),
            message_count=5,
            total_tokens=100,
            title="Test Session",
            tags=["test", "example"]
        )
        
        assert metadata.title == "Test Session"
        assert metadata.tags == ["test", "example"]
    
    def test_metadata_serialization(self):
        """Test metadata serialization."""
        now = datetime.now()
        metadata = SessionMetadata(
            session_id="test-123",
            created_at=now,
            updated_at=now,
            message_count=10,
            total_tokens=200,
            title="My Session"
        )
        
        data = metadata.to_dict()
        
        assert data["session_id"] == "test-123"
        assert data["message_count"] == 10
        assert data["total_tokens"] == 200
        assert data["title"] == "My Session"
    
    def test_metadata_deserialization(self):
        """Test metadata deserialization."""
        data = {
            "session_id": "test-456",
            "created_at": "2025-10-04T10:00:00",
            "updated_at": "2025-10-04T11:00:00",
            "message_count": 5,
            "total_tokens": 150,
            "title": "Test",
            "tags": ["foo"]
        }
        
        metadata = SessionMetadata.from_dict(data)
        
        assert metadata.session_id == "test-456"
        assert metadata.message_count == 5
        assert metadata.total_tokens == 150
        assert metadata.title == "Test"
        assert metadata.tags == ["foo"]


class TestSession:
    """Tests for Session class."""
    
    def test_session_creation(self):
        """Test creating a session."""
        metadata = SessionMetadata(
            session_id="test-123",
            created_at=datetime.now(),
            updated_at=datetime.now(),
            message_count=0,
            total_tokens=0
        )
        session = Session(metadata=metadata)
        
        assert session.metadata == metadata
        assert session.messages == []
    
    def test_add_message(self):
        """Test adding message to session."""
        metadata = SessionMetadata(
            session_id="test-123",
            created_at=datetime.now(),
            updated_at=datetime.now(),
            message_count=0,
            total_tokens=0
        )
        session = Session(metadata=metadata)
        
        msg = Message(
            role=MessageRole.USER,
            content="Test",
            timestamp=datetime.now(),
            token_count=10
        )
        
        session.add_message(msg)
        
        assert len(session.messages) == 1
        assert session.metadata.message_count == 1
        assert session.metadata.total_tokens == 10
    
    def test_add_multiple_messages(self):
        """Test adding multiple messages."""
        metadata = SessionMetadata(
            session_id="test-123",
            created_at=datetime.now(),
            updated_at=datetime.now(),
            message_count=0,
            total_tokens=0
        )
        session = Session(metadata=metadata)
        
        for i in range(5):
            msg = Message(
                role=MessageRole.USER if i % 2 == 0 else MessageRole.ASSISTANT,
                content=f"Message {i}",
                timestamp=datetime.now(),
                token_count=10
            )
            session.add_message(msg)
        
        assert len(session.messages) == 5
        assert session.metadata.message_count == 5
        assert session.metadata.total_tokens == 50
    
    def test_session_serialization(self):
        """Test session serialization."""
        metadata = SessionMetadata(
            session_id="test-123",
            created_at=datetime.now(),
            updated_at=datetime.now(),
            message_count=0,
            total_tokens=0
        )
        session = Session(metadata=metadata)
        
        msg = Message(
            role=MessageRole.USER,
            content="Test",
            timestamp=datetime.now()
        )
        session.add_message(msg)
        
        data = session.to_dict()
        
        assert "metadata" in data
        assert "messages" in data
        assert len(data["messages"]) == 1
    
    def test_session_deserialization(self):
        """Test session deserialization."""
        data = {
            "metadata": {
                "session_id": "test-123",
                "created_at": "2025-10-04T10:00:00",
                "updated_at": "2025-10-04T11:00:00",
                "message_count": 1,
                "total_tokens": 10,
                "title": None,
                "tags": []
            },
            "messages": [
                {
                    "role": "user",
                    "content": "Test",
                    "timestamp": "2025-10-04T10:00:00",
                    "token_count": 10,
                    "tool_calls": None,
                    "metadata": {}
                }
            ]
        }
        
        session = Session.from_dict(data)
        
        assert session.metadata.session_id == "test-123"
        assert len(session.messages) == 1
        assert session.messages[0].content == "Test"


class TestSessionManager:
    """Tests for SessionManager class."""
    
    @pytest.fixture
    def temp_sessions_dir(self):
        """Create temporary sessions directory."""
        with tempfile.TemporaryDirectory() as tmpdir:
            yield Path(tmpdir)
    
    def test_manager_initialization(self, temp_sessions_dir):
        """Test session manager initialization."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        
        assert manager.sessions_dir == temp_sessions_dir
        assert manager.sessions_dir.exists()
        assert manager.index_file.exists()
        assert manager.current_session is None
    
    def test_index_creation(self, temp_sessions_dir):
        """Test index file creation."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        
        with open(manager.index_file, 'r') as f:
            index = json.load(f)
        
        assert index["version"] == "1.0"
        assert index["sessions"] == []
    
    def test_create_session(self, temp_sessions_dir):
        """Test creating a session."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        session = manager.create_session(title="Test Session")
        
        assert session is not None
        assert session.metadata.title == "Test Session"
        assert manager.current_session == session
        
        # Check index updated
        with open(manager.index_file, 'r') as f:
            index = json.load(f)
        
        assert len(index["sessions"]) == 1
        assert index["sessions"][0]["title"] == "Test Session"
    
    def test_save_session(self, temp_sessions_dir):
        """Test saving a session."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        session = manager.create_session(title="Test")
        
        # Add a message
        msg = Message(
            role=MessageRole.USER,
            content="Hello",
            timestamp=datetime.now()
        )
        session.add_message(msg)
        
        # Save session
        success = manager.save_session(session)
        
        assert success is True
        
        # Check file exists
        session_file = temp_sessions_dir / f"{session.metadata.session_id}.json"
        assert session_file.exists()
        
        # Check content
        with open(session_file, 'r') as f:
            data = json.load(f)
        
        assert len(data["messages"]) == 1
        assert data["messages"][0]["content"] == "Hello"
    
    def test_load_session(self, temp_sessions_dir):
        """Test loading a session."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        
        # Create and save session
        session = manager.create_session(title="Test")
        msg = Message(
            role=MessageRole.USER,
            content="Test message",
            timestamp=datetime.now()
        )
        session.add_message(msg)
        manager.save_session(session)
        
        session_id = session.metadata.session_id
        
        # Clear current session
        manager.current_session = None
        
        # Load session
        loaded = manager.load_session(session_id)
        
        assert loaded is not None
        assert loaded.metadata.session_id == session_id
        assert len(loaded.messages) == 1
        assert loaded.messages[0].content == "Test message"
    
    def test_load_nonexistent_session(self, temp_sessions_dir):
        """Test loading non-existent session."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        loaded = manager.load_session("nonexistent-123")
        
        assert loaded is None
    
    def test_load_session_path_traversal_protection(self, temp_sessions_dir):
        """Test path traversal protection in load_session."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        
        # Try various path traversal attempts
        assert manager.load_session("../etc/passwd") is None
        assert manager.load_session("..\\windows\\system32") is None
        assert manager.load_session("foo/bar") is None
    
    def test_list_sessions(self, temp_sessions_dir):
        """Test listing sessions."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        
        # Create multiple sessions
        for i in range(3):
            session = manager.create_session(title=f"Session {i}")
            manager.save_session(session)
        
        # List sessions
        sessions = manager.list_sessions()
        
        assert len(sessions) == 3
        # Should be sorted by update time (newest first)
        assert isinstance(sessions[0], SessionMetadata)
    
    def test_list_sessions_with_limit(self, temp_sessions_dir):
        """Test listing sessions with limit."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        
        # Create multiple sessions
        for i in range(5):
            session = manager.create_session(title=f"Session {i}")
            manager.save_session(session)
        
        # List with limit
        sessions = manager.list_sessions(limit=2)
        
        assert len(sessions) == 2
    
    def test_delete_session(self, temp_sessions_dir):
        """Test deleting a session."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        
        # Create and save session
        session = manager.create_session(title="To Delete")
        manager.save_session(session)
        session_id = session.metadata.session_id
        
        # Verify file exists
        session_file = temp_sessions_dir / f"{session_id}.json"
        assert session_file.exists()
        
        # Delete session
        success = manager.delete_session(session_id)
        
        assert success is True
        assert not session_file.exists()
        
        # Check index updated
        sessions = manager.list_sessions()
        assert len(sessions) == 0
    
    def test_delete_session_path_traversal_protection(self, temp_sessions_dir):
        """Test path traversal protection in delete_session."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        
        # Try various path traversal attempts
        assert manager.delete_session("../etc/passwd") is False
        assert manager.delete_session("..\\windows\\system32") is False
        assert manager.delete_session("foo/bar") is False
    
    def test_export_to_markdown(self, temp_sessions_dir):
        """Test exporting session to markdown."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        session = manager.create_session(title="Test Export")
        
        # Add messages
        session.add_message(Message(
            role=MessageRole.USER,
            content="Hello",
            timestamp=datetime.now()
        ))
        session.add_message(Message(
            role=MessageRole.ASSISTANT,
            content="Hi there!",
            timestamp=datetime.now()
        ))
        
        # Export
        markdown = manager.export_to_markdown(session)
        
        assert "# Test Export" in markdown
        assert "Session ID:" in markdown
        assert "Hello" in markdown
        assert "Hi there!" in markdown
        assert "👤" in markdown  # User emoji
        assert "🤖" in markdown  # Assistant emoji
    
    def test_export_empty_session(self, temp_sessions_dir):
        """Test exporting empty session."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        session = manager.create_session(title="Empty")
        
        markdown = manager.export_to_markdown(session)
        
        assert "# Empty" in markdown
        assert "Messages: 0" in markdown
    
    def test_export_with_tool_calls(self, temp_sessions_dir):
        """Test exporting session with tool calls."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        session = manager.create_session(title="With Tools")
        
        tool_calls = [{"tool": "test_tool", "args": {"param": "value"}}]
        session.add_message(Message(
            role=MessageRole.ASSISTANT,
            content="Using tool",
            timestamp=datetime.now(),
            tool_calls=tool_calls
        ))
        
        markdown = manager.export_to_markdown(session)
        
        assert "Tool Calls:" in markdown
        assert "test_tool" in markdown
    
    def test_cleanup_old_sessions(self, temp_sessions_dir):
        """Test cleaning up old sessions."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        
        # Create many sessions
        for i in range(15):
            session = manager.create_session(title=f"Session {i}")
            manager.save_session(session)
        
        # Cleanup to keep only 10
        deleted = manager.cleanup_old_sessions(max_sessions=10)
        
        assert deleted == 5
        assert len(manager.list_sessions()) == 10
    
    def test_cleanup_no_excess_sessions(self, temp_sessions_dir):
        """Test cleanup with no excess sessions."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        
        # Create few sessions
        for i in range(3):
            session = manager.create_session(title=f"Session {i}")
            manager.save_session(session)
        
        # Try cleanup
        deleted = manager.cleanup_old_sessions(max_sessions=10)
        
        assert deleted == 0
        assert len(manager.list_sessions()) == 3
    
    def test_corrupted_index_recovery(self, temp_sessions_dir):
        """Test recovery from corrupted index file."""
        # Create corrupted index
        index_file = temp_sessions_dir / "index.json"
        index_file.parent.mkdir(parents=True, exist_ok=True)
        with open(index_file, 'w') as f:
            f.write("{ corrupted json")
        
        # Initialize manager - should recover
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        
        assert manager.index == {"sessions": [], "version": "1.0"}
    
    def test_save_session_without_current(self, temp_sessions_dir):
        """Test saving when no current session."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        success = manager.save_session()
        
        assert success is False
    
    def test_export_without_current_session(self, temp_sessions_dir):
        """Test export when no current session."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        markdown = manager.export_to_markdown()
        
        assert markdown == ""
    
    def test_session_id_uniqueness(self, temp_sessions_dir):
        """Test that duplicate session IDs are handled."""
        manager = SessionManager(sessions_dir=temp_sessions_dir)
        
        # Create first session
        session1 = manager.create_session()
        session_id1 = session1.metadata.session_id
        
        # Manually create file with same ID to force collision
        session_file = temp_sessions_dir / f"{session_id1}.json"
        with open(session_file, 'w') as f:
            json.dump(session1.to_dict(), f)
        
        # Create another session immediately - should get unique ID
        session2 = manager.create_session()
        session_id2 = session2.metadata.session_id
        
        # IDs should be different (second one has suffix)
        assert session_id1 != session_id2 or not (temp_sessions_dir / f"{session_id2}.json").exists()
