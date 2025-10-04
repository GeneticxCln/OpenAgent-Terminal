# Keyboard Shortcuts Design

**Created:** 2025-10-04  
**Status:** Design Complete - Ready for Implementation  
**Priority:** High (UX improvement)  
**Estimated Time:** 4-6 hours

---

## 🎯 Objective

Add comprehensive keyboard shortcuts to improve user experience and make the terminal more powerful and efficient.

---

## ⌨️ Keyboard Shortcuts to Implement

### Essential Shortcuts

| Shortcut | Action | Priority | Status |
|----------|--------|----------|--------|
| **Ctrl+C** | Cancel current operation | High | ✅ Built-in |
| **Ctrl+D** | Exit (EOF) | High | ✅ Built-in |
| **Up Arrow** | Navigate history (older) | High | ⏳ Backend ready |
| **Down Arrow** | Navigate history (newer) | High | ⏳ Backend ready |
| **Ctrl+R** | Reverse search history | High | ⏳ Backend ready |
| **Ctrl+K** | Clear screen | Medium | ⏳ To implement |
| **Ctrl+L** | Show command history | Medium | ⏳ To implement |
| **Ctrl+N** | New session | Medium | ⏳ To implement |
| **Ctrl+W** | Close current session | Low | ⏳ To implement |
| **Tab** | Auto-complete (future) | Low | 📋 Planned |

---

## 🏗️ Implementation Architecture

### Current State

**Problem:** The current implementation uses `tokio::io::stdin()` with `read_line()`, which:
- Only reads complete lines
- Doesn't capture arrow keys, Ctrl combinations
- Doesn't support raw terminal mode

**Current Input Flow:**
```
User types → read_line() → Full line → Parse command
```

### Target State

**Solution:** Use `crossterm` for raw terminal input handling

**New Input Flow:**
```
User input → crossterm events → Key handler → Line editor → Command parser
```

---

## 📦 Required Dependencies

### Add to `Cargo.toml`:
```toml
[dependencies]
# Existing...
crossterm = "0.27"        # Terminal manipulation and events
tokio = { version = "1", features = ["full"] }
```

### Backend (Python) - Already Complete! ✅
The `HistoryManager` in `backend/openagent_terminal/history_manager.py` already supports:
- ✅ `navigate_up()` - For Up arrow
- ✅ `navigate_down()` - For Down arrow  
- ✅ `start_search()`, `update_search()` - For Ctrl+R
- ✅ `get_recent()` - For Ctrl+L

---

## 🔨 Implementation Plan

### Phase 1: Terminal Setup (1 hour)

**Goal:** Enable raw terminal mode for keyboard capture

```rust
// src/terminal.rs (new file)
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{self, ClearType},
    ExecutableCommand,
};
use std::io::{self, Write};

pub struct TerminalManager {
    original_mode: Option<()>,
}

impl TerminalManager {
    pub fn new() -> Result<Self> {
        // Enable raw mode
        terminal::enable_raw_mode()?;
        
        Ok(Self {
            original_mode: Some(()),
        })
    }
    
    pub fn clear_screen(&mut self) -> Result<()> {
        io::stdout()
            .execute(terminal::Clear(ClearType::All))?
            .execute(crossterm::cursor::MoveTo(0, 0))?;
        Ok(())
    }
    
    pub fn restore(&mut self) -> Result<()> {
        terminal::disable_raw_mode()?;
        Ok(())
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}
```

---

### Phase 2: Line Editor (2 hours)

**Goal:** Implement line editing with history support

```rust
// src/line_editor.rs (new file)
use crossterm::event::{KeyCode, KeyModifiers};

pub struct LineEditor {
    buffer: String,
    cursor: usize,
    history_index: Option<usize>,
}

impl LineEditor {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            history_index: None,
        }
    }
    
    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> EditorAction {
        match (key, modifiers) {
            // Navigation
            (KeyCode::Left, KeyModifiers::NONE) => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                EditorAction::Redraw
            }
            (KeyCode::Right, KeyModifiers::NONE) => {
                if self.cursor < self.buffer.len() {
                    self.cursor += 1;
                }
                EditorAction::Redraw
            }
            (KeyCode::Home, _) | (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                self.cursor = 0;
                EditorAction::Redraw
            }
            (KeyCode::End, _) | (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                self.cursor = self.buffer.len();
                EditorAction::Redraw
            }
            
            // Editing
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.buffer.insert(self.cursor, c);
                self.cursor += 1;
                EditorAction::Redraw
            }
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                if self.cursor > 0 {
                    self.buffer.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
                EditorAction::Redraw
            }
            (KeyCode::Delete, KeyModifiers::NONE) => {
                if self.cursor < self.buffer.len() {
                    self.buffer.remove(self.cursor);
                }
                EditorAction::Redraw
            }
            
            // History navigation
            (KeyCode::Up, KeyModifiers::NONE) => {
                EditorAction::HistoryUp
            }
            (KeyCode::Down, KeyModifiers::NONE) => {
                EditorAction::HistoryDown
            }
            
            // Commands
            (KeyCode::Enter, KeyModifiers::NONE) => {
                EditorAction::Submit(self.buffer.clone())
            }
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                EditorAction::Cancel
            }
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                if self.buffer.is_empty() {
                    EditorAction::Exit
                } else {
                    EditorAction::None
                }
            }
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                EditorAction::ClearScreen
            }
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                EditorAction::ShowHistory
            }
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                EditorAction::ReverseSearch
            }
            (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                EditorAction::NewSession
            }
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                EditorAction::CloseSession
            }
            
            _ => EditorAction::None,
        }
    }
    
    pub fn set_buffer(&mut self, text: String) {
        self.buffer = text;
        self.cursor = self.buffer.len();
    }
    
    pub fn get_buffer(&self) -> &str {
        &self.buffer
    }
    
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.history_index = None;
    }
    
    pub fn render(&self, prompt: &str) -> String {
        format!("{}{}", prompt, self.buffer)
    }
}

pub enum EditorAction {
    None,
    Redraw,
    Submit(String),
    Cancel,
    Exit,
    HistoryUp,
    HistoryDown,
    ReverseSearch,
    ClearScreen,
    ShowHistory,
    NewSession,
    CloseSession,
}
```

---

### Phase 3: Event Loop Integration (2 hours)

**Goal:** Replace read_line() with event-based input

```rust
// In main.rs - update run_interactive_loop()

async fn run_interactive_loop(
    client: &mut ipc::client::IpcClient,
    session_manager: &mut session::SessionManager,
) -> Result<()> {
    use crossterm::event::{self, Event};
    
    let mut terminal = TerminalManager::new()?;
    let mut editor = LineEditor::new();
    let mut history = HistoryManager::new(); // Local history cache
    
    loop {
        // Show prompt
        let prompt = get_prompt(session_manager);
        print!("{}", editor.render(&prompt));
        io::stdout().flush()?;
        
        // Wait for keyboard event with timeout
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key_event) = event::read()? {
                let action = editor.handle_key(key_event.code, key_event.modifiers);
                
                match action {
                    EditorAction::Submit(command) => {
                        println!(); // New line after submission
                        
                        // Add to history
                        history.add(&command);
                        
                        // Process command
                        process_command(&command, client, session_manager).await?;
                        
                        // Clear editor
                        editor.clear();
                    }
                    
                    EditorAction::HistoryUp => {
                        if let Some(cmd) = history.navigate_up(editor.get_buffer()) {
                            editor.set_buffer(cmd);
                        }
                    }
                    
                    EditorAction::HistoryDown => {
                        if let Some(cmd) = history.navigate_down() {
                            editor.set_buffer(cmd);
                        } else {
                            editor.clear();
                        }
                    }
                    
                    EditorAction::ClearScreen => {
                        terminal.clear_screen()?;
                    }
                    
                    EditorAction::ShowHistory => {
                        show_history(&history);
                    }
                    
                    EditorAction::ReverseSearch => {
                        reverse_search(&mut editor, &history)?;
                    }
                    
                    EditorAction::NewSession => {
                        create_new_session(session_manager).await?;
                    }
                    
                    EditorAction::Exit => {
                        break;
                    }
                    
                    EditorAction::Redraw => {
                        // Redraw the line
                        print!("\r\x1b[K{}", editor.render(&prompt));
                        io::stdout().flush()?;
                    }
                    
                    _ => {}
                }
            }
        }
    }
    
    terminal.restore()?;
    Ok(())
}
```

---

## 🎨 UI/UX Design

### Ctrl+R (Reverse Search)

```
(reverse-i-search)`git': git status
```

**Behavior:**
1. User presses Ctrl+R
2. Mode changes to search mode
3. User types search query
4. Shows matching command from history
5. Ctrl+R again: next match
6. Enter: execute command
7. Esc: cancel search

### Ctrl+L (Show History)

```
╔═══════════════════════════════════════════╗
║           Command History (Recent)        ║
╚═══════════════════════════════════════════╝

 10  cargo build --release
  9  python -m pytest
  8  git commit -m "Add feature"
  7  git status
  6  ls -la
  ...

Press number to recall, or any key to dismiss
```

### Ctrl+K (Clear Screen)

- Clear screen completely
- Show fresh prompt at top
- Preserve session state

### Ctrl+N (New Session)

```
Create new session? [Y/n]: _
Title (optional): _
```

---

## 🔧 Helper Functions

### History Manager (Rust Side)

```rust
// src/history.rs (new file)
use std::collections::VecDeque;
use std::path::PathBuf;

pub struct HistoryManager {
    history: VecDeque<String>,
    max_size: usize,
    navigation_index: Option<usize>,
    navigation_buffer: Option<String>,
}

impl HistoryManager {
    pub fn new() -> Self {
        let mut manager = Self {
            history: VecDeque::new(),
            max_size: 1000,
            navigation_index: None,
            navigation_buffer: None,
        };
        
        // Load from file if exists
        let _ = manager.load_from_file();
        
        manager
    }
    
    pub fn add(&mut self, command: &str) {
        if command.is_empty() || command.starts_with(' ') {
            return;
        }
        
        // Skip duplicates
        if let Some(last) = self.history.back() {
            if last == command {
                return;
            }
        }
        
        self.history.push_back(command.to_string());
        
        if self.history.len() > self.max_size {
            self.history.pop_front();
        }
        
        // Save to file
        let _ = self.save_to_file();
    }
    
    pub fn navigate_up(&mut self, current: &str) -> Option<String> {
        if self.history.is_empty() {
            return None;
        }
        
        if self.navigation_index.is_none() {
            self.navigation_index = Some(self.history.len());
            self.navigation_buffer = Some(current.to_string());
        }
        
        if let Some(idx) = self.navigation_index {
            if idx > 0 {
                self.navigation_index = Some(idx - 1);
                return self.history.get(idx - 1).cloned();
            }
        }
        
        None
    }
    
    pub fn navigate_down(&mut self) -> Option<String> {
        if let Some(idx) = self.navigation_index {
            let next_idx = idx + 1;
            
            if next_idx >= self.history.len() {
                self.navigation_index = None;
                return self.navigation_buffer.take();
            }
            
            self.navigation_index = Some(next_idx);
            return self.history.get(next_idx).cloned();
        }
        
        None
    }
    
    fn history_file() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("openagent-terminal")
            .join("history")
    }
    
    fn load_from_file(&mut self) -> Result<()> {
        let path = Self::history_file();
        if !path.exists() {
            return Ok(());
        }
        
        let contents = std::fs::read_to_string(path)?;
        for line in contents.lines() {
            if let Some((_ts, cmd)) = line.split_once(':') {
                self.history.push_back(cmd.to_string());
            }
        }
        
        Ok(())
    }
    
    fn save_to_file(&self) -> Result<()> {
        let path = Self::history_file();
        std::fs::create_dir_all(path.parent().unwrap())?;
        
        let mut contents = String::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        for cmd in &self.history {
            contents.push_str(&format!("{}:{}\n", now, cmd));
        }
        
        std::fs::write(path, contents)?;
        Ok(())
    }
}
```

---

## 📝 Configuration

### Add to `config.toml`:

```toml
[shortcuts]
# Enable/disable shortcuts
enabled = true

# Custom keybindings (advanced)
[shortcuts.keys]
clear_screen = "Ctrl+K"
show_history = "Ctrl+L"
reverse_search = "Ctrl+R"
new_session = "Ctrl+N"
close_session = "Ctrl+W"
```

---

## 🧪 Testing Strategy

### Manual Tests

1. **Up/Down Navigation**
   - Type several commands
   - Press Up: should show previous command
   - Press Down: should show next command
   - Press Down at bottom: should restore current input

2. **Ctrl+R Search**
   - Press Ctrl+R
   - Type search query
   - Should show matching command
   - Press Ctrl+R again: next match

3. **Ctrl+K Clear**
   - Fill screen with output
   - Press Ctrl+K
   - Screen should clear, prompt at top

4. **Ctrl+L History**
   - Press Ctrl+L
   - Should show recent commands
   - Can dismiss or select

### Automated Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_line_editor_insert() {
        let mut editor = LineEditor::new();
        editor.handle_key(KeyCode::Char('h'), KeyModifiers::NONE);
        editor.handle_key(KeyCode::Char('i'), KeyModifiers::NONE);
        assert_eq!(editor.get_buffer(), "hi");
    }
    
    #[test]
    fn test_history_navigation() {
        let mut history = HistoryManager::new();
        history.add("command1");
        history.add("command2");
        
        let cmd = history.navigate_up("");
        assert_eq!(cmd, Some("command2".to_string()));
    }
}
```

---

## 🎯 Success Criteria

| Feature | Criteria | Status |
|---------|----------|--------|
| Raw keyboard input | Captures all keys | ⏳ |
| Up/Down navigation | Works smoothly | ⏳ |
| Ctrl+R search | Finds commands | ⏳ |
| Ctrl+K clear | Clears screen | ⏳ |
| Line editing | Cursor movement works | ⏳ |
| History persistence | Survives restart | ⏳ |

---

## 🚀 Rollout Plan

### Phase 1: Foundation (Day 1)
- Add crossterm dependency
- Implement TerminalManager
- Enable raw mode

### Phase 2: Line Editor (Day 2)
- Implement LineEditor
- Add keyboard event handling
- Test basic editing

### Phase 3: History (Day 3)
- Integrate HistoryManager
- Add Up/Down navigation
- Test persistence

### Phase 4: Advanced (Day 4)
- Implement Ctrl+R search
- Add Ctrl+K, Ctrl+L, Ctrl+N
- Polish UX

---

## 📚 References

- [crossterm docs](https://docs.rs/crossterm/)
- [rustyline](https://github.com/kkawakam/rustyline) - Reference implementation
- [ratatui](https://github.com/ratatui-org/ratatui) - TUI framework (for future)

---

## 🎉 Expected Impact

**Before:**
- No history navigation
- No keyboard shortcuts
- Basic line editing only

**After:**
- Full bash/zsh-style history
- Power user shortcuts
- Professional terminal experience

---

**Status:** 📋 Design Complete - Ready for Rust Implementation  
**Estimated Time:** 4-6 hours  
**Priority:** High (UX improvement)  
**Next Step:** Implement TerminalManager with crossterm
