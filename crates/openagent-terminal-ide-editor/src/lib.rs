//! Minimal text editor core with a rope buffer and cursor management.

use anyhow::Result;
use parking_lot::RwLock;
use ropey::Rope;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cursor {
    pub line: usize,
    pub column: usize,
}

impl Default for Cursor {
    fn default() -> Self { Self { line: 0, column: 0 } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorBufferMeta {
    pub path: Option<PathBuf>,
    pub language_id: Option<String>,
    pub version: i32,
    pub modified: bool,
}

impl Default for EditorBufferMeta {
    fn default() -> Self {
        Self { path: None, language_id: None, version: 1, modified: false }
    }
}

#[derive(Debug, Clone)]
pub struct EditorBuffer {
    pub rope: Arc<RwLock<Rope>>,
    pub cursor: Arc<RwLock<Cursor>>,
    pub meta: Arc<RwLock<EditorBufferMeta>>,
}

impl EditorBuffer {
    pub fn new() -> Self {
        Self { rope: Arc::new(RwLock::new(Rope::new())), cursor: Arc::new(RwLock::new(Cursor::default())), meta: Arc::new(RwLock::new(EditorBufferMeta::default())) }
    }

    pub fn from_text(text: &str) -> Self {
        Self { rope: Arc::new(RwLock::new(Rope::from_str(text))), cursor: Arc::new(RwLock::new(Cursor::default())), meta: Arc::new(RwLock::new(EditorBufferMeta::default())) }
    }

    pub fn open_file(path: PathBuf) -> Result<Self> {
        let text = fs::read_to_string(&path).unwrap_or_default();
        let mut meta = EditorBufferMeta::default();
        meta.path = Some(path);
        Ok(Self { rope: Arc::new(RwLock::new(Rope::from_str(&text))), cursor: Arc::new(RwLock::new(Cursor::default())), meta: Arc::new(RwLock::new(meta)) })
    }

    pub fn save(&self) -> Result<()> {
        let path = self.meta.read().path.clone().ok_or_else(|| anyhow::anyhow!("no path"))?;
        let text = self.rope.read().to_string();
        fs::write(path, text)?;
        self.meta.write().modified = false;
        Ok(())
    }

    pub fn insert(&self, ch: char) {
        let mut rope = self.rope.write();
        let mut cur = self.cursor.write();
        let char_idx = self.char_index_of_cursor(&rope, &cur);
        rope.insert_char(char_idx, ch);
        // Recompute cursor from new char index
        let new_idx = char_idx + 1;
        let new_line = rope.char_to_line(new_idx);
        let new_col = new_idx - rope.line_to_char(new_line);
        cur.line = new_line;
        cur.column = new_col;
        let mut meta = self.meta.write();
        meta.modified = true;
        meta.version += 1;
    }

    pub fn backspace(&self) {
        let mut rope = self.rope.write();
        let mut cur = self.cursor.write();
        let char_idx = self.char_index_of_cursor(&rope, &cur);
        if char_idx == 0 { return; }
        rope.remove(char_idx - 1..char_idx);
        let new_idx = char_idx - 1;
        let new_line = rope.char_to_line(new_idx);
        let new_col = new_idx - rope.line_to_char(new_line);
        cur.line = new_line;
        cur.column = new_col;
        let mut meta = self.meta.write();
        meta.modified = true;
        meta.version += 1;
    }

    pub fn move_left(&self) { let mut c = self.cursor.write(); if c.column > 0 { c.column -= 1; } }
    pub fn move_right(&self) { let mut c = self.cursor.write(); c.column += 1; }
    pub fn move_up(&self) { let mut c = self.cursor.write(); if c.line > 0 { c.line -= 1; } }
    pub fn move_down(&self) { let mut c = self.cursor.write(); c.line += 1; }

    fn char_index_of_cursor(&self, rope: &Rope, cur: &Cursor) -> usize {
        let line_start = rope.line_to_char(cur.line);
        line_start + cur.column
    }

    pub fn text(&self) -> String { self.rope.read().to_string() }

    /// Get a snapshot of the current cursor position
    pub fn cursor_position(&self) -> Cursor { self.cursor.read().clone() }

    /// Set cursor position (line, column). Clamps to buffer bounds.
    pub fn set_cursor_position(&self, line: usize, column: usize) {
        let rope = self.rope.read();
        let total_lines = rope.len_lines();
        let line = line.min(total_lines.saturating_sub(1));
        let line_len = rope.line(line).len_chars();
        let column = column.min(line_len);
        let mut cur = self.cursor.write();
        cur.line = line; cur.column = column;
    }

    /// Compute UTF-16 code unit offset for current cursor position (LSP-compatible)
    pub fn cursor_position_utf16(&self) -> (u32, u32) {
        let rope = self.rope.read();
        let cur = self.cursor.read();
        let line_slice = rope.line(cur.line);
        let mut u16_units: u32 = 0;
        let mut iter = line_slice.chars();
        for (i, ch) in iter.by_ref().enumerate() {
            if i >= cur.column { break; }
            let mut buf = [0u16; 2];
            u16_units += ch.encode_utf16(&mut buf).len() as u32;
        }
        (cur.line as u32, u16_units)
    }
}

