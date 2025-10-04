"""
Unit tests for tool handler.

Tests tool execution, safety checks, approval flow, and error handling.
"""

import pytest
import asyncio
import os
import tempfile
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).parent.parent))

from openagent_terminal.tool_handler import (
    ToolHandler,
    Tool,
    RiskLevel,
    ToolExecution,
)


class TestToolHandler:
    """Test cases for ToolHandler"""
    
    def test_init_demo_mode(self):
        """Test initialization in demo mode"""
        handler = ToolHandler(demo_mode=True)
        assert handler.demo_mode is True
        assert len(handler.tools) == 5  # 5 tools should be registered
        
    def test_init_real_mode(self):
        """Test initialization in real mode"""
        handler = ToolHandler(demo_mode=False)
        assert handler.demo_mode is False
        
    def test_register_tools(self):
        """Test that all tools are registered correctly"""
        handler = ToolHandler()
        
        # Check all expected tools exist
        assert "file_read" in handler.tools
        assert "file_write" in handler.tools
        assert "file_delete" in handler.tools
        assert "shell_command" in handler.tools
        assert "directory_list" in handler.tools
        
        # Check risk levels
        assert handler.tools["file_read"].risk_level == RiskLevel.LOW
        assert handler.tools["file_write"].risk_level == RiskLevel.MEDIUM
        assert handler.tools["file_delete"].risk_level == RiskLevel.HIGH
        
    @pytest.mark.asyncio
    async def test_low_risk_tool_auto_execute(self):
        """Test that low-risk tools execute without approval"""
        handler = ToolHandler(demo_mode=True)
        
        # file_read should execute without approval
        result = await handler.request_tool_execution(
            "test-123",
            "file_read",
            {"path": "test.txt"}
        )
        
        assert result["status"] == "executed"
        assert "result" in result
        
    @pytest.mark.asyncio
    async def test_high_risk_tool_requires_approval(self):
        """Test that high-risk tools require approval"""
        handler = ToolHandler(demo_mode=True)
        
        # file_write should require approval
        result = await handler.request_tool_execution(
            "test-456",
            "file_write",
            {"path": "test.txt", "content": "hello"}
        )
        
        assert result["status"] == "awaiting_approval"
        assert result["tool_name"] == "file_write"
        assert result["risk_level"] == "medium"
        assert "preview" in result
        
    @pytest.mark.asyncio
    async def test_approval_flow(self):
        """Test tool approval workflow"""
        handler = ToolHandler(demo_mode=True)
        
        # Request tool execution
        result = await handler.request_tool_execution(
            "test-789",
            "file_write",
            {"path": "test.txt", "content": "hello"}
        )
        
        assert result["status"] == "awaiting_approval"
        execution_id = result["execution_id"]
        
        # Approve the tool
        approval_result = await handler.approve_tool(execution_id)
        
        assert approval_result["status"] == "executed"
        assert "result" in approval_result
        
    @pytest.mark.asyncio
    async def test_rejection_flow(self):
        """Test tool rejection workflow"""
        handler = ToolHandler(demo_mode=True)
        
        # Request tool execution
        result = await handler.request_tool_execution(
            "test-reject",
            "file_delete",
            {"path": "test.txt"}
        )
        
        assert result["status"] == "awaiting_approval"
        execution_id = result["execution_id"]
        
        # Reject the tool
        reject_result = await handler.reject_tool(execution_id)
        
        assert reject_result["status"] == "rejected"
        
    @pytest.mark.asyncio
    async def test_unknown_tool(self):
        """Test handling of unknown tool"""
        handler = ToolHandler()
        
        result = await handler.request_tool_execution(
            "test-unknown",
            "nonexistent_tool",
            {}
        )
        
        assert result["status"] == "error"
        assert "Unknown tool" in result["error"]
        
    def test_preview_generation_file_write(self):
        """Test preview generation for file_write"""
        handler = ToolHandler()
        tool = handler.tools["file_write"]
        
        preview = handler._generate_preview(tool, {
            "path": "/tmp/test.txt",
            "content": "Hello World! " * 20
        })
        
        assert "/tmp/test.txt" in preview
        assert "Hello World!" in preview
        
    def test_preview_generation_file_delete(self):
        """Test preview generation for file_delete"""
        handler = ToolHandler()
        tool = handler.tools["file_delete"]
        
        preview = handler._generate_preview(tool, {
            "path": "/tmp/test.txt"
        })
        
        assert "/tmp/test.txt" in preview
        assert "cannot be undone" in preview.lower()
        
    def test_safe_path_validation(self):
        """Test path safety validation"""
        handler = ToolHandler()
        
        # Current directory should be safe
        assert handler._is_safe_path("test.txt")
        assert handler._is_safe_path("./test.txt")
        
        # Home directory should be safe
        home = os.path.expanduser("~")
        assert handler._is_safe_path(os.path.join(home, "test.txt"))
        
        # System directories should not be safe
        assert not handler._is_safe_path("/etc/passwd")
        assert not handler._is_safe_path("/sys/test")
        assert not handler._is_safe_path("/proc/test")
        
    @pytest.mark.asyncio
    async def test_demo_mode_execution(self):
        """Test that demo mode doesn't actually execute"""
        handler = ToolHandler(demo_mode=True)
        
        # Request and approve file write in demo mode
        result = await handler.request_tool_execution(
            "demo-test",
            "file_write",
            {"path": "demo_test.txt", "content": "test"}
        )
        
        execution_id = result["execution_id"]
        exec_result = await handler.approve_tool(execution_id)
        
        # Should return success but with demo note
        assert exec_result["status"] == "executed"
        assert "note" in exec_result["result"]
        assert "Demo mode" in exec_result["result"]["note"]
        
        # File should NOT exist
        assert not os.path.exists("demo_test.txt")
        
    @pytest.mark.asyncio
    async def test_real_mode_file_write(self):
        """Test real file write in real execution mode"""
        with tempfile.TemporaryDirectory() as tmpdir:
            handler = ToolHandler(demo_mode=False)
            test_file = os.path.join(tmpdir, "real_test.txt")
            test_content = "Real execution test"
            
            # Request and approve file write
            result = await handler.request_tool_execution(
                "real-test",
                "file_write",
                {"path": test_file, "content": test_content}
            )
            
            exec_result = await handler.approve_tool(result["execution_id"])
            
            # File should actually exist
            assert os.path.exists(test_file)
            
            # Content should match
            with open(test_file, 'r') as f:
                assert f.read() == test_content
                
    @pytest.mark.asyncio
    async def test_real_mode_safety_blocks_system_paths(self):
        """Test that safety checks block system paths even in real mode"""
        handler = ToolHandler(demo_mode=False)
        
        # Try to write to /etc (should be blocked by safety check)
        result = await handler.request_tool_execution(
            "unsafe-test",
            "file_write",
            {"path": "/etc/test.txt", "content": "bad"}
        )
        
        # Approve it
        exec_result = await handler.approve_tool(result["execution_id"])
        
        # Should fail with safety error
        assert exec_result["status"] == "error" or \
               (exec_result["status"] == "executed" and not exec_result["result"]["success"])


if __name__ == "__main__":
    # Run tests
    pytest.main([__file__, "-v"])
