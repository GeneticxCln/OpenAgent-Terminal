"""
Agent Handler - Manages AI agent queries and streaming responses.

For Phase 2, we'll start with a simple mock agent that demonstrates
the streaming architecture. This can be replaced with OpenAgent integration
once the streaming flow is working.
"""

import asyncio
import logging
from typing import AsyncIterator, Optional

from .block_formatter import BlockFormatter

logger = logging.getLogger(__name__)


class AgentHandler:
    """Handles agent queries and manages streaming responses."""

    def __init__(self):
        """Initialize the agent handler."""
        self.active_queries = {}  # query_id -> task
        self.block_formatter = BlockFormatter()
        logger.info("🤖 Agent handler initialized")

    async def query(
        self, query_id: str, message: str, context: Optional[dict] = None
    ) -> AsyncIterator[dict]:
        """
        Process an agent query and yield streaming tokens.

        Args:
            query_id: Unique identifier for this query
            message: User's query message
            context: Optional context (cwd, shell state, etc.)

        Yields:
            dict: Token notifications with content or tool requests
        """
        logger.info(f"🔍 Processing query {query_id}: {message[:50]}...")

        # For Phase 2/3, we'll use a simple mock agent
        # This demonstrates the streaming architecture
        response = await self._mock_agent_response(message, context)

        # Check if response is a tool request
        if response.startswith("__TOOL_REQUEST__"):
            # Parse tool request
            parts = response.split("__")[2:]  # Skip empty and TOOL_REQUEST
            if len(parts) >= 3:
                tool_name = parts[0]
                path = parts[1]
                content = parts[2]
                
                # Yield tool request
                yield {
                    "type": "tool_request",
                    "tool_name": tool_name,
                    "params": {
                        "path": path,
                        "content": content
                    },
                    "is_tool_request": True,
                }
                return

        # Stream the response token by token
        async for token_data in self._stream_tokens(response):
            yield token_data

        logger.info(f"✅ Query {query_id} complete")

    async def _mock_agent_response(
        self, message: str, context: Optional[dict] = None
    ) -> str:
        """
        Mock agent response for Phase 2 testing.
        
        In Phase 3, this will be replaced with actual OpenAgent integration.
        For now, it provides intelligent-looking responses to test streaming.
        """
        # Minimal delay just to yield control (removed 500ms artificial delay)
        await asyncio.sleep(0.01)

        # Generate contextual response based on keywords
        message_lower = message.lower()

        # Check for tool requests first (highest priority)
        if "test.txt" in message_lower or ("write" in message_lower and "hello" in message_lower):
            # This will trigger a tool request
            return "__TOOL_REQUEST__file_write__test.txt__Hello, World!\n\nThis is a test file created by OpenAgent-Terminal."

        elif "hello" in message_lower or "hi" in message_lower:
            return (
                "Hello! I'm the OpenAgent-Terminal AI assistant. "
                "I can help you with:\n"
                "• Running shell commands\n"
                "• Analyzing code\n"
                "• Debugging errors\n"
                "• Explaining concepts\n\n"
                "What would you like help with?"
            )

        elif "help" in message_lower:
            return (
                "I can assist you with:\n\n"
                "1. **Code Analysis**: Show me code and I'll explain it\n"
                "2. **Command Help**: Ask about shell commands\n"
                "3. **Debugging**: Share error messages for solutions\n"
                "4. **File Operations**: Help with reading/writing files\n\n"
                "Try asking me something specific!"
            )

        elif "error" in message_lower or "bug" in message_lower:
            return (
                "I'd be happy to help debug that! To provide the best assistance:\n\n"
                "1. Share the exact error message\n"
                "2. Show me the relevant code\n"
                "3. Tell me what you were trying to do\n\n"
                "Then I can suggest solutions and fixes."
            )

        elif "code" in message_lower or "function" in message_lower:
            return (
                "I can help with code! I can:\n"
                "• Explain how code works\n"
                "• Suggest improvements\n"
                "• Find potential bugs\n"
                "• Write new code for you\n\n"
                "Paste the code you'd like me to look at, or describe what you want to build."
            )

        elif any(word in message_lower for word in ["rust", "cargo", "tokio"]):
            return (
                "Rust development! Great choice. I can help with:\n\n"
                "```rust\n"
                "// Example: Async function\n"
                "async fn fetch_data() -> Result<String> {\n"
                "    let response = reqwest::get(\"https://api.example.com\")\n"
                "        .await?\n"
                "        .text()\n"
                "        .await?;\n"
                "    Ok(response)\n"
                "}\n"
                "```\n\n"
                "What specific Rust topic would you like help with?"
            )

        elif any(word in message_lower for word in ["file", "save"]):
            return (
                "I can help you work with files! \n\n"
                "For example, I could write code to a file. "
                "Would you like me to demonstrate the tool approval system?\n\n"
                "Try asking: 'write hello world to test.txt'"
            )

        elif any(word in message_lower for word in ["python", "django", "flask"]):
            return (
                "Python development! I can help with:\n\n"
                "```python\n"
                "# Example: Async function\n"
                "async def fetch_data(url: str) -> str:\n"
                "    async with aiohttp.ClientSession() as session:\n"
                "        async with session.get(url) as response:\n"
                "            return await response.text()\n"
                "```\n\n"
                "What Python topic would you like help with?"
            )

        else:
            # Generic response
            return (
                f"I received your query: '{message}'\n\n"
                "This is a Phase 2 demo response showing streaming token delivery. "
                "In the next phase, I'll be connected to a real LLM that can:\n"
                "• Understand complex queries\n"
                "• Execute tools and commands\n"
                "• Analyze code and files\n"
                "• Provide detailed explanations\n\n"
                "For now, try asking about:\n"
                "• 'hello' - Get a greeting\n"
                "• 'help' - See what I can do\n"
                "• 'rust' or 'python' - Get coding examples\n"
                "• 'error' - Get debugging help"
            )

    async def _stream_tokens(self, text: str) -> AsyncIterator[dict]:
        """
        Stream text as individual tokens or blocks with realistic timing.

        Args:
            text: Full text to stream

        Yields:
            dict: Token data with 'content' and 'type' fields, or block data
        """
        # Parse text into blocks
        blocks = self.block_formatter.format_response(text)

        for block in blocks:
            if block.type in ["code", "diff"]:
                # Send entire block at once with formatting
                yield {
                    "content": block.content,
                    "type": block.type,
                    "language": block.language or "text",
                    "is_block": True,
                }
                # Small delay after block
                await asyncio.sleep(0.2)
            else:
                # Stream text token by token
                words = block.content.split()
                for i, word in enumerate(words):
                    # Add space before all words except the first
                    token = word if i == 0 else f" {word}"

                    yield {"content": token, "type": "text"}

                    # Minimal delay just to yield control (reduced from 50-200ms)
                    await asyncio.sleep(0.01)

    async def cancel_query(self, query_id: str) -> bool:
        """
        Cancel an active query.

        Args:
            query_id: ID of query to cancel

        Returns:
            bool: True if cancelled, False if not found
        """
        if query_id in self.active_queries:
            task = self.active_queries[query_id]
            task.cancel()
            del self.active_queries[query_id]
            logger.info(f"🛑 Cancelled query {query_id}")
            return True
        return False

    def get_stats(self) -> dict:
        """Get agent statistics."""
        return {
            "active_queries": len(self.active_queries),
            "total_queries": 0,  # TODO: Track this
            "agent_type": "mock",  # Will be "openagent" later
            "status": "ready",
        }


# TODO: Phase 3 - Replace with real OpenAgent integration
# class OpenAgentHandler(AgentHandler):
#     """Production handler using OpenAgent."""
#
#     def __init__(self, config: dict):
#         super().__init__()
#         from openagent import Agent
#         self.agent = Agent(config)
#
#     async def query(self, query_id: str, message: str, context: dict = None):
#         # Use actual OpenAgent API
#         response = await self.agent.query(message, context=context)
#         async for token in response:
#             yield token
