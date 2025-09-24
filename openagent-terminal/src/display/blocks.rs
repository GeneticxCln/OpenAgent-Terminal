//! Stub implementation for command blocks functionality
//! This provides minimal types and methods to satisfy compilation
//! when the blocks system has been removed.

use std::time::Instant;
use unicode_width::UnicodeWidthStr;
use openagent_terminal_core::index::{Line, Point};

#[derive(Debug, Clone)]
pub struct Blocks {
    pub enabled: bool,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub cmd: Option<String>,
    pub cwd: Option<String>,
    pub started_at: Instant,
    pub ended_at: Option<Instant>,
    pub exit: Option<i32>,
    pub start_total_line: usize,
    pub end_total_line: Option<usize>,
    pub anim_start: Option<Instant>,
    pub anim_duration_ms: u32,
    pub anim_opening: bool,
}

impl Default for Blocks {
    fn default() -> Self {
        Self::new()
    }
}

impl Blocks {
    pub fn new() -> Self {
        Self {
            enabled: false,
            blocks: Vec::new(),
        }
    }
    
    pub fn header_at_viewport_line(&self, _display_offset: usize, _line: Line) -> Option<BlockHeader> {
        None
    }
    
    pub fn block_at_header_viewport_line(&self, _display_offset: usize, _line: Line) -> Option<&Block> {
        None
    }
    
    pub fn toggle_fold_header_at_viewport_line(&mut self, _display_offset: usize, _line: Line) -> bool {
        false
    }
    
    pub fn folded_label_at_viewport_line(&self, _display_offset: usize, _line: usize) -> Option<String> {
        None
    }
    
    pub fn is_viewport_line_elided(&self, _display_offset: usize, _line: usize) -> bool {
        false
    }
    
    pub fn toggle_fold_at_viewport_point(&mut self, _display_offset: usize, _point: Point) -> bool {
        false
    }
    
    pub fn next_block_after(&self, _display_offset: usize) -> Option<Line> {
        None
    }
    
    pub fn prev_block_before(&self, _display_offset: usize) -> Option<Line> {
        None
    }
    
    pub fn any_running(&self) -> bool {
        false
    }
    
    pub fn on_event(&mut self, _total_lines: usize, _event: &str) {
        // Stub implementation - blocks system removed  
    }
    
    pub fn chip_hit_at(_header: &BlockHeader, _mouse_col: usize, _clip_cols: usize) -> Option<usize> {
        None
    }
    
    pub fn ensure_unfold_at_total_line(&mut self, _total_line: usize) -> bool {
        false
    }
}

impl BlockHeader {
    pub fn chars(&self) -> std::str::Chars<'_> {
        self.content.chars()
    }
    
    pub fn width(&self) -> usize {
        self.content.width()
    }
}
