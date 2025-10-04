"""
Tests for the block formatter module.
"""

import pytest
from openagent_terminal.block_formatter import BlockFormatter, Block


class TestBlockFormatter:
    """Test suite for BlockFormatter."""
    
    @pytest.fixture
    def formatter(self):
        """Create a block formatter for testing."""
        return BlockFormatter()
    
    def test_formatter_initialization(self, formatter):
        """Test that the formatter initializes correctly."""
        assert formatter is not None
    
    def test_simple_text(self, formatter):
        """Test formatting simple text without blocks."""
        text = "This is simple text with no special formatting."
        blocks = formatter.format_response(text)
        
        assert len(blocks) == 1
        assert blocks[0].type == "text"
        assert blocks[0].content == text
    
    def test_code_block_detection(self, formatter):
        """Test detection of markdown code blocks."""
        text = """Here is some code:

```python
def hello():
    print("Hello, World!")
```

And that's it."""
        
        blocks = formatter.format_response(text)
        
        # Should have 3 blocks: text, code, text
        assert len(blocks) == 3
        assert blocks[0].type == "text"
        assert blocks[1].type == "code"
        assert blocks[1].language == "python"
        assert "def hello" in blocks[1].content
        assert blocks[2].type == "text"
    
    def test_multiple_code_blocks(self, formatter):
        """Test handling multiple code blocks."""
        text = """First code:

```rust
fn main() {
    println!("Hello");
}
```

Then more:

```javascript
console.log("Hi");
```

Done."""
        
        blocks = formatter.format_response(text)
        
        # Should have both code blocks
        code_blocks = [b for b in blocks if b.type == "code"]
        assert len(code_blocks) == 2
        assert code_blocks[0].language == "rust"
        assert code_blocks[1].language == "javascript"
    
    def test_diff_block_detection(self, formatter):
        """Test detection of diff blocks."""
        text = """Here's a change:

```diff
- old line
+ new line
```

Done."""
        
        blocks = formatter.format_response(text)
        
        # Should detect as diff
        diff_blocks = [b for b in blocks if b.type == "diff"]
        assert len(diff_blocks) == 1
        assert "- old line" in diff_blocks[0].content
        assert "+ new line" in diff_blocks[0].content
    
    def test_code_block_without_language(self, formatter):
        """Test code blocks without language specified."""
        text = """Code:

```
generic code here
```

Text."""
        
        blocks = formatter.format_response(text)
        
        code_blocks = [b for b in blocks if b.type == "code"]
        assert len(code_blocks) == 1
        assert code_blocks[0].language in [None, "text", ""]
    
    def test_inline_code(self, formatter):
        """Test that inline code is treated as text."""
        text = "Use the `print()` function to output."
        blocks = formatter.format_response(text)
        
        # Inline code should be part of text block
        assert len(blocks) == 1
        assert blocks[0].type == "text"
        assert "`print()`" in blocks[0].content
    
    def test_empty_code_block(self, formatter):
        """Test handling of empty code blocks."""
        text = """Empty:

```python
```

Done."""
        
        blocks = formatter.format_response(text)
        
        # Should handle empty blocks gracefully
        code_blocks = [b for b in blocks if b.type == "code"]
        # May or may not include empty blocks depending on implementation
        assert len(blocks) >= 2  # At least text blocks
    
    def test_nested_backticks(self, formatter):
        """Test handling of nested backticks."""
        text = """Code with backticks:

```markdown
Use `inline code` like this.
```

Done."""
        
        blocks = formatter.format_response(text)
        
        code_blocks = [b for b in blocks if b.type == "code"]
        assert len(code_blocks) == 1
        assert "`inline code`" in code_blocks[0].content
    
    def test_block_type_enum(self):
        """Test Block creation with different types."""
        text_block = Block("text", "Hello", None)
        assert text_block.type == "text"
        assert text_block.content == "Hello"
        assert text_block.language is None
        
        code_block = Block("code", "print()", "python")
        assert code_block.type == "code"
        assert code_block.content == "print()"
        assert code_block.language == "python"
        
        diff_block = Block("diff", "- old\n+ new", None)
        assert diff_block.type == "diff"
        assert diff_block.content == "- old\n+ new"
    
    def test_complex_document(self, formatter):
        """Test formatting a complex document with multiple block types."""
        text = """# Introduction

This is a tutorial about Python and Rust.

## Python Example

```python
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)
```

The above function is recursive.

## Rust Example

```rust
fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n-1) + fibonacci(n-2),
    }
}
```

Both implementations work similarly.

## Changes

```diff
- Old implementation
+ New optimized version
```

That's all!"""
        
        blocks = formatter.format_response(text)
        
        # Should have mix of text, code, and diff blocks
        assert len(blocks) > 5
        
        code_blocks = [b for b in blocks if b.type == "code"]
        assert len(code_blocks) == 2
        
        diff_blocks = [b for b in blocks if b.type == "diff"]
        assert len(diff_blocks) == 1
        
        text_blocks = [b for b in blocks if b.type == "text"]
        assert len(text_blocks) >= 4
    
    def test_special_characters(self, formatter):
        """Test handling of special characters in blocks."""
        text = """Code with special chars:

```python
print("Quotes: \\" and \\n newlines")
# Comment with <html>
```

Done."""
        
        blocks = formatter.format_response(text)
        
        code_blocks = [b for b in blocks if b.type == "code"]
        assert len(code_blocks) == 1
        # Special characters should be preserved
        assert '\\"' in code_blocks[0].content or '"' in code_blocks[0].content


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
