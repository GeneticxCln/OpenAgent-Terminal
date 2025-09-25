//! Command blocks implementation (reintroduced)
//!
//! This module tracks command block boundaries based on OSC 133 events emitted by the
//! core event loop. It provides header rendering metadata, folding/elision controls,
//! and basic copy/export helpers used elsewhere in the UI.

use std::time::Instant;
use unicode_width::UnicodeWidthStr;
use openagent_terminal_core::index::{Line, Point};
use openagent_terminal_core::event::CommandBlockEvent;

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
    pub folded: bool,
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
        Self { enabled: false, blocks: Vec::new() }
    }

    /// Compute the header text for a block (lightweight; used for chip placement and copy/export).
    fn header_text_for(block: &Block) -> String {
        if let Some(cmd) = &block.cmd {
            cmd.clone()
        } else {
            "(command)".to_string()
        }
    }

    /// Return the header metadata for the viewport line if it starts a block.
    pub fn header_at_viewport_line(
        &self,
        display_offset: usize,
        line: Line,
    ) -> Option<BlockHeader> {
        let vp = line.0 as usize;
        for b in &self.blocks {
            let start_vp = b.start_total_line.saturating_sub(display_offset);
            if start_vp == vp {
                // If folded, we still return a header; the folded overlay will replace drawing.
                return Some(BlockHeader { content: Self::header_text_for(b) });
            }
        }
        None
    }

    /// Return a reference to the block whose header starts at the given viewport line.
    pub fn block_at_header_viewport_line(
        &self,
        display_offset: usize,
        line: Line,
    ) -> Option<&Block> {
        let vp = line.0 as usize;
        self.blocks.iter().find(|b| b.start_total_line.saturating_sub(display_offset) == vp)
    }

    /// Toggle folding state for the block with header at the given viewport line.
    pub fn toggle_fold_header_at_viewport_line(
        &mut self,
        display_offset: usize,
        line: Line,
    ) -> bool {
        let vp = line.0 as usize;
        if let Some(b) = self
            .blocks
            .iter_mut()
            .find(|b| b.start_total_line.saturating_sub(display_offset) == vp)
        {
            // Only allow fold when the block has a known end (finished).
            if b.end_total_line.is_some() {
                let now = Instant::now();
                let opening = !b.folded; // if currently unfolded, we are opening animation on fold? invert below
                b.folded = !b.folded;
                b.anim_opening = !b.folded; // true when unfolding to reveal
                b.anim_start = Some(now);
                b.anim_duration_ms = 220;
                // opening variable not used afterward, but computed to clarify intent.
                let _ = opening;
                return true;
            }
        }
        false
    }

    /// If the given viewport line is the first visible line of a folded region, return an overlay label.
    /// We display the folded label on the header line to avoid drawing both header and label.
    pub fn folded_label_at_viewport_line(
        &self,
        display_offset: usize,
        line: usize,
    ) -> Option<String> {
        for b in &self.blocks {
            if b.folded {
                if let Some(end) = b.end_total_line {
                    let start_vp = b.start_total_line.saturating_sub(display_offset);
                    if start_vp == line {
                        let header = Self::header_text_for(b);
                        let _end_vp = end.saturating_sub(display_offset);
                        let lines = end.saturating_sub(b.start_total_line);
                        let status = match b.exit {
                            Some(0) => "✓",
                            Some(_) => "✗",
                            None => "…",
                        };
                        return Some(format!("{} {}  [{} lines folded]", status, header, lines));
                    }
                }
            }
        }
        None
    }

    /// Return true if the given viewport line belongs to a folded block body (not including header line).
    pub fn is_viewport_line_elided(&self, display_offset: usize, line: usize) -> bool {
        for b in &self.blocks {
            if !b.folded {
                continue;
            }
            if let Some(end) = b.end_total_line {
                let start_vp = b.start_total_line.saturating_sub(display_offset);
                let end_vp = end.saturating_sub(display_offset);
                if line > start_vp && line <= end_vp {
                    return true;
                }
            }
        }
        false
    }

    /// Toggle folding at an arbitrary viewport point (header-only for now).
    pub fn toggle_fold_at_viewport_point(&mut self, display_offset: usize, point: Point) -> bool {
        self.toggle_fold_header_at_viewport_line(display_offset, point.line)
    }

    pub fn next_block_after(&self, display_offset: usize) -> Option<Line> {
        let mut starts: Vec<usize> = self
            .blocks
            .iter()
            .map(|b| b.start_total_line.saturating_sub(display_offset))
            .collect();
        starts.sort_unstable();
        let cursor = 0usize;
        starts
            .into_iter()
            .find(|s| *s > cursor)
            .map(|s| Line(s as i32))
    }

    pub fn prev_block_before(&self, display_offset: usize) -> Option<Line> {
        let mut starts: Vec<usize> = self
            .blocks
            .iter()
            .map(|b| b.start_total_line.saturating_sub(display_offset))
            .collect();
        starts.sort_unstable();
        let cursor = 0usize;
        starts
            .into_iter()
            .rev()
            .find(|s| *s < cursor)
            .map(|s| Line(s as i32))
    }

    pub fn any_running(&self) -> bool {
        self.blocks.iter().any(|b| b.exit.is_none())
    }

    /// Handle a command-block lifecycle event coming from the core event loop.
    /// Now consumes structured CommandBlockEvent instead of parsing Debug strings.
    pub fn on_event(&mut self, total_lines: usize, ev: &CommandBlockEvent) {
        match ev {
            CommandBlockEvent::PromptStart => {
                // No-op for now.
            }
            CommandBlockEvent::CommandStart { cmd } => {
                self.blocks.push(Block {
                    cmd: cmd.clone(),
                    cwd: None,
                    started_at: Instant::now(),
                    ended_at: None,
                    exit: None,
                    start_total_line: total_lines,
                    end_total_line: None,
                    folded: false,
                    anim_start: None,
                    anim_duration_ms: 220,
                    anim_opening: true,
                });
            }
            CommandBlockEvent::CommandEnd { exit, cwd } => {
                if let Some(last) = self.blocks.last_mut() {
                    last.exit = *exit;
                    last.cwd = cwd.clone();
                    last.ended_at = Some(Instant::now());
                    let end_line = total_lines.saturating_sub(1);
                    last.end_total_line = Some(end_line.max(last.start_total_line));
                    last.anim_start = Some(Instant::now());
                    last.anim_opening = true;
                }
            }
            CommandBlockEvent::PromptEnd => {
                if let Some(last) = self.blocks.last_mut() {
                    if last.end_total_line.is_none() {
                        let end_line = total_lines.saturating_sub(1);
                        last.end_total_line = Some(end_line.max(last.start_total_line));
                    }
                }
            }
        }
    }

    /// Compute [start,end) ranges for header action chips in columns based on header width.
    pub fn compute_header_chip_ranges(header: &str) -> Vec<(usize, usize)> {
        // Expanded chips for richer inline actions. Order optimized for small widths.
        // Copy (safe subset), Retry, Fix, Diff, Explain
        const CHIPS: [&str; 5] = ["[Copy]", "[Retry]", "[Fix]", "[Diff]", "[Explain]"];
        let base = header.width() + 2; // two spaces after header text
        let mut col = base;
        let mut out = Vec::with_capacity(CHIPS.len());
        for label in CHIPS.iter() {
            let w = label.width();
            let start = col;
            let end = start + w;
            out.push((start, end));
            col = end + 1; // one space between chips
        }
        out
    }

    /// Hit-test header action chips with clipping columns (to leave room for right-aligned time).
    pub fn chip_hit_at(header: &str, mouse_col: usize, clip_cols: usize) -> Option<usize> {
        let ranges = Self::compute_header_chip_ranges(header);
        for (i, (start, end)) in ranges.iter().enumerate() {
            // Respect clip columns (anything beyond is offscreen/reserved)
            if *start >= clip_cols {
                break;
            }
            let e = (*end).min(clip_cols);
            if mouse_col >= *start && mouse_col < e {
                return Some(i);
            }
        }
        None
    }

    /// Ensure block containing the absolute total line is unfolded.
    pub fn ensure_unfold_at_total_line(&mut self, total_line: usize) -> bool {
        if let Some(b) = self.blocks.iter_mut().find(|b| {
            if let Some(end) = b.end_total_line {
                b.start_total_line <= total_line && total_line <= end
            } else {
                b.start_total_line <= total_line
            }
        }) {
            if b.folded {
                b.folded = false;
                b.anim_opening = true;
                b.anim_start = Some(Instant::now());
                b.anim_duration_ms = 220;
                return true;
            }
        }
        false
    }
}

/// Additional per-block content tracking for channel splitting and actions.
#[derive(Debug, Default, Clone)]
pub struct BlockContent {
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
    pub collapsed_stdout: bool,
    pub collapsed_stderr: bool,
    /// Last completed run’s combined stdout for diffing
    pub last_stdout: Option<String>,
}

impl BlockContent {
    pub fn toggle_stdout(&mut self) { self.collapsed_stdout = !self.collapsed_stdout; }
    pub fn toggle_stderr(&mut self) { self.collapsed_stderr = !self.collapsed_stderr; }

    /// Produce a copy-only "safe subset" by stripping ANSI escapes and common secret patterns.
    pub fn safe_subset(text: &str) -> String {
        // Strip ANSI CSI sequences (very simple)
        let ansi_re = regex::Regex::new(r"\u{001b}\[[0-9;?]*[ -/]*[@-~]").ok();
        let mut s = if let Some(re) = ansi_re { re.replace_all(text, "").into_owned() } else { text.to_string() };
        // Redact common secret patterns (very simple heuristic)
        let secret_re = regex::Regex::new(r"(?i)(api[_-]?key|token|secret)\s*[:=]\s*([A-Za-z0-9\-_]{8,})").ok();
        if let Some(re) = secret_re {
            s = re.replace_all(&s, |caps: &regex::Captures<'_>| {
                format!("{}: {{redacted}}", &caps[1])
            }).into_owned();
        }
        s
    }

    /// Compute a simple unified diff between previous and current outputs.
    pub fn diff_previous(&self, current_stdout: &str) -> String {
        let prev = self.last_stdout.as_deref().unwrap_or("");
        let mut out = String::new();
        out.push_str("--- previous\n+++ current\n");
        let prev_lines: Vec<&str> = prev.lines().collect();
        let curr_lines: Vec<&str> = current_stdout.lines().collect();
        let mut i = 0usize;
        let mut j = 0usize;
        while i < prev_lines.len() || j < curr_lines.len() {
            if i < prev_lines.len() && j < curr_lines.len() {
                if prev_lines[i] == curr_lines[j] {
                    // context omitted to keep output brief
                    i += 1; j += 1;
                } else {
                    out.push_str(&format!("-{}\n", prev_lines[i]));
                    out.push_str(&format!("+{}\n", curr_lines[j]));
                    i += 1; j += 1;
                }
            } else if i < prev_lines.len() {
                out.push_str(&format!("-{}\n", prev_lines[i]));
                i += 1;
            } else if j < curr_lines.len() {
                out.push_str(&format!("+{}\n", curr_lines[j]));
                j += 1;
            }
        }
        out
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
