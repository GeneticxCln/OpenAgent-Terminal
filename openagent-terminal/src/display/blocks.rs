use std::time::Instant;

use openagent_terminal_core::event::CommandBlockEvent;
use openagent_terminal_core::index::Point;

#[derive(Clone, Debug)]
pub struct CommandBlock {
    pub start_total_line: usize,
    pub end_total_line: Option<usize>,
    pub cmd: Option<String>,
    pub cwd: Option<String>,
    pub exit: Option<i32>,
    pub ended_at: Option<Instant>,
    pub started_at: Instant,
    pub folded: bool,
}

impl CommandBlock {
    fn contains_total_line(&self, line: usize) -> bool {
        let end = self.end_total_line.unwrap_or(usize::MAX);
        self.start_total_line <= line && line <= end
    }
}

#[derive(Default)]
pub struct Blocks {
    pub enabled: bool,
    pub blocks: Vec<CommandBlock>,
}

impl Blocks {
    pub fn new() -> Self { Self { enabled: false, blocks: Vec::new() } }

    pub fn on_event(&mut self, total_lines: usize, ev: &CommandBlockEvent) {
        match ev {
            CommandBlockEvent::PromptStart => {
                // Close any unterminated previous block at prompt start.
                if let Some(last) = self.blocks.last_mut() {
                    if last.end_total_line.is_none() {
                        last.end_total_line = total_lines.checked_sub(1);
                        last.ended_at = Some(Instant::now());
                    }
                }
            },
            CommandBlockEvent::CommandStart { cmd } => {
                let block = CommandBlock {
                    start_total_line: total_lines,
                    end_total_line: None,
                    cmd: cmd.clone(),
                    cwd: None,
                    exit: None,
                    ended_at: None,
                    started_at: Instant::now(),
                    folded: false,
                };
                self.blocks.push(block);
            },
            CommandBlockEvent::CommandEnd { exit, cwd } => {
                if let Some(last) = self.blocks.last_mut() {
                    last.exit = *exit;
                    last.cwd = cwd.clone();
                    if last.end_total_line.is_none() {
                        last.end_total_line = total_lines.checked_sub(1);
                        last.ended_at = Some(Instant::now());
                    }
                }
            },
            CommandBlockEvent::PromptEnd => {
                // Nothing special for now.
            },
        }
    }

    pub fn toggle_fold_at_viewport_point(
        &mut self,
        display_offset: usize,
        viewport_point: Point<usize>,
    ) -> bool {
        let total_line = display_offset + viewport_point.line;
        if let Some(block) = self.blocks.iter_mut().rev().find(|b| b.contains_total_line(total_line)) {
            block.folded = !block.folded;
            return true;
        }
        false
    }

    /// Return folded region label to draw at a viewport line if it is the first visible line
    /// of a folded block; returns None otherwise.
    pub fn folded_label_at_viewport_line(
        &self,
        display_offset: usize,
        viewport_line: usize,
    ) -> Option<String> {
        let total_line = display_offset + viewport_line;
        for block in &self.blocks {
            if block.folded && block.contains_total_line(total_line) {
                // Show label only at the block's first visible line in viewport.
                if total_line == block.start_total_line {
                    let end = block.end_total_line.unwrap_or(total_line);
                    let lines = end.saturating_sub(block.start_total_line) + 1;
                    let status = block.exit.map(|c| if c == 0 { "✓" } else { "✗" }).unwrap_or("…");
                    let cmd = block.cmd.clone().unwrap_or_else(|| String::from("(command)"));
                    return Some(format!("⟞ Folded {lines} lines [{status}] {cmd}"));
                }
            }
        }
        None
    }

    /// Is this viewport line within a folded block region (including header line)?
    pub fn is_viewport_line_elided(&self, display_offset: usize, viewport_line: usize) -> bool {
        let total_line = display_offset + viewport_line;
        self.blocks
            .iter()
            .any(|b| b.folded && b.contains_total_line(total_line))
    }


    /// Toggle folding if the viewport line corresponds to a block header.
    #[allow(dead_code)]
    pub fn toggle_fold_header_at_viewport_line(
        &mut self,
        display_offset: usize,
        viewport_line: usize,
    ) -> bool {
        let total_line = display_offset + viewport_line;
        if let Some(block) = self.blocks.iter_mut().find(|b| total_line == b.start_total_line) {
            block.folded = !block.folded;
            return true;
        }
        false
    }

    /// Ensure the block containing `total_line` is unfolded; returns true if state changed.
    pub fn ensure_unfold_at_total_line(&mut self, total_line: usize) -> bool {
        if let Some(block) = self
            .blocks
            .iter_mut()
            .find(|b| b.folded && b.contains_total_line(total_line))
        {
            block.folded = false;
            return true;
        }
        false
    }


    /// Find next block starting after current display_offset and return its start total_line.
    pub fn next_block_after(&self, display_offset: usize) -> Option<usize> {
        self.blocks
            .iter()
            .find(|b| b.start_total_line > display_offset)
            .map(|b| b.start_total_line)
    }

    /// Find previous block starting before current display_offset and return its start total_line.
    pub fn prev_block_before(&self, display_offset: usize) -> Option<usize> {
        self.blocks
            .iter()
            .rev()
            .find(|b| b.start_total_line < display_offset)
            .map(|b| b.start_total_line)
    }

    /// Return block header to draw at a viewport line if it is the first visible line
    /// of an unfolded block; returns None otherwise.
    pub fn header_at_viewport_line(
        &self,
        display_offset: usize,
        viewport_line: usize,
    ) -> Option<String> {
        let total_line = display_offset + viewport_line;
        for block in &self.blocks {
            if !block.folded && total_line == block.start_total_line {
                // Only show header for blocks that have a command and are long enough
                if block.cmd.is_some() && block.end_total_line.is_some_and(|end| end > block.start_total_line) {
                    let cmd = block.cmd.as_ref().unwrap();
                    let status = block.exit.map(|c| if c == 0 { "✓" } else { "✗" }).unwrap_or("…");
                    
                    // Calculate elapsed time
                    let elapsed = if let Some(ended_at) = block.ended_at {
                        ended_at.duration_since(block.started_at)
                    } else {
                        Instant::now().duration_since(block.started_at)
                    };
                    
                    let time_str = if elapsed.as_secs() < 60 {
                        format!("{:.1}s", elapsed.as_secs_f32())
                    } else if elapsed.as_secs() < 3600 {
                        format!("{}m{}s", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
                    } else {
                        format!("{}h{}m", elapsed.as_secs() / 3600, (elapsed.as_secs() % 3600) / 60)
                    };
                    
                    // Format working directory (show last component if too long)
                    let cwd_str = if let Some(ref cwd) = block.cwd {
                        if cwd.len() > 40 {
                            format!("…{}", &cwd[cwd.len() - 37..])
                        } else {
                            cwd.clone()
                        }
                    } else {
                        String::from("~")
                    };
                    
                    return Some(format!("▶ {} [{}] {} ({})", cmd, status, time_str, cwd_str));
                }
            }
        }
        None
    }

    /// Check if the viewport line is a block header line (but not folded).
    pub fn is_viewport_line_header(&self, display_offset: usize, viewport_line: usize) -> bool {
        let total_line = display_offset + viewport_line;
        self.blocks
            .iter()
            .any(|b| !b.folded && total_line == b.start_total_line && b.cmd.is_some())
    }
}

