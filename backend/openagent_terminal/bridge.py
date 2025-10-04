"""
Terminal Bridge - IPC Server

Unix domain socket server that handles JSON-RPC requests from Rust frontend
and routes them to OpenAgent core.
"""

import asyncio
import json
import logging
import os
import signal
import sys
import uuid
from pathlib import Path
from typing import Any, Dict, Optional

from .agent_handler import AgentHandler
from .session import Message, MessageRole, SessionManager
from .tool_handler import ToolHandler

logger = logging.getLogger(__name__)


class TerminalBridge:
    """IPC server bridge between Rust frontend and OpenAgent backend."""

    def __init__(self, socket_path: str | None = None, demo_mode: bool = True):
        """
        Initialize the terminal bridge.

        Args:
            socket_path: Path to Unix socket. If None, auto-generate.
            demo_mode: If True, tools run in safe demo mode. If False, real execution.
        """
        if socket_path is None:
            runtime_dir = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
            pid = os.getpid()
            socket_path = f"{runtime_dir}/openagent-terminal-{pid}.sock"

        self.socket_path = Path(socket_path)
        self.server = None
        self.running = False
        self.demo_mode = demo_mode
        self.agent_handler = AgentHandler()
        self.tool_handler = ToolHandler(demo_mode=demo_mode)
        self.session_manager = SessionManager()
        self.current_session = self.session_manager.create_session()
        self.active_streams = {}  # query_id -> writer for streaming responses
        
        logger.info(f"📝 Session created: {self.current_session.metadata.session_id}")

    async def start(self):
        """Start the IPC server."""
        logger.info(f"🚀 Starting Terminal Bridge at {self.socket_path}")

        # Remove old socket if exists
        if self.socket_path.exists():
            logger.info(f"Removing old socket at {self.socket_path}")
            self.socket_path.unlink()

        # Create Unix socket server
        self.server = await asyncio.start_unix_server(
            self.handle_connection, path=str(self.socket_path)
        )

        # Set permissions to 0600 (user-only)
        os.chmod(self.socket_path, 0o600)
        logger.info(f"✅ Socket permissions set to 0600")

        self.running = True
        logger.info(f"✅ Terminal Bridge ready at {self.socket_path}")
        logger.info("Waiting for connections...")

        # Start serving
        async with self.server:
            await self.server.serve_forever()

    async def handle_connection(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
        """Handle incoming connection from Rust frontend."""
        addr = writer.get_extra_info("peername")
        logger.info(f"📞 New connection from {addr}")

        try:
            while True:
                # Read newline-delimited JSON messages
                line = await reader.readline()
                if not line:
                    logger.info("Client disconnected")
                    break

                try:
                    message = line.decode("utf-8").strip()
                    logger.debug(f"📨 Received: {message}")

                    # Parse JSON-RPC message
                    data = json.loads(message)

                    # Check if it's a request (has 'id') or notification (no 'id')
                    if "id" in data:
                        response = await self.handle_request(data, writer)
                        # Send response
                        response_json = json.dumps(response) + "\n"
                        writer.write(response_json.encode("utf-8"))
                        await writer.drain()
                        logger.debug(f"📤 Sent: {response_json.strip()}")
                    else:
                        # It's a notification, handle but don't respond
                        await self.handle_notification(data)

                except json.JSONDecodeError as e:
                    logger.error(f"Failed to parse JSON: {e}")
                    error_response = self.create_error_response(
                        None, -32700, f"Parse error: {e}"
                    )
                    writer.write((json.dumps(error_response) + "\n").encode("utf-8"))
                    await writer.drain()
                except Exception as e:
                    logger.error(f"Error handling message: {e}", exc_info=True)

        except Exception as e:
            logger.error(f"Connection error: {e}", exc_info=True)
        finally:
            writer.close()
            await writer.wait_closed()
            logger.info("Connection closed")

    async def handle_request(self, request: dict, writer: asyncio.StreamWriter) -> dict:
        """Handle a JSON-RPC request and return response."""
        method = request.get("method")
        params = request.get("params", {})
        request_id = request.get("id")

        logger.info(f"🔧 Handling request: {method} (id={request_id})")

        try:
            # Route to appropriate handler
            if method == "initialize":
                result = await self.handle_initialize(params)
            elif method == "agent.query":
                # Agent query needs writer for streaming
                result = await self.handle_agent_query(params, request_id, writer)
            elif method == "agent.cancel":
                result = await self.handle_agent_cancel(params)
            elif method == "tool.approve":
                result = await self.handle_tool_approve(params)
            elif method == "session.list":
                result = await self.handle_session_list(params)
            elif method == "session.load":
                result = await self.handle_session_load(params)
            elif method == "session.export":
                result = await self.handle_session_export(params)
            elif method == "session.delete":
                result = await self.handle_session_delete(params)
            else:
                return self.create_error_response(
                    request_id, -32601, f"Method not found: {method}"
                )

            return self.create_response(request_id, result)

        except Exception as e:
            logger.error(f"Error in handler: {e}", exc_info=True)
            return self.create_error_response(
                request_id, -32603, f"Internal error: {str(e)}"
            )

    async def handle_notification(self, notification: dict):
        """Handle a JSON-RPC notification (no response needed)."""
        method = notification.get("method")
        params = notification.get("params", {})
        logger.info(f"📬 Notification: {method}")
        # TODO: Handle notifications like context.update

    async def handle_initialize(self, params: dict) -> dict:
        """Handle initialize request (see IPC_PROTOCOL.md)."""
        logger.info(f"Initialize request: {params}")
        logger.info(
            f"  Protocol version: {params.get('protocol_version')}")
        logger.info(f"  Client: {params.get('client_info')}")
        logger.info(
            f"  Terminal size: {params.get('terminal_size')}")
        logger.info(f"  Capabilities: {params.get('capabilities')}")

        return {
            "status": "ready",
            "server_info": {
                "name": "openagent-terminal-backend",
                "version": "0.1.0",
            },
            "capabilities": [
                "streaming",
                "blocks",
                "tool_execution",
            ],
        }

    async def handle_agent_query(self, params: dict, request_id: Any, writer: asyncio.StreamWriter) -> dict:
        """
        Handle agent.query request.
        
        This spawns a background task to stream tokens back to the client
        and immediately returns the query ID.
        """
        message = params.get("message", "")
        context = params.get("context", {})
        
        # Generate unique query ID
        query_id = str(uuid.uuid4())
        
        logger.info(f"🤖 Starting agent query {query_id}: {message[:50]}...")
        
        # Save user message to session
        from datetime import datetime
        user_msg = Message(
            role=MessageRole.USER,
            content=message,
            timestamp=datetime.now(),
            metadata={"query_id": query_id}
        )
        self.current_session.add_message(user_msg)
        self.session_manager.save_session(self.current_session)
        logger.debug(f"💾 Saved user message to session {self.current_session.metadata.session_id}")
        
        # Start streaming task in background
        task = asyncio.create_task(
            self._stream_agent_response(query_id, message, context, writer)
        )
        self.active_streams[query_id] = task
        
        # Return immediately with query ID
        return {
            "query_id": query_id,
            "status": "streaming",
        }

    async def _stream_agent_response(
        self, query_id: str, message: str, context: dict, writer: asyncio.StreamWriter
    ):
        """
        Background task that streams agent response tokens.
        """
        from datetime import datetime
        assistant_response_content = []  # Collect response for session
        
        try:
            # Get streaming response from agent
            async for token_data in self.agent_handler.query(query_id, message, context):
                # Check if it's a tool request
                if token_data.get("is_tool_request"):
                    # Request tool execution
                    execution_id = str(uuid.uuid4())
                    result = await self.tool_handler.request_tool_execution(
                        execution_id,
                        token_data["tool_name"],
                        token_data["params"]
                    )
                    
                    if result["status"] == "awaiting_approval":
                        # Send tool.request_approval notification
                        notification = {
                            "jsonrpc": "2.0",
                            "method": "tool.request_approval",
                            "params": {
                                "execution_id": execution_id,
                                "tool_name": result["tool_name"],
                                "description": result["description"],
                                "risk_level": result["risk_level"],
                                "preview": result["preview"],
                            },
                        }
                        logger.info(f"🔒 Tool approval request: {result['tool_name']}")
                    else:
                        # Tool executed directly (auto-approved)
                        notification = {
                            "jsonrpc": "2.0",
                            "method": "tool.result",
                            "params": {
                                "execution_id": execution_id,
                                "status": "success",
                                "result": result.get("result"),
                            },
                        }
                        logger.info(f"✅ Tool auto-executed: {token_data['tool_name']}")
                # Check if it's a block or regular token
                elif token_data.get("is_block"):
                    # Send stream.block notification
                    notification = {
                        "jsonrpc": "2.0",
                        "method": "stream.block",
                        "params": {
                            "query_id": query_id,
                            "type": token_data["type"],
                            "content": token_data["content"],
                            "language": token_data.get("language", "text"),
                        },
                    }
                    logger.info(f"📝 Block: {token_data['type']} ({token_data.get('language', 'text')})")
                else:
                    # Create stream.token notification
                    notification = {
                        "jsonrpc": "2.0",
                        "method": "stream.token",
                        "params": {
                            "query_id": query_id,
                            "content": token_data["content"],
                            "type": token_data.get("type", "text"),
                        },
                    }
                    logger.debug(f"📤 Token: {token_data['content'][:30]}...")
                    # Collect content for session
                    assistant_response_content.append(token_data["content"])
                
                # Send notification
                notification_json = json.dumps(notification) + "\n"
                writer.write(notification_json.encode("utf-8"))
                await writer.drain()
            
            # Send stream.complete notification
            complete_notification = {
                "jsonrpc": "2.0",
                "method": "stream.complete",
                "params": {
                    "query_id": query_id,
                    "status": "success",
                },
            }
            complete_json = json.dumps(complete_notification) + "\n"
            writer.write(complete_json.encode("utf-8"))
            await writer.drain()
            
            # Save assistant response to session
            if assistant_response_content:
                full_response = "".join(assistant_response_content)
                assistant_msg = Message(
                    role=MessageRole.ASSISTANT,
                    content=full_response,
                    timestamp=datetime.now(),
                    token_count=len(full_response.split()),  # Simple word count
                    metadata={"query_id": query_id}
                )
                self.current_session.add_message(assistant_msg)
                self.session_manager.save_session(self.current_session)
                logger.debug(f"💾 Saved assistant response ({len(full_response)} chars) to session")
            
            logger.info(f"✅ Query {query_id} complete")
            
        except asyncio.CancelledError:
            logger.info(f"🛑 Query {query_id} cancelled")
            # Send cancellation notification
            cancel_notification = {
                "jsonrpc": "2.0",
                "method": "stream.complete",
                "params": {
                    "query_id": query_id,
                    "status": "cancelled",
                },
            }
            cancel_json = json.dumps(cancel_notification) + "\n"
            writer.write(cancel_json.encode("utf-8"))
            await writer.drain()
        except Exception as e:
            logger.error(f"❌ Error in query {query_id}: {e}", exc_info=True)
            # Send error notification
            error_notification = {
                "jsonrpc": "2.0",
                "method": "stream.complete",
                "params": {
                    "query_id": query_id,
                    "status": "error",
                    "error": str(e),
                },
            }
            error_json = json.dumps(error_notification) + "\n"
            writer.write(error_json.encode("utf-8"))
            await writer.drain()
        finally:
            # Clean up
            if query_id in self.active_streams:
                del self.active_streams[query_id]

    async def handle_agent_cancel(self, params: dict) -> dict:
        """Handle agent.cancel request."""
        # TODO: Phase 2 implementation
        logger.info(f"Agent cancel (Phase 2 - not implemented): {params}")
        return {"status": "not_implemented"}

    async def handle_tool_approve(self, params: dict) -> dict:
        """Handle tool.approve request."""
        execution_id = params.get("execution_id")
        approved = params.get("approved", True)
        
        if not execution_id:
            return {"status": "error", "error": "execution_id required"}
        
        if approved:
            result = await self.tool_handler.approve_tool(execution_id)
        else:
            result = await self.tool_handler.reject_tool(execution_id)
        
        return result
    
    async def handle_session_list(self, params: dict) -> dict:
        """Handle session.list request."""
        limit = params.get("limit", 10)
        
        try:
            sessions = self.session_manager.list_sessions(limit=limit)
            return {
                "status": "success",
                "sessions": [
                    {
                        "session_id": s.session_id,
                        "title": s.title,
                        "created_at": s.created_at.isoformat(),
                        "updated_at": s.updated_at.isoformat(),
                        "message_count": s.message_count,
                        "total_tokens": s.total_tokens,
                    }
                    for s in sessions
                ]
            }
        except Exception as e:
            logger.error(f"Error listing sessions: {e}")
            return {"status": "error", "error": str(e)}
    
    async def handle_session_load(self, params: dict) -> dict:
        """Handle session.load request."""
        session_id = params.get("session_id")
        
        if not session_id:
            return {"status": "error", "error": "session_id required"}
        
        try:
            session = self.session_manager.load_session(session_id)
            if session:
                self.current_session = session
                logger.info(f"📂 Loaded session: {session_id} ({len(session.messages)} messages)")
                return {
                    "status": "success",
                    "session_id": session.metadata.session_id,
                    "message_count": len(session.messages),
                    "messages": [
                        {
                            "role": msg.role.value,
                            "content": msg.content,
                            "timestamp": msg.timestamp.isoformat(),
                            "token_count": msg.token_count,
                        }
                        for msg in session.messages
                    ]
                }
            else:
                return {"status": "error", "error": f"Session {session_id} not found"}
        except Exception as e:
            logger.error(f"Error loading session: {e}")
            return {"status": "error", "error": str(e)}
    
    async def handle_session_export(self, params: dict) -> dict:
        """Handle session.export request."""
        session_id = params.get("session_id")
        format_type = params.get("format", "markdown")
        
        try:
            # Use current session if no session_id specified
            if session_id:
                session = self.session_manager.load_session(session_id)
                if not session:
                    return {"status": "error", "error": f"Session {session_id} not found"}
            else:
                session = self.current_session
            
            if format_type == "markdown":
                content = self.session_manager.export_to_markdown(session)
                return {
                    "status": "success",
                    "format": "markdown",
                    "content": content
                }
            else:
                return {"status": "error", "error": f"Unsupported format: {format_type}"}
        except Exception as e:
            logger.error(f"Error exporting session: {e}")
            return {"status": "error", "error": str(e)}
    
    async def handle_session_delete(self, params: dict) -> dict:
        """Handle session.delete request."""
        session_id = params.get("session_id")
        
        if not session_id:
            return {"status": "error", "error": "session_id required"}
        
        try:
            success = self.session_manager.delete_session(session_id)
            if success:
                logger.info(f"🗑️  Deleted session: {session_id}")
                return {"status": "success", "deleted": session_id}
            else:
                return {"status": "error", "error": f"Failed to delete session {session_id}"}
        except Exception as e:
            logger.error(f"Error deleting session: {e}")
            return {"status": "error", "error": str(e)}

    def create_response(self, request_id: Any, result: Any) -> dict:
        """Create a JSON-RPC success response."""
        return {"jsonrpc": "2.0", "id": request_id, "result": result}

    def create_error_response(self, request_id: Any, code: int, message: str) -> dict:
        """Create a JSON-RPC error response."""
        return {
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {"code": code, "message": message},
        }

    async def stop(self):
        """Stop the IPC server and clean up."""
        logger.info("🛑 Stopping Terminal Bridge...")
        self.running = False

        if self.server:
            self.server.close()
            await self.server.wait_closed()

        if self.socket_path.exists():
            self.socket_path.unlink()
            logger.info(f"Removed socket at {self.socket_path}")

        logger.info("✅ Terminal Bridge stopped")


def main():
    """Entry point for bridge server."""
    import argparse
    
    parser = argparse.ArgumentParser(description="OpenAgent-Terminal Backend Bridge")
    parser.add_argument(
        "--socket", 
        type=str, 
        default=None,
        help="Path to Unix socket (default: auto-generate or use OPENAGENT_SOCKET env var)"
    )
    parser.add_argument(
        "--debug",
        action="store_true",
        help="Enable debug logging"
    )
    parser.add_argument(
        "--execute",
        action="store_true",
        help="Enable REAL file operations (⚠️  WARNING: modifies file system!)"
    )
    args = parser.parse_args()
    
    # Set up logging
    log_level = logging.DEBUG if args.debug else logging.INFO
    logging.basicConfig(
        level=log_level, 
        format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
    )

    print("╔════════════════════════════════════════════╗")
    print("║  OpenAgent-Terminal Backend (Python)      ║")
    print("║  IPC Bridge Server                         ║")
    print("╚════════════════════════════════════════════╝")
    print()
    print("✅  Phase 1: Foundation - IPC Server Ready")
    
    # Show execution mode
    if args.execute:
        print("⚠️  REAL EXECUTION MODE - Tools will modify file system!")
        demo_mode = False
    else:
        print("🔒 Demo mode - Tools will simulate execution (use --execute for real ops)")
        demo_mode = True
    
    print("📦 Starting Unix socket server...")
    print()

    # Determine socket path - check env var, then arg, then default
    socket_path = args.socket or os.environ.get("OPENAGENT_SOCKET")
    if socket_path is None:
        # Use test socket for now
        runtime_dir = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
        socket_path = f"{runtime_dir}/openagent-terminal-test.sock"
        print(f"📦 Using test socket: {socket_path}")
    
    bridge = TerminalBridge(socket_path, demo_mode=demo_mode)
    
    # Handle Ctrl+C gracefully
    async def run_server():
        try:
            await bridge.start()
        except KeyboardInterrupt:
            print("\n\n⚠️  Interrupted by user")
        finally:
            await bridge.stop()
    
    try:
        asyncio.run(run_server())
    except KeyboardInterrupt:
        print("\nShutdown complete.")


if __name__ == "__main__":
    main()
