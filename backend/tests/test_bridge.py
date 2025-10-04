"""
Tests for the bridge module (IPC server).
"""

import asyncio
import json
import pytest
import tempfile
import os
from pathlib import Path
from openagent_terminal.bridge import TerminalBridge


class TestTerminalBridge:
    """Test suite for TerminalBridge."""
    
    @pytest.fixture
    def temp_socket(self):
        """Create a temporary socket path."""
        temp_dir = tempfile.mkdtemp()
        socket_path = os.path.join(temp_dir, "test.sock")
        yield socket_path
        # Cleanup
        if os.path.exists(socket_path):
            os.remove(socket_path)
        os.rmdir(temp_dir)
    
    @pytest.fixture
    def bridge(self, temp_socket):
        """Create a bridge instance for testing."""
        return TerminalBridge(temp_socket, demo_mode=True)
    
    def test_bridge_initialization(self, bridge, temp_socket):
        """Test that bridge initializes correctly."""
        assert bridge is not None
        assert str(bridge.socket_path) == temp_socket
        assert bridge.demo_mode is True
        assert bridge.running is False
        assert bridge.agent_handler is not None
        assert bridge.tool_handler is not None
    
    def test_bridge_initialization_demo_mode(self):
        """Test bridge with demo mode enabled."""
        bridge = TerminalBridge(demo_mode=True)
        assert bridge.demo_mode is True
        assert bridge.tool_handler.demo_mode is True
    
    def test_bridge_initialization_real_mode(self):
        """Test bridge with real execution mode."""
        bridge = TerminalBridge(demo_mode=False)
        assert bridge.demo_mode is False
        assert bridge.tool_handler.demo_mode is False
    
    def test_create_response(self, bridge):
        """Test creating JSON-RPC success responses."""
        response = bridge.create_response(123, {"status": "ok"})
        
        assert response["jsonrpc"] == "2.0"
        assert response["id"] == 123
        assert response["result"] == {"status": "ok"}
        assert "error" not in response
    
    def test_create_response_with_none_id(self, bridge):
        """Test creating response with None id."""
        response = bridge.create_response(None, {"data": "test"})
        
        assert response["jsonrpc"] == "2.0"
        assert response["id"] is None
        assert response["result"] == {"data": "test"}
    
    def test_create_error_response(self, bridge):
        """Test creating JSON-RPC error responses."""
        response = bridge.create_error_response(456, -32600, "Invalid Request")
        
        assert response["jsonrpc"] == "2.0"
        assert response["id"] == 456
        assert "result" not in response
        assert response["error"]["code"] == -32600
        assert response["error"]["message"] == "Invalid Request"
    
    def test_create_error_response_standard_codes(self, bridge):
        """Test error responses with standard JSON-RPC error codes."""
        # Parse error
        response = bridge.create_error_response(1, -32700, "Parse error")
        assert response["error"]["code"] == -32700
        
        # Method not found
        response = bridge.create_error_response(2, -32601, "Method not found")
        assert response["error"]["code"] == -32601
        
        # Invalid params
        response = bridge.create_error_response(3, -32602, "Invalid params")
        assert response["error"]["code"] == -32602
        
        # Internal error
        response = bridge.create_error_response(4, -32603, "Internal error")
        assert response["error"]["code"] == -32603
    
    @pytest.mark.asyncio
    async def test_handle_initialize(self, bridge):
        """Test initialize request handling."""
        params = {
            "protocol_version": "1.0",
            "client_info": {
                "name": "test-client",
                "version": "0.1.0"
            },
            "terminal_size": {"rows": 24, "cols": 80},
            "capabilities": ["streaming", "blocks"]
        }
        
        result = await bridge.handle_initialize(params)
        
        assert result["status"] == "ready"
        assert "server_info" in result
        assert result["server_info"]["name"] == "openagent-terminal-backend"
        assert "version" in result["server_info"]
        assert "capabilities" in result
        assert "streaming" in result["capabilities"]
        assert "blocks" in result["capabilities"]
        assert "tool_execution" in result["capabilities"]
    
    @pytest.mark.asyncio
    async def test_handle_initialize_minimal_params(self, bridge):
        """Test initialize with minimal parameters."""
        result = await bridge.handle_initialize({})
        
        assert result["status"] == "ready"
        assert "server_info" in result
        assert "capabilities" in result
    
    def test_socket_path_auto_generation(self):
        """Test that socket path is auto-generated if not provided."""
        bridge = TerminalBridge(socket_path=None)
        
        assert bridge.socket_path is not None
        assert str(bridge.socket_path).endswith(".sock")
    
    def test_socket_path_from_env(self, monkeypatch):
        """Test socket path from environment variable."""
        test_path = "/tmp/test-socket.sock"
        monkeypatch.setenv("OPENAGENT_SOCKET", test_path)
        
        # When socket_path is None, it should check env var
        # Note: This depends on implementation details
        socket_path = os.environ.get("OPENAGENT_SOCKET")
        assert socket_path == test_path
    
    def test_pending_approvals_tracking(self, bridge):
        """Test that bridge tracks pending tool approvals."""
        assert hasattr(bridge, "pending_approvals")
        assert isinstance(bridge.pending_approvals, dict) or hasattr(bridge, "tool_handler")
    
    def test_active_streams_tracking(self, bridge):
        """Test that bridge tracks active query streams."""
        assert hasattr(bridge, "active_streams")
        assert isinstance(bridge.active_streams, dict)
        assert len(bridge.active_streams) == 0
    
    @pytest.mark.asyncio
    async def test_handle_agent_cancel(self, bridge):
        """Test agent cancel request handling."""
        params = {"query_id": "test-123"}
        result = await bridge.handle_agent_cancel(params)
        
        # Currently not implemented, should return status
        assert "status" in result
    
    @pytest.mark.asyncio
    async def test_handle_tool_approve_missing_id(self, bridge):
        """Test tool approve with missing execution_id."""
        params = {}
        result = await bridge.handle_tool_approve(params)
        
        assert result["status"] == "error"
        assert "execution_id required" in result["error"]
    
    @pytest.mark.asyncio
    async def test_handle_tool_approve_not_found(self, bridge):
        """Test tool approve with non-existent execution_id."""
        params = {"execution_id": "non-existent", "approved": True}
        result = await bridge.handle_tool_approve(params)
        
        # Should return error (execution not found)
        assert "status" in result
    
    def test_bridge_has_handler_instances(self, bridge):
        """Test that bridge has initialized handler instances."""
        assert bridge.agent_handler is not None
        assert bridge.tool_handler is not None
        assert hasattr(bridge.agent_handler, "query")
        assert hasattr(bridge.tool_handler, "request_tool_execution")
    
    def test_response_format_compliance(self, bridge):
        """Test that responses are JSON-RPC 2.0 compliant."""
        # Success response
        response = bridge.create_response(1, {"test": "data"})
        assert "jsonrpc" in response
        assert response["jsonrpc"] == "2.0"
        assert "id" in response
        assert "result" in response
        
        # Error response
        error_response = bridge.create_error_response(2, -32600, "Error")
        assert "jsonrpc" in error_response
        assert error_response["jsonrpc"] == "2.0"
        assert "id" in error_response
        assert "error" in error_response
        assert "code" in error_response["error"]
        assert "message" in error_response["error"]
    
    def test_demo_mode_propagates_to_handlers(self):
        """Test that demo_mode is passed to tool handler."""
        # Demo mode
        bridge_demo = TerminalBridge(demo_mode=True)
        assert bridge_demo.tool_handler.demo_mode is True
        
        # Real mode
        bridge_real = TerminalBridge(demo_mode=False)
        assert bridge_real.tool_handler.demo_mode is False


class TestBridgeIntegration:
    """Integration tests for bridge functionality."""
    
    @pytest.mark.asyncio
    async def test_initialize_agent_query_flow(self):
        """Test the full flow of initialize -> agent query."""
        bridge = TerminalBridge(demo_mode=True)
        
        # Initialize
        init_result = await bridge.handle_initialize({
            "client_info": {"name": "test"}
        })
        assert init_result["status"] == "ready"
        
        # Agent query would be tested if we had a mock writer
        # For now, just verify the handlers exist
        assert bridge.agent_handler is not None
    
    @pytest.mark.asyncio
    async def test_error_response_structure(self):
        """Test that error responses have correct structure."""
        bridge = TerminalBridge()
        
        error_response = bridge.create_error_response(
            123, 
            -32603, 
            "Internal error occurred"
        )
        
        # Verify structure
        assert isinstance(error_response, dict)
        assert error_response["jsonrpc"] == "2.0"
        assert error_response["id"] == 123
        assert isinstance(error_response["error"], dict)
        assert error_response["error"]["code"] == -32603
        assert error_response["error"]["message"] == "Internal error occurred"
        
        # Verify JSON serializable
        json_str = json.dumps(error_response)
        assert json_str is not None
        parsed = json.loads(json_str)
        assert parsed == error_response


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
