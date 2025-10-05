# OpenAgent Terminal - Keyboard Shortcuts

Quick reference for all available keyboard shortcuts in the OpenAgent Terminal.

## Navigation

| Key | Action | Description |
|-----|--------|-------------|
| **←** | Move Left | Move cursor left by one character/grapheme |
| **→** | Move Right | Move cursor right by one character/grapheme |
| **Ctrl+A** | Home | Move to beginning of line |
| **Ctrl+E** | End | Move to end of line |
| **Home** | Home | Move to beginning of line |
| **End** | End | Move to end of line |
| **Ctrl+←** | Word Left | Move to beginning of previous word |
| **Ctrl+→** | Word Right | Move to beginning of next word |

## Editing

| Key | Action | Description |
|-----|--------|-------------|
| **Backspace** | Delete Back | Delete character/grapheme before cursor |
| **Delete** | Delete Forward | Delete character/grapheme at cursor |
| **Ctrl+W** | Delete Word | Delete previous word |
| **Ctrl+U** | Clear to Start | Delete from cursor to beginning of line |
| **Ctrl+K** | Clear to End | Delete from cursor to end of line |

## History

| Key | Action | Description |
|-----|--------|-------------|
| **↑** | History Up | Navigate to previous command in history |
| **↓** | History Down | Navigate to next command in history |
| **Ctrl+R** | Reverse Search | Search through command history (placeholder) |

## Control

| Key | Action | Description |
|-----|--------|-------------|
| **Enter** | Submit | Execute the current command |
| **Ctrl+C** | Cancel | Cancel current input/operation |
| **Ctrl+D** | Exit | Exit terminal (on empty line) |
| **Ctrl+L** | Clear Screen | Clear the terminal screen |

## Unicode Support

The editor properly handles:
- **Emoji**: 👋 🎉 🚀 (treated as single characters)
- **Combined characters**: café, naïve (proper grapheme boundaries)
- **International text**: 日本語, العربية, עברית
- **Complex scripts**: Proper handling of combining marks

### Examples

```bash
# Emoji work as single characters
> Hello 👋 World
# Press Left - cursor jumps over entire emoji

# Combined characters handled correctly
> café
# Press Backspace twice - removes 'é' and 'f'

# Word operations respect Unicode
> git commit -m "message"
# Press Ctrl+W - deletes last word including quotes
```

## Tips

1. **Fast editing**: Use Ctrl+W to quickly delete words
2. **Line clearing**: Ctrl+U clears everything before cursor (useful for retyping)
3. **Command recall**: Use ↑/↓ to browse through previous commands
4. **Screen management**: Ctrl+L clears screen without losing history

## Compatibility

These shortcuts follow standard readline/bash/zsh conventions, making the terminal familiar to most users.

## Coming Soon

- **Full Ctrl+R**: Interactive reverse search with real-time feedback
- **Tab completion**: Command and path auto-completion
- **Alt+B/F**: Alternative word navigation (if terminal supports)
- **Kill ring**: Advanced copy/paste buffer

---

For detailed implementation information, see [INPUT_HANDLING_FIX.md](INPUT_HANDLING_FIX.md)
