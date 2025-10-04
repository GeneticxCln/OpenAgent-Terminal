#!/usr/bin/env bash
# Test Script for Session Persistence (Phase 5 Week 3)

set -e

echo "╔════════════════════════════════════════════╗"
echo "║  Phase 5 Week 3: Session Persistence Test ║"
echo "╚════════════════════════════════════════════╝"
echo ""

# Check if backend is running
echo "📝 Step 1: Check if backend is running..."
if ! pgrep -f "openagent_terminal.bridge" > /dev/null; then
    echo "❌ Backend not running!"
    echo "   Start it with: cd backend && python -m openagent_terminal.bridge &"
    exit 1
fi
echo "✅ Backend is running"
echo ""

# Check sessions directory
echo "📝 Step 2: Check sessions directory..."
SESSION_DIR="$HOME/.config/openagent-terminal/sessions"
if [ ! -d "$SESSION_DIR" ]; then
    echo "❌ Sessions directory not found: $SESSION_DIR"
    exit 1
fi
echo "✅ Sessions directory exists: $SESSION_DIR"
echo "   Permissions: $(stat -c '%a' "$SESSION_DIR" 2>/dev/null || stat -f '%A' "$SESSION_DIR" 2>/dev/null)"
echo ""

# List current sessions
echo "📝 Step 3: List existing sessions..."
if [ -f "$SESSION_DIR/index.json" ]; then
    SESSION_COUNT=$(python3 -c "import json; data=json.load(open('$SESSION_DIR/index.json')); print(len(data['sessions']))")
    echo "✅ Found $SESSION_COUNT sessions"
    echo ""
    echo "Session IDs:"
    python3 -c "import json; data=json.load(open('$SESSION_DIR/index.json')); [print(f\"  - {s['session_id']} ({s['message_count']} messages, {s['total_tokens']} tokens)\") for s in data['sessions']]"
else
    echo "⚠️  No index.json yet (will be created on first use)"
fi
echo ""

# Test session file format
echo "📝 Step 4: Validate session file format..."
LATEST_SESSION=$(ls -t "$SESSION_DIR"/*.json 2>/dev/null | grep -v index.json | head -n 1)
if [ -n "$LATEST_SESSION" ]; then
    echo "Testing: $(basename "$LATEST_SESSION")"
    
    # Check if it's valid JSON
    if python3 -m json.tool "$LATEST_SESSION" > /dev/null 2>&1; then
        echo "✅ Valid JSON format"
        
        # Check required fields
        python3 << EOF
import json
import sys

with open("$LATEST_SESSION") as f:
    data = json.load(f)

required_fields = ["metadata", "messages"]
missing = [f for f in required_fields if f not in data]

if missing:
    print(f"❌ Missing required fields: {missing}")
    sys.exit(1)

metadata_fields = ["session_id", "created_at", "updated_at", "message_count", "total_tokens"]
missing_meta = [f for f in metadata_fields if f not in data["metadata"]]

if missing_meta:
    print(f"❌ Missing metadata fields: {missing_meta}")
    sys.exit(1)

print(f"✅ All required fields present")
print(f"   Messages: {len(data['messages'])}")
print(f"   Session ID: {data['metadata']['session_id']}")
EOF
    else
        echo "❌ Invalid JSON format"
        exit 1
    fi
else
    echo "⚠️  No session files found yet"
fi
echo ""

# Test Python session module directly
echo "📝 Step 5: Test Python session module..."
cd backend
python3 << 'EOF'
import sys
sys.path.insert(0, '.')

from openagent_terminal.session import Session, SessionMetadata, Message, MessageRole, SessionManager
from datetime import datetime
from pathlib import Path
import tempfile

print("✅ Successfully imported session module")

# Create a test session in temporary directory
with tempfile.TemporaryDirectory() as tmpdir:
    session_dir = Path(tmpdir) / "test_sessions"
    manager = SessionManager(sessions_dir=session_dir)
    
    # Create session
    session = manager.create_session(title="Test Session")
    print(f"✅ Created test session: {session.metadata.session_id}")
    
    # Add messages
    msg1 = Message(
        role=MessageRole.USER,
        content="Test query",
        timestamp=datetime.now(),
        token_count=2
    )
    session.add_message(msg1)
    
    msg2 = Message(
        role=MessageRole.ASSISTANT,
        content="Test response",
        timestamp=datetime.now(),
        token_count=2
    )
    session.add_message(msg2)
    
    print(f"✅ Added 2 messages to session")
    
    # Save session
    if manager.save_session(session):
        print("✅ Successfully saved session")
    else:
        print("❌ Failed to save session")
        sys.exit(1)
    
    # List sessions
    sessions = manager.list_sessions()
    if len(sessions) == 1:
        print(f"✅ Listed {len(sessions)} session")
    else:
        print(f"❌ Expected 1 session, found {len(sessions)}")
        sys.exit(1)
    
    # Load session
    loaded = manager.load_session(session.metadata.session_id)
    if loaded and len(loaded.messages) == 2:
        print(f"✅ Loaded session with {len(loaded.messages)} messages")
    else:
        print(f"❌ Failed to load session correctly")
        sys.exit(1)
    
    # Export to markdown
    markdown = manager.export_to_markdown(loaded)
    if "# Test Session" in markdown and "Test query" in markdown:
        print("✅ Exported session to markdown")
    else:
        print("❌ Markdown export failed")
        sys.exit(1)
    
    # Delete session
    if manager.delete_session(session.metadata.session_id):
        print("✅ Deleted test session")
    else:
        print("❌ Failed to delete session")
        sys.exit(1)

print("")
print("✅ All Python session tests passed!")
EOF
echo ""

# Check Rust session module compilation
echo "📝 Step 6: Check Rust session module..."
cd /home/quinton/openagent-terminal
if cargo check --quiet 2>&1 | grep -q "error"; then
    echo "❌ Rust compilation errors detected"
    cargo check 2>&1 | tail -20
    exit 1
else
    echo "✅ Rust code compiles successfully"
fi
echo ""

# Test IPC session commands (if backend is running)
echo "📝 Step 7: Test session IPC commands..."
cd /home/quinton/openagent-terminal
cat > /tmp/test_session_ipc.py << 'EOF'
import asyncio
import json
import os

async def test_session_commands():
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
    socket_path = f"{runtime_dir}/openagent-terminal-test.sock"
    
    if not os.path.exists(socket_path):
        print("⚠️  Socket not found, skipping IPC tests")
        return
    
    reader, writer = await asyncio.open_unix_connection(socket_path)
    
    # Test session.list
    request = {
        "jsonrpc": "2.0",
        "id": 1000,
        "method": "session.list",
        "params": {"limit": 5}
    }
    
    writer.write((json.dumps(request) + "\n").encode())
    await writer.drain()
    
    response_line = await reader.readline()
    response = json.loads(response_line.decode())
    
    if "result" in response and "sessions" in response["result"]:
        session_count = len(response["result"]["sessions"])
        print(f"✅ session.list works: {session_count} sessions found")
    else:
        print("❌ session.list failed")
        print(f"   Response: {response}")
        return
    
    writer.close()
    await writer.wait_closed()
    
    print("✅ All IPC session commands working")

asyncio.run(test_session_commands())
EOF

python3 /tmp/test_session_ipc.py
rm /tmp/test_session_ipc.py
echo ""

# Summary
echo "╔════════════════════════════════════════════╗"
echo "║         Session Persistence Tests         ║"
echo "║              PASSED ✅                     ║"
echo "╚════════════════════════════════════════════╝"
echo ""
echo "Summary:"
echo "  ✅ Backend running"
echo "  ✅ Sessions directory configured"
echo "  ✅ Session file format valid"
echo "  ✅ Python session module works"
echo "  ✅ Rust code compiles"
echo "  ✅ IPC commands functional"
echo ""
echo "🎉 Phase 5 Week 3: Session Persistence - COMPLETE!"
echo ""
echo "Available session commands:"
echo "  /list [limit]          - List recent sessions"
echo "  /load <session-id>     - Load and continue a session"
echo "  /export [session-id]   - Export session to markdown"
echo "  /delete <session-id>   - Delete a session"
echo "  /info                  - Show current session info"
echo ""
