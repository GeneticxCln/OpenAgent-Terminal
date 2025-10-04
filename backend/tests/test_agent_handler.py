"""
Tests for the agent handler module.
"""

import asyncio
import pytest
from openagent_terminal.agent_handler import AgentHandler


class TestAgentHandler:
    """Test suite for AgentHandler."""
    
    @pytest.fixture
    def handler(self):
        """Create an agent handler for testing."""
        return AgentHandler()
    
    def test_handler_initialization(self, handler):
        """Test that the handler initializes correctly."""
        assert handler is not None
        assert handler.active_queries == {}
        assert handler.block_formatter is not None
    
    @pytest.mark.asyncio
    async def test_simple_query(self, handler):
        """Test a simple query returns tokens."""
        tokens = []
        async for token_data in handler.query("q1", "hello", {}):
            tokens.append(token_data)
        
        # Should have received some tokens
        assert len(tokens) > 0
        
        # Tokens should have content
        for token in tokens:
            assert "content" in token
    
    @pytest.mark.asyncio
    async def test_greeting_response(self, handler):
        """Test that greeting queries get appropriate responses."""
        response_text = ""
        async for token_data in handler.query("q2", "hello", {}):
            if not token_data.get("is_block") and not token_data.get("is_tool_request"):
                response_text += token_data.get("content", "")
        
        # Response should mention AI assistant
        assert "assistant" in response_text.lower()
    
    @pytest.mark.asyncio
    async def test_help_response(self, handler):
        """Test that help queries provide assistance."""
        response_text = ""
        async for token_data in handler.query("q3", "help", {}):
            if not token_data.get("is_block") and not token_data.get("is_tool_request"):
                response_text += token_data.get("content", "")
        
        # Response should mention what the AI can do
        assert "assist" in response_text.lower() or "help" in response_text.lower()
    
    @pytest.mark.asyncio
    async def test_code_block_response(self, handler):
        """Test that code queries return code blocks."""
        blocks = []
        async for token_data in handler.query("q4", "show me rust code", {}):
            if token_data.get("is_block"):
                blocks.append(token_data)
        
        # Should have at least one code block
        assert len(blocks) > 0
        
        # Block should have language and content
        block = blocks[0]
        assert block.get("type") == "code"
        assert "language" in block
        assert "content" in block
        assert len(block["content"]) > 0
    
    @pytest.mark.asyncio
    async def test_tool_request(self, handler):
        """Test that file write requests trigger tool execution."""
        tool_requests = []
        async for token_data in handler.query("q5", "write hello world to test.txt", {}):
            if token_data.get("is_tool_request"):
                tool_requests.append(token_data)
        
        # Should have a tool request
        assert len(tool_requests) > 0
        
        # Tool request should have proper structure
        tool_req = tool_requests[0]
        assert tool_req["tool_name"] == "file_write"
        assert "params" in tool_req
        assert "path" in tool_req["params"]
        assert "content" in tool_req["params"]
    
    @pytest.mark.asyncio
    async def test_token_timing(self, handler):
        """Test that tokens arrive with realistic timing."""
        import time
        
        start_time = time.time()
        token_count = 0
        
        async for token_data in handler.query("q6", "hello", {}):
            token_count += 1
            if token_count >= 5:  # Just check first few tokens
                break
        
        elapsed = time.time() - start_time
        
        # Should take some time (realistic streaming)
        assert elapsed > 0.1  # At least 100ms for 5 tokens
        assert elapsed < 5.0  # But not too slow
    
    def test_get_stats(self, handler):
        """Test that stats are returned correctly."""
        stats = handler.get_stats()
        
        assert "active_queries" in stats
        assert "agent_type" in stats
        assert "status" in stats
        assert stats["agent_type"] == "mock"
        assert stats["status"] == "ready"
    
    @pytest.mark.asyncio
    async def test_query_with_context(self, handler):
        """Test that queries can include context."""
        context = {
            "cwd": "/home/user/project",
            "shell": "zsh"
        }
        
        response_text = ""
        async for token_data in handler.query("q7", "what is my current directory?", context):
            if not token_data.get("is_block") and not token_data.get("is_tool_request"):
                response_text += token_data.get("content", "")
        
        # Should get some response
        assert len(response_text) > 0
    
    @pytest.mark.asyncio
    async def test_different_query_types(self, handler):
        """Test various query types get appropriate responses."""
        queries = [
            ("hello", "greeting"),
            ("help me", "help"),
            ("what's rust?", "rust"),
            ("show me python", "python"),
            ("I have an error", "error")
        ]
        
        for query, expected_keyword in queries:
            response_text = ""
            async for token_data in handler.query(f"q_{expected_keyword}", query, {}):
                if not token_data.get("is_block") and not token_data.get("is_tool_request"):
                    response_text += token_data.get("content", "")
            
            # Response should not be empty
            assert len(response_text) > 0, f"Query '{query}' got empty response"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
