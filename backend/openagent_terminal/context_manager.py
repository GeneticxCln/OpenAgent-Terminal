"""Context Manager - Gather environment information for AI agent.

This module collects relevant context about the user's environment including
working directory, git status, recent commands, and environment variables.
"""

import asyncio
import os
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Any
import logging

logger = logging.getLogger(__name__)


@dataclass
class EnvironmentContext:
    """Complete environment context for agent queries."""
    
    # Current working directory
    cwd: str
    
    # Git repository information (if available)
    git_branch: Optional[str] = None
    git_status: Optional[str] = None
    git_uncommitted_changes: bool = False
    
    # File system information
    files_in_directory: List[str] = field(default_factory=list)
    subdirectories: List[str] = field(default_factory=list)
    
    # Environment variables (filtered)
    relevant_env_vars: Dict[str, str] = field(default_factory=dict)
    
    # System information
    platform: str = ""
    shell: str = ""
    python_version: str = ""
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "cwd": self.cwd,
            "git": {
                "branch": self.git_branch,
                "status": self.git_status,
                "uncommitted_changes": self.git_uncommitted_changes
            } if self.git_branch else None,
            "filesystem": {
                "files": self.files_in_directory[:20],  # Limit to 20 files
                "directories": self.subdirectories[:10],  # Limit to 10 dirs
            },
            "environment": self.relevant_env_vars,
            "system": {
                "platform": self.platform,
                "shell": self.shell,
                "python": self.python_version
            }
        }


class ContextManager:
    """Manages context gathering for AI agent queries."""
    
    def __init__(self, max_files: int = 20, max_dirs: int = 10):
        """
        Initialize context manager.
        
        Args:
            max_files: Maximum number of files to include in context
            max_dirs: Maximum number of directories to include in context
        """
        self.max_files = max_files
        self.max_dirs = max_dirs
        
        # Relevant environment variables to include
        self.relevant_env_vars = [
            "PATH", "HOME", "USER", "SHELL", "TERM",
            "LANG", "LC_ALL", "EDITOR", "VISUAL",
            "VIRTUAL_ENV", "CONDA_DEFAULT_ENV",  # Python environments
            "NODE_ENV", "npm_package_name",  # Node.js
            "CARGO_HOME", "RUSTUP_HOME",  # Rust
            "GOPATH", "GOROOT",  # Go
        ]
    
    async def get_context(self, cwd: Optional[str] = None) -> EnvironmentContext:
        """
        Gather complete environment context.
        
        Non-blocking: All I/O operations run in thread pool to avoid blocking event loop.
        
        Args:
            cwd: Current working directory. If None, uses process cwd.
            
        Returns:
            EnvironmentContext with all gathered information
        """
        if cwd is None:
            cwd = os.getcwd()
        
        context = EnvironmentContext(cwd=cwd)
        
        # Get event loop for running blocking operations in executor
        loop = asyncio.get_event_loop()
        
        # Run all blocking I/O operations in thread pool (non-blocking)
        context.platform = await loop.run_in_executor(None, self._get_platform)
        context.shell = await loop.run_in_executor(None, self._get_shell)
        context.python_version = await loop.run_in_executor(None, self._get_python_version)
        context.relevant_env_vars = await loop.run_in_executor(None, self._get_relevant_env_vars)
        
        # Get filesystem info in executor (non-blocking)
        fs_result = await loop.run_in_executor(None, self._get_filesystem_info, cwd)
        context.files_in_directory, context.subdirectories = fs_result
        
        # Get git info in executor (non-blocking)
        git_info = await loop.run_in_executor(None, self._get_git_info, cwd)
        if git_info:
            context.git_branch = git_info.get("branch")
            context.git_status = git_info.get("status")
            context.git_uncommitted_changes = git_info.get("uncommitted_changes", False)
        
        logger.debug(f"Context gathered for: {cwd} (non-blocking)")
        return context
    
    def _get_platform(self) -> str:
        """Get platform information."""
        import platform
        return f"{platform.system()} {platform.release()}"
    
    def _get_shell(self) -> str:
        """Get current shell."""
        return os.environ.get("SHELL", "unknown")
    
    def _get_python_version(self) -> str:
        """Get Python version."""
        import sys
        return f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}"
    
    def _get_relevant_env_vars(self) -> Dict[str, str]:
        """Get relevant environment variables."""
        env_vars = {}
        for var in self.relevant_env_vars:
            value = os.environ.get(var)
            if value:
                env_vars[var] = value
        return env_vars
    
    def _get_filesystem_info(self, cwd: str) -> tuple[List[str], List[str]]:
        """
        Get filesystem information for current directory.
        
        Returns:
            Tuple of (files, directories)
        """
        try:
            path = Path(cwd)
            if not path.exists() or not path.is_dir():
                return [], []
            
            files = []
            dirs = []
            
            for item in path.iterdir():
                # Skip hidden files/dirs
                if item.name.startswith('.'):
                    continue
                
                if item.is_file():
                    files.append(item.name)
                elif item.is_dir():
                    dirs.append(item.name)
                
                # Limit results
                if len(files) >= self.max_files and len(dirs) >= self.max_dirs:
                    break
            
            # Sort for consistency
            files.sort()
            dirs.sort()
            
            return files[:self.max_files], dirs[:self.max_dirs]
        
        except (PermissionError, OSError) as e:
            logger.warning(f"Error reading directory {cwd}: {e}")
            return [], []
    
    def _get_git_info(self, cwd: str) -> Optional[Dict[str, Any]]:
        """
        Get git repository information.
        
        Returns:
            Dictionary with git info or None if not a git repo
        """
        try:
            # Check if in a git repo
            result = subprocess.run(
                ["git", "rev-parse", "--git-dir"],
                cwd=cwd,
                capture_output=True,
                text=True,
                timeout=1
            )
            
            if result.returncode != 0:
                return None
            
            git_info = {}
            
            # Get current branch
            result = subprocess.run(
                ["git", "rev-parse", "--abbrev-ref", "HEAD"],
                cwd=cwd,
                capture_output=True,
                text=True,
                timeout=1
            )
            if result.returncode == 0:
                git_info["branch"] = result.stdout.strip()
            
            # Get status (short format)
            result = subprocess.run(
                ["git", "status", "--short"],
                cwd=cwd,
                capture_output=True,
                text=True,
                timeout=2
            )
            if result.returncode == 0:
                status = result.stdout.strip()
                git_info["status"] = status if status else "clean"
                git_info["uncommitted_changes"] = bool(status)
            
            return git_info
        
        except (subprocess.TimeoutExpired, FileNotFoundError, OSError) as e:
            logger.debug(f"Git info not available: {e}")
            return None
    
    def format_context_for_agent(self, context: EnvironmentContext) -> str:
        """
        Format context as a human-readable string for agent.
        
        Args:
            context: Environment context
            
        Returns:
            Formatted context string
        """
        lines = [
            "# Current Environment Context",
            "",
            f"**Working Directory:** `{context.cwd}`",
        ]
        
        # Add git info if available
        if context.git_branch:
            lines.extend([
                "",
                "## Git Repository",
                f"- **Branch:** `{context.git_branch}`",
                f"- **Status:** {'⚠️ Uncommitted changes' if context.git_uncommitted_changes else '✅ Clean'}",
            ])
            if context.git_status and context.git_status != "clean":
                lines.append(f"```\n{context.git_status}\n```")
        
        # Add filesystem info
        if context.files_in_directory or context.subdirectories:
            lines.extend(["", "## Files in Directory"])
            
            if context.subdirectories:
                lines.append(f"**Directories:** {', '.join(context.subdirectories[:5])}")
                if len(context.subdirectories) > 5:
                    lines.append(f"  _(and {len(context.subdirectories) - 5} more)_")
            
            if context.files_in_directory:
                lines.append(f"**Files:** {', '.join(context.files_in_directory[:10])}")
                if len(context.files_in_directory) > 10:
                    lines.append(f"  _(and {len(context.files_in_directory) - 10} more)_")
        
        # Add system info
        lines.extend([
            "",
            "## System Information",
            f"- **Platform:** {context.platform}",
            f"- **Shell:** {context.shell}",
            f"- **Python:** {context.python_version}",
        ])
        
        # Add relevant env vars
        if context.relevant_env_vars:
            important_vars = ["VIRTUAL_ENV", "CONDA_DEFAULT_ENV", "NODE_ENV"]
            for var in important_vars:
                if var in context.relevant_env_vars:
                    lines.append(f"- **{var}:** `{context.relevant_env_vars[var]}`")
        
        return "\n".join(lines)
