#!/usr/bin/env python3
"""Simple test runner for session module."""

import sys
import tempfile
from pathlib import Path

# Add current directory to path
sys.path.insert(0, str(Path(__file__).parent))

from datetime import datetime
from openagent_terminal.session import (
    Message,
    MessageRole,
    Session,
    SessionManager,
    SessionMetadata,
)


def test_message_creation():
    """Test creating a message."""
    msg = Message(
        role=MessageRole.USER,
        content="Test message",
        timestamp=datetime.now()
    )
    
    assert msg.role == MessageRole.USER
    assert msg.content == "Test message"
    assert isinstance(msg.timestamp, datetime)
    print("✓ test_message_creation passed")


def test_message_serialization():
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
    print("✓ test_message_serialization passed")


def test_session_metadata():
    """Test session metadata."""
    now = datetime.now()
    metadata = SessionMetadata(
        session_id="test-123",
        created_at=now,
        updated_at=now,
        message_count=0,
        total_tokens=0,
        title="Test Session"
    )
    
    assert metadata.session_id == "test-123"
    assert metadata.title == "Test Session"
    
    # Test serialization
    data = metadata.to_dict()
    restored = SessionMetadata.from_dict(data)
    assert restored.session_id == metadata.session_id
    print("✓ test_session_metadata passed")


def test_session():
    """Test session with messages."""
    metadata = SessionMetadata(
        session_id="test-123",
        created_at=datetime.now(),
        updated_at=datetime.now(),
        message_count=0,
        total_tokens=0
    )
    session = Session(metadata=metadata)
    
    # Add messages
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
    print("✓ test_session passed")


def test_session_manager():
    """Test session manager with temp directory."""
    with tempfile.TemporaryDirectory() as tmpdir:
        temp_dir = Path(tmpdir)
        manager = SessionManager(sessions_dir=temp_dir)
        
        # Test initialization
        assert manager.sessions_dir == temp_dir
        assert manager.index_file.exists()
        
        # Create session
        session = manager.create_session(title="Test Session")
        assert session is not None
        assert session.metadata.title == "Test Session"
        
        # Add message
        msg = Message(
            role=MessageRole.USER,
            content="Hello",
            timestamp=datetime.now()
        )
        session.add_message(msg)
        
        # Save session
        success = manager.save_session(session)
        assert success is True
        
        # Load session
        session_id = session.metadata.session_id
        loaded = manager.load_session(session_id)
        assert loaded is not None
        assert len(loaded.messages) == 1
        assert loaded.messages[0].content == "Hello"
        
        # List sessions
        sessions = manager.list_sessions()
        assert len(sessions) == 1
        
        # Export to markdown
        markdown = manager.export_to_markdown(session)
        assert "# Test Session" in markdown
        assert "Hello" in markdown
        
        # Delete session
        success = manager.delete_session(session_id)
        assert success is True
        assert len(manager.list_sessions()) == 0
        
        print("✓ test_session_manager passed")


def test_path_traversal_protection():
    """Test path traversal protection."""
    with tempfile.TemporaryDirectory() as tmpdir:
        temp_dir = Path(tmpdir)
        manager = SessionManager(sessions_dir=temp_dir)
        
        # Try various path traversal attempts
        assert manager.load_session("../etc/passwd") is None
        assert manager.load_session("..\\windows\\system32") is None
        assert manager.load_session("foo/bar") is None
        
        assert manager.delete_session("../etc/passwd") is False
        assert manager.delete_session("foo/bar") is False
        
        print("✓ test_path_traversal_protection passed")


def test_cleanup():
    """Test session cleanup."""
    with tempfile.TemporaryDirectory() as tmpdir:
        temp_dir = Path(tmpdir)
        manager = SessionManager(sessions_dir=temp_dir)
        
        # Create many sessions
        for i in range(15):
            session = manager.create_session(title=f"Session {i}")
            manager.save_session(session)
        
        # Cleanup to keep only 10
        deleted = manager.cleanup_old_sessions(max_sessions=10)
        
        assert deleted == 5
        assert len(manager.list_sessions()) == 10
        
        print("✓ test_cleanup passed")


def main():
    """Run all tests."""
    print("Running session persistence tests...\n")
    
    tests = [
        test_message_creation,
        test_message_serialization,
        test_session_metadata,
        test_session,
        test_session_manager,
        test_path_traversal_protection,
        test_cleanup,
    ]
    
    passed = 0
    failed = 0
    
    for test in tests:
        try:
            test()
            passed += 1
        except AssertionError as e:
            print(f"✗ {test.__name__} failed: {e}")
            failed += 1
        except Exception as e:
            print(f"✗ {test.__name__} error: {e}")
            failed += 1
    
    print(f"\n{'='*60}")
    print(f"Results: {passed} passed, {failed} failed out of {len(tests)} total")
    print(f"{'='*60}")
    
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
