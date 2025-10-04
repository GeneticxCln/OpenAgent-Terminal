"""
Block Formatter - Detects and formats structured content blocks.

Handles:
- Code blocks (```language)
- Diffs
- Lists and formatted text
- Tables (future)
"""

import re
from dataclasses import dataclass
from typing import List, Optional


@dataclass
class Block:
    """Represents a formatted content block."""

    type: str  # 'code', 'text', 'diff', 'list'
    content: str
    language: Optional[str] = None  # For code blocks
    metadata: Optional[dict] = None  # Additional info

    def to_dict(self) -> dict:
        """Convert to dictionary for JSON serialization."""
        result = {
            "type": self.type,
            "content": self.content,
        }
        if self.language:
            result["language"] = self.language
        if self.metadata:
            result["metadata"] = self.metadata
        return result


class BlockFormatter:
    """Formats AI responses into structured blocks."""

    # Regex for markdown code blocks
    CODE_BLOCK_PATTERN = re.compile(
        r"```(\w+)?\n(.*?)```", re.DOTALL | re.MULTILINE
    )

    # Regex for diff-like content
    DIFF_PATTERN = re.compile(
        r"^[\+\-]\s", re.MULTILINE
    )

    def __init__(self):
        """Initialize the block formatter."""
        self.supported_languages = {
            "rust", "python", "javascript", "typescript", "bash", "sh",
            "json", "yaml", "toml", "markdown", "html", "css", "sql",
            "c", "cpp", "go", "java", "ruby", "php"
        }

    def format_response(self, text: str) -> List[Block]:
        """
        Parse text into structured blocks.

        Args:
            text: Raw text from AI response

        Returns:
            List of Block objects
        """
        blocks = []
        current_pos = 0

        # Find all code blocks
        for match in self.CODE_BLOCK_PATTERN.finditer(text):
            start, end = match.span()

            # Add text before code block as text block
            if start > current_pos:
                pre_text = text[current_pos:start].strip()
                if pre_text:
                    blocks.append(self._create_text_block(pre_text))

            # Add code block
            language = match.group(1) or "text"
            code_content = match.group(2).strip()

            # Check if it's a diff
            if self._looks_like_diff(code_content):
                blocks.append(Block(
                    type="diff",
                    content=code_content,
                    language=language,
                ))
            else:
                blocks.append(Block(
                    type="code",
                    content=code_content,
                    language=language.lower(),
                ))

            current_pos = end

        # Add remaining text
        if current_pos < len(text):
            remaining = text[current_pos:].strip()
            if remaining:
                blocks.append(self._create_text_block(remaining))

        # If no blocks were found, treat entire text as one text block
        if not blocks:
            blocks.append(self._create_text_block(text))

        return blocks

    def _create_text_block(self, text: str) -> Block:
        """Create a text block, detecting special formatting."""
        # Check if it's a list
        if self._looks_like_list(text):
            return Block(
                type="list",
                content=text,
                metadata={"format": "markdown"}
            )
        else:
            return Block(
                type="text",
                content=text,
            )

    def _looks_like_diff(self, text: str) -> bool:
        """Check if text looks like a diff."""
        lines = text.split('\n')
        diff_lines = sum(1 for line in lines if line.startswith(('+', '-')))
        return diff_lines > 2  # At least 3 diff lines

    def _looks_like_list(self, text: str) -> bool:
        """Check if text is a bulleted/numbered list."""
        lines = text.strip().split('\n')
        if len(lines) < 2:
            return False

        list_patterns = [
            r'^\s*[\•\-\*]\s',  # Bullet points
            r'^\s*\d+\.\s',      # Numbered
        ]

        list_lines = sum(
            1 for line in lines
            if any(re.match(pattern, line) for pattern in list_patterns)
        )

        return list_lines >= 2  # At least 2 list items

    def stream_with_blocks(self, text: str) -> List[dict]:
        """
        Convert text to streaming format with block boundaries.

        Returns list of events:
        - {"type": "text", "content": "..."}
        - {"type": "block_start", "block": {...}}
        - {"type": "block_content", "content": "..."}
        - {"type": "block_end"}
        """
        events = []
        blocks = self.format_response(text)

        for block in blocks:
            if block.type in ["code", "diff"]:
                # Emit block boundary
                events.append({
                    "type": "block_start",
                    "block": block.to_dict()
                })
                # Content will be streamed token by token
                events.append({
                    "type": "block_content",
                    "content": block.content
                })
                events.append({
                    "type": "block_end"
                })
            else:
                # Regular text, stream as tokens
                events.append({
                    "type": "text",
                    "content": block.content
                })

        return events

    def highlight_language(self, language: str) -> str:
        """
        Get canonical language name for highlighting.

        Args:
            language: Language identifier from code block

        Returns:
            Canonical language name or 'text' if unknown
        """
        lang_lower = language.lower()

        # Aliases
        aliases = {
            "rs": "rust",
            "py": "python",
            "js": "javascript",
            "ts": "typescript",
            "sh": "bash",
            "yml": "yaml",
        }

        lang_lower = aliases.get(lang_lower, lang_lower)

        if lang_lower in self.supported_languages:
            return lang_lower
        return "text"


# Example usage and testing
if __name__ == "__main__":
    formatter = BlockFormatter()

    # Test with code block
    test_text = """
Here's an example:

```rust
fn main() {
    println!("Hello, world!");
}
```

And some more text.
"""

    blocks = formatter.format_response(test_text)
    for i, block in enumerate(blocks):
        print(f"Block {i}: {block.type}")
        if block.language:
            print(f"  Language: {block.language}")
        print(f"  Content: {block.content[:50]}...")
        print()
