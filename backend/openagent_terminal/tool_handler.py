"""
Tool Handler - Manages tool execution with approval flow.

Phase 4: Implements mock tools and approval workflow.
Later: Will integrate with OpenAgent's real tool system.
"""

import asyncio
import logging
import os
import subprocess
from dataclasses import dataclass
from enum import Enum
from typing import Optional, Dict, Any

logger = logging.getLogger(__name__)


class RiskLevel(Enum):
    """Risk levels for tool operations."""
    LOW = "low"        # Read-only operations
    MEDIUM = "medium"  # Write operations
    HIGH = "high"      # Destructive or system operations
    CRITICAL = "critical"  # Dangerous operations


@dataclass
class Tool:
    """Represents a tool that can be executed."""
    name: str
    description: str
    risk_level: RiskLevel
    requires_approval: bool = True


@dataclass
class ToolExecution:
    """Represents a tool execution request."""
    id: str
    tool: Tool
    params: Dict[str, Any]
    approved: Optional[bool] = None
    result: Optional[Any] = None
    error: Optional[str] = None


class ToolHandler:
    """Handles tool execution with approval flow."""

    def __init__(self, demo_mode: bool = True):
        """Initialize the tool handler.
        
        Args:
            demo_mode: If True, tools simulate execution without side effects.
                      If False, tools perform real operations.
        """
        self.demo_mode = demo_mode
        self.pending_approvals: Dict[str, ToolExecution] = {}
        self.tools = self._register_tools()
        mode_str = "demo" if demo_mode else "REAL EXECUTION"
        logger.info(f"🔧 Tool handler initialized with {len(self.tools)} tools (mode: {mode_str})")

    def _register_tools(self) -> Dict[str, Tool]:
        """Register available tools."""
        return {
            "file_read": Tool(
                name="file_read",
                description="Read contents of a file",
                risk_level=RiskLevel.LOW,
                requires_approval=False,  # Low risk
            ),
            "file_write": Tool(
                name="file_write",
                description="Write content to a file",
                risk_level=RiskLevel.MEDIUM,
                requires_approval=True,
            ),
            "file_delete": Tool(
                name="file_delete",
                description="Delete a file",
                risk_level=RiskLevel.HIGH,
                requires_approval=True,
            ),
            "shell_command": Tool(
                name="shell_command",
                description="Execute a shell command",
                risk_level=RiskLevel.HIGH,
                requires_approval=True,
            ),
            "directory_list": Tool(
                name="directory_list",
                description="List files in a directory",
                risk_level=RiskLevel.LOW,
                requires_approval=False,
            ),
        }

    async def request_tool_execution(
        self, execution_id: str, tool_name: str, params: Dict[str, Any]
    ) -> Dict[str, Any]:
        """
        Request execution of a tool.

        Returns approval request or executes directly if no approval needed.
        """
        if tool_name not in self.tools:
            return {
                "status": "error",
                "error": f"Unknown tool: {tool_name}"
            }

        tool = self.tools[tool_name]
        execution = ToolExecution(
            id=execution_id,
            tool=tool,
            params=params
        )

        if tool.requires_approval:
            # Store for approval
            self.pending_approvals[execution_id] = execution
            logger.info(f"🔒 Tool {tool_name} requires approval (execution_id={execution_id})")

            # Generate preview
            preview = self._generate_preview(tool, params)

            return {
                "status": "awaiting_approval",
                "execution_id": execution_id,
                "tool_name": tool_name,
                "description": tool.description,
                "risk_level": tool.risk_level.value,
                "preview": preview,
            }
        else:
            # Execute directly
            logger.info(f"✅ Tool {tool_name} auto-approved (low risk)")
            result = await self._execute_tool(tool, params)
            return {
                "status": "executed",
                "execution_id": execution_id,
                "result": result,
            }

    def _generate_preview(self, tool: Tool, params: Dict[str, Any]) -> str:
        """Generate a preview of what the tool will do."""
        if tool.name == "file_write":
            path = params.get("path", "unknown")
            content_preview = params.get("content", "")[:100]
            return f"Write to file: {path}\nContent preview:\n{content_preview}..."

        elif tool.name == "file_delete":
            path = params.get("path", "unknown")
            return f"Delete file: {path}\n⚠️  This action cannot be undone!"

        elif tool.name == "shell_command":
            command = params.get("command", "unknown")
            return f"Execute command:\n$ {command}\n\n⚠️  Shell commands can modify your system"

        else:
            return f"Execute {tool.name} with params:\n{params}"

    async def approve_tool(self, execution_id: str) -> Dict[str, Any]:
        """Approve a pending tool execution."""
        if execution_id not in self.pending_approvals:
            return {"status": "error", "error": "Execution not found"}

        execution = self.pending_approvals[execution_id]
        execution.approved = True

        logger.info(f"✅ Tool {execution.tool.name} approved (execution_id={execution_id})")

        # Execute the tool
        try:
            result = await self._execute_tool(execution.tool, execution.params)
            execution.result = result

            # Clean up
            del self.pending_approvals[execution_id]

            return {
                "status": "executed",
                "result": result,
            }
        except Exception as e:
            logger.error(f"❌ Tool execution failed: {e}")
            execution.error = str(e)
            del self.pending_approvals[execution_id]

            return {
                "status": "error",
                "error": str(e),
            }

    async def reject_tool(self, execution_id: str) -> Dict[str, Any]:
        """Reject a pending tool execution."""
        if execution_id not in self.pending_approvals:
            return {"status": "error", "error": "Execution not found"}

        execution = self.pending_approvals[execution_id]
        logger.info(f"❌ Tool {execution.tool.name} rejected (execution_id={execution_id})")

        del self.pending_approvals[execution_id]

        return {
            "status": "rejected",
            "message": "Tool execution rejected by user",
        }

    async def _execute_tool(self, tool: Tool, params: Dict[str, Any]) -> Dict[str, Any]:
        """
        Execute a tool with given parameters.
        
        Delegates to either demo or real execution based on mode.
        """
        logger.info(f"🔧 Executing tool: {tool.name} (demo_mode={self.demo_mode})")
        
        if self.demo_mode:
            return await self._execute_tool_demo(tool, params)
        else:
            return await self._execute_tool_real(tool, params)
    
    async def _execute_tool_demo(self, tool: Tool, params: Dict[str, Any]) -> Dict[str, Any]:
        """
        Execute a tool in demo mode (safe, no side effects).
        """

        # Simulate execution time
        await asyncio.sleep(0.5)

        if tool.name == "file_read":
            path = params.get("path", "")
            try:
                # For demo, read first 500 chars
                if os.path.exists(path):
                    with open(path, 'r') as f:
                        content = f.read(500)
                    return {
                        "success": True,
                        "content": content,
                        "message": f"Read {len(content)} characters from {path}"
                    }
                else:
                    return {
                        "success": False,
                        "error": f"File not found: {path}"
                    }
            except Exception as e:
                return {"success": False, "error": str(e)}

        elif tool.name == "file_write":
            path = params.get("path", "")
            content = params.get("content", "")
            return {
                "success": True,
                "message": f"Would write {len(content)} bytes to {path}",
                "note": "Demo mode - file not actually written"
            }

        elif tool.name == "file_delete":
            path = params.get("path", "")
            return {
                "success": True,
                "message": f"Would delete {path}",
                "note": "Demo mode - file not actually deleted"
            }

        elif tool.name == "shell_command":
            command = params.get("command", "")
            # For safety, only allow a whitelist of safe commands in demo
            safe_commands = ["ls", "pwd", "date", "whoami"]
            cmd_name = command.split()[0] if command else ""

            if cmd_name in safe_commands:
                try:
                    result = subprocess.run(
                        command,
                        shell=True,
                        capture_output=True,
                        text=True,
                        timeout=5
                    )
                    return {
                        "success": result.returncode == 0,
                        "stdout": result.stdout,
                        "stderr": result.stderr,
                        "returncode": result.returncode,
                    }
                except Exception as e:
                    return {"success": False, "error": str(e)}
            else:
                return {
                    "success": True,
                    "message": f"Would execute: {command}",
                    "note": "Demo mode - only safe commands actually executed"
                }

        elif tool.name == "directory_list":
            path = params.get("path", ".")
            try:
                files = os.listdir(path)[:20]  # Limit to 20 files
                return {
                    "success": True,
                    "files": files,
                    "count": len(files),
                    "message": f"Listed {len(files)} files in {path}"
                }
            except Exception as e:
                return {"success": False, "error": str(e)}

        else:
            return {
                "success": False,
                "error": f"Tool {tool.name} not implemented"
            }
    
    async def _execute_tool_real(self, tool: Tool, params: Dict[str, Any]) -> Dict[str, Any]:
        """
        Execute a tool with REAL side effects.
        
        ⚠️  WARNING: This actually modifies the file system!
        """
        # Small delay for realism
        await asyncio.sleep(0.1)
        
        if tool.name == "file_read":
            path = params.get("path", "")
            try:
                # Safety: Check if path is safe
                if not self._is_safe_path(path):
                    return {
                        "success": False,
                        "error": f"Access denied: {path} is not in a safe directory"
                    }
                
                if os.path.exists(path):
                    with open(path, 'r') as f:
                        content = f.read()
                    return {
                        "success": True,
                        "content": content,
                        "message": f"Read {len(content)} bytes from {path}"
                    }
                else:
                    return {
                        "success": False,
                        "error": f"File not found: {path}"
                    }
            except Exception as e:
                return {"success": False, "error": str(e)}
        
        elif tool.name == "file_write":
            path = params.get("path", "")
            content = params.get("content", "")
            
            try:
                # Safety: Check if path is safe
                if not self._is_safe_path(path):
                    return {
                        "success": False,
                        "error": f"Access denied: {path} is not in a safe directory"
                    }
                
                # Create directory if it doesn't exist
                os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
                
                # Write the file
                with open(path, 'w') as f:
                    f.write(content)
                
                return {
                    "success": True,
                    "message": f"Successfully wrote {len(content)} bytes to {path}",
                    "path": os.path.abspath(path)
                }
            except Exception as e:
                return {"success": False, "error": str(e)}
        
        elif tool.name == "file_delete":
            path = params.get("path", "")
            
            try:
                # Safety: Check if path is safe
                if not self._is_safe_path(path):
                    return {
                        "success": False,
                        "error": f"Access denied: {path} is not in a safe directory"
                    }
                
                if os.path.exists(path):
                    os.remove(path)
                    return {
                        "success": True,
                        "message": f"Successfully deleted {path}"
                    }
                else:
                    return {
                        "success": False,
                        "error": f"File not found: {path}"
                    }
            except Exception as e:
                return {"success": False, "error": str(e)}
        
        elif tool.name == "shell_command":
            command = params.get("command", "")
            
            try:
                # Execute the command with timeout
                result = subprocess.run(
                    command,
                    shell=True,
                    capture_output=True,
                    text=True,
                    timeout=10  # 10 second timeout
                )
                
                return {
                    "success": result.returncode == 0,
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                    "returncode": result.returncode,
                    "command": command
                }
            except subprocess.TimeoutExpired:
                return {
                    "success": False,
                    "error": f"Command timed out after 10 seconds: {command}"
                }
            except Exception as e:
                return {"success": False, "error": str(e)}
        
        elif tool.name == "directory_list":
            path = params.get("path", ".")
            
            try:
                # Safety: Check if path is safe
                if not self._is_safe_path(path):
                    return {
                        "success": False,
                        "error": f"Access denied: {path} is not in a safe directory"
                    }
                
                files = os.listdir(path)
                return {
                    "success": True,
                    "files": files,
                    "count": len(files),
                    "message": f"Listed {len(files)} files in {path}"
                }
            except Exception as e:
                return {"success": False, "error": str(e)}
        
        else:
            return {
                "success": False,
                "error": f"Tool {tool.name} not implemented"
            }
    
    def _is_safe_path(self, path: str) -> bool:
        """
        Check if a path is safe to operate on.
        
        Safety rules:
        - Must be within current working directory or user's home
        - Cannot access system directories
        - Cannot use .. to escape
        """
        try:
            # Resolve to absolute path
            abs_path = os.path.abspath(path)
            
            # Get safe directories
            cwd = os.getcwd()
            home = os.path.expanduser("~")
            
            # Check if path is within CWD or home
            if abs_path.startswith(cwd) or abs_path.startswith(home):
                # Additional check: no access to system directories
                forbidden = ["/etc", "/sys", "/proc", "/dev", "/boot"]
                for forbidden_dir in forbidden:
                    if abs_path.startswith(forbidden_dir):
                        return False
                return True
            
            return False
        except Exception:
            return False

    def get_stats(self) -> Dict[str, Any]:
        """Get tool handler statistics."""
        return {
            "available_tools": len(self.tools),
            "pending_approvals": len(self.pending_approvals),
            "tools": {name: tool.risk_level.value for name, tool in self.tools.items()}
        }
