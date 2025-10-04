"""
Command History Manager - Persistent command history with search capabilities.

This module provides command history management similar to bash/zsh history,
including persistence, navigation, and reverse search (Ctrl+R).
"""

import os
from collections import deque
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import List, Optional, Deque
import logging

logger = logging.getLogger(__name__)


@dataclass
class HistoryEntry:
    """A single command history entry."""
    command: str
    timestamp: datetime
    session_id: Optional[str] = None
    
    def to_line(self) -> str:
        """Convert to history file line format."""
        ts = int(self.timestamp.timestamp())
        return f"{ts}:{self.command}\n"
    
    @classmethod
    def from_line(cls, line: str, session_id: Optional[str] = None) -> Optional["HistoryEntry"]:
        """Parse from history file line format."""
        line = line.strip()
        if not line or line.startswith('#'):
            return None
        
        try:
            # Format: "timestamp:command" or just "command" (legacy)
            if ':' in line and line.split(':', 1)[0].isdigit():
                ts_str, command = line.split(':', 1)
                timestamp = datetime.fromtimestamp(int(ts_str))
            else:
                # Legacy format without timestamp
                command = line
                timestamp = datetime.now()
            
            return cls(
                command=command,
                timestamp=timestamp,
                session_id=session_id
            )
        except (ValueError, IndexError) as e:
            logger.warning(f"Failed to parse history line: {line[:50]}... - {e}")
            return None


class HistoryManager:
    """Manages command history with persistence and search."""
    
    def __init__(
        self, 
        history_file: Optional[Path] = None,
        max_size: int = 10000,
        max_memory: int = 1000
    ):
        """
        Initialize history manager.
        
        Args:
            history_file: Path to history file. Defaults to ~/.config/openagent-terminal/history
            max_size: Maximum entries to keep in file (oldest pruned)
            max_memory: Maximum entries to keep in memory
        """
        if history_file is None:
            history_file = Path.home() / ".config" / "openagent-terminal" / "history"
        
        self.history_file = history_file
        self.max_size = max_size
        self.max_memory = max_memory
        
        # In-memory history (most recent, for quick access)
        self.history: Deque[HistoryEntry] = deque(maxlen=max_memory)
        
        # Navigation state
        self.navigation_index: Optional[int] = None
        self.navigation_buffer: Optional[str] = None
        
        # Search state
        self.search_active: bool = False
        self.search_query: str = ""
        self.search_results: List[HistoryEntry] = []
        self.search_index: int = 0
        
        # Ensure history file exists
        self.history_file.parent.mkdir(parents=True, exist_ok=True)
        
        # Load existing history
        self._load_history()
        
        logger.info(f"📚 History manager initialized with {len(self.history)} entries")
    
    def add(self, command: str, session_id: Optional[str] = None) -> None:
        """
        Add a command to history.
        
        Args:
            command: Command to add
            session_id: Optional session ID
        """
        # Skip commands starting with space (privacy feature) - check BEFORE stripping
        if command.startswith(' '):
            return
        
        command = command.strip()
        
        # Skip empty commands
        if not command:
            return
        
        # Skip duplicates of the last command
        if self.history and self.history[-1].command == command:
            return
        
        # Create entry
        entry = HistoryEntry(
            command=command,
            timestamp=datetime.now(),
            session_id=session_id
        )
        
        # Add to memory
        self.history.append(entry)
        
        # Append to file
        self._append_to_file(entry)
        
        # Reset navigation state
        self.navigation_index = None
        self.navigation_buffer = None
        
        logger.debug(f"Added to history: {command[:50]}...")
    
    def navigate_up(self, current_input: str = "") -> Optional[str]:
        """
        Navigate up in history (older commands).
        
        Args:
            current_input: Current input buffer
            
        Returns:
            Previous command or None
        """
        if not self.history:
            return None
        
        # Initialize navigation
        if self.navigation_index is None:
            self.navigation_index = len(self.history)
            self.navigation_buffer = current_input
        
        # Move up (towards older)
        if self.navigation_index > 0:
            self.navigation_index -= 1
            return self.history[self.navigation_index].command
        
        return None
    
    def navigate_down(self) -> Optional[str]:
        """
        Navigate down in history (newer commands).
        
        Returns:
            Next command, original buffer, or None
        """
        if not self.history or self.navigation_index is None:
            return None
        
        # Move down (towards newer)
        self.navigation_index += 1
        
        # Reached the bottom (current input)
        if self.navigation_index >= len(self.history):
            self.navigation_index = None
            result = self.navigation_buffer
            self.navigation_buffer = None
            return result
        
        return self.history[self.navigation_index].command
    
    def reset_navigation(self) -> None:
        """Reset navigation state."""
        self.navigation_index = None
        self.navigation_buffer = None
    
    def start_search(self, query: str = "") -> None:
        """
        Start reverse search (Ctrl+R style).
        
        Args:
            query: Initial search query
        """
        self.search_active = True
        self.search_query = query
        self.search_index = 0
        self._update_search_results()
    
    def update_search(self, query: str) -> Optional[str]:
        """
        Update search query and return current match.
        
        Args:
            query: Updated search query
            
        Returns:
            Current matching command or None
        """
        self.search_query = query
        self.search_index = 0
        self._update_search_results()
        
        if self.search_results:
            return self.search_results[0].command
        return None
    
    def next_search_result(self) -> Optional[str]:
        """
        Move to next search result (older).
        
        Returns:
            Next matching command or None
        """
        if not self.search_results:
            return None
        
        self.search_index = min(
            self.search_index + 1, 
            len(self.search_results) - 1
        )
        return self.search_results[self.search_index].command
    
    def previous_search_result(self) -> Optional[str]:
        """
        Move to previous search result (newer).
        
        Returns:
            Previous matching command or None
        """
        if not self.search_results:
            return None
        
        self.search_index = max(0, self.search_index - 1)
        return self.search_results[self.search_index].command
    
    def end_search(self) -> Optional[str]:
        """
        End search and return selected command.
        
        Returns:
            Selected command or None
        """
        self.search_active = False
        result = self.search_results[self.search_index].command if self.search_results else None
        self.search_query = ""
        self.search_results = []
        self.search_index = 0
        return result
    
    def cancel_search(self) -> None:
        """Cancel search without selecting."""
        self.search_active = False
        self.search_query = ""
        self.search_results = []
        self.search_index = 0
    
    def get_recent(self, limit: int = 10) -> List[str]:
        """
        Get recent commands.
        
        Args:
            limit: Maximum number of commands to return
            
        Returns:
            List of recent commands (newest first)
        """
        commands = [entry.command for entry in reversed(self.history)]
        return commands[:limit]
    
    def search_history(self, query: str, limit: int = 20) -> List[str]:
        """
        Search history for commands containing query.
        
        Args:
            query: Search query
            limit: Maximum results
            
        Returns:
            List of matching commands (newest first)
        """
        query_lower = query.lower()
        matches = []
        
        for entry in reversed(self.history):
            if query_lower in entry.command.lower():
                matches.append(entry.command)
                if len(matches) >= limit:
                    break
        
        return matches
    
    def clear(self) -> None:
        """Clear all history (memory and file)."""
        self.history.clear()
        self.navigation_index = None
        self.navigation_buffer = None
        self.search_active = False
        self.search_query = ""
        self.search_results = []
        
        # Clear file
        if self.history_file.exists():
            self.history_file.unlink()
        
        logger.info("Cleared all history")
    
    def _load_history(self) -> None:
        """Load history from file."""
        if not self.history_file.exists():
            return
        
        try:
            with open(self.history_file, 'r', encoding='utf-8') as f:
                for line in f:
                    entry = HistoryEntry.from_line(line)
                    if entry:
                        self.history.append(entry)
            
            logger.info(f"Loaded {len(self.history)} history entries")
        except (IOError, OSError) as e:
            logger.error(f"Failed to load history: {e}")
    
    def _append_to_file(self, entry: HistoryEntry) -> None:
        """Append entry to history file."""
        try:
            # Append to file
            with open(self.history_file, 'a', encoding='utf-8') as f:
                f.write(entry.to_line())
            
            # Check if we need to prune
            if self._get_file_line_count() > self.max_size:
                self._prune_history_file()
        except (IOError, OSError) as e:
            logger.error(f"Failed to append to history: {e}")
    
    def _get_file_line_count(self) -> int:
        """Get number of lines in history file."""
        try:
            with open(self.history_file, 'r', encoding='utf-8') as f:
                return sum(1 for _ in f)
        except (IOError, OSError):
            return 0
    
    def _prune_history_file(self) -> None:
        """Prune history file to max_size by removing oldest entries."""
        try:
            # Read all entries
            entries = []
            with open(self.history_file, 'r', encoding='utf-8') as f:
                for line in f:
                    entry = HistoryEntry.from_line(line)
                    if entry:
                        entries.append(entry)
            
            # Keep only most recent max_size entries
            entries = entries[-self.max_size:]
            
            # Write back
            with open(self.history_file, 'w', encoding='utf-8') as f:
                for entry in entries:
                    f.write(entry.to_line())
            
            logger.info(f"Pruned history file to {len(entries)} entries")
        except (IOError, OSError) as e:
            logger.error(f"Failed to prune history: {e}")
    
    def _update_search_results(self) -> None:
        """Update search results based on current query."""
        if not self.search_query:
            self.search_results = []
            return
        
        query_lower = self.search_query.lower()
        self.search_results = [
            entry for entry in reversed(self.history)
            if query_lower in entry.command.lower()
        ]
    
    def get_stats(self) -> dict:
        """Get history statistics."""
        return {
            "total_entries": len(self.history),
            "file_path": str(self.history_file),
            "max_size": self.max_size,
            "max_memory": self.max_memory,
            "navigation_active": self.navigation_index is not None,
            "search_active": self.search_active,
        }
