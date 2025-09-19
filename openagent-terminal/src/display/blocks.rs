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
    // Animation state for fold/unfold transitions
    pub anim_start: Option<Instant>,
    pub anim_opening: bool,
    pub anim_duration_ms: u32,
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
    pub fn new() -> Self {
        Self {
            enabled: false,
            blocks: Vec::new(),
        }
    }

    /// Return true if any command block is currently running (exit is None).
    pub fn any_running(&self) -> bool {
        self.blocks.iter().any(|b| b.exit.is_none())
    }

    /// Compute the start/end column ranges for action chips following a given header text.
    /// Chips are rendered with a single space between them, starting at header.width() + 2.
    pub fn compute_header_chip_ranges(header: &str) -> Vec<(usize, usize)> {
        use unicode_width::UnicodeWidthStr as _;
        let mut col = header.width() + 2;
        let chips = ["[Copy]", "[Rerun]", "[Export]"];
        let mut ranges = Vec::with_capacity(chips.len());
        for chip in chips {
            let start = col;
            let end = start + chip.width();
            ranges.push((start, end));
            col = end + 1;
        }
        ranges
    }

    /// Hit-test chip index under a given mouse column, respecting the visible columns width.
    /// Returns Some(index) if the mouse column is over the visible portion of a chip.
    pub fn chip_hit_at(header: &str, mouse_col: usize, columns: usize) -> Option<usize> {
        let ranges = Self::compute_header_chip_ranges(header);
        for (i, (start, end)) in ranges.iter().enumerate() {
            // Skip chips fully outside the grid width
            if *start >= columns {
                continue;
            }
            let visible_end = (*end).min(columns);
            if mouse_col >= *start && mouse_col < visible_end {
                return Some(i);
            }
        }
        None
    }

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
            }
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
                    anim_start: None,
                    anim_opening: false,
                    anim_duration_ms: 140,
                };
                self.blocks.push(block);
            }
            CommandBlockEvent::CommandEnd { exit, cwd } => {
                if let Some(last) = self.blocks.last_mut() {
                    last.exit = *exit;
                    last.cwd = cwd.clone();
                    if last.end_total_line.is_none() {
                        last.end_total_line = total_lines.checked_sub(1);
                        last.ended_at = Some(Instant::now());
                        // Auto-collapse very long blocks to reduce visual noise
                        if let Some(end) = last.end_total_line {
                            let lines = end.saturating_sub(last.start_total_line) + 1;
                            if lines > 200 {
                                last.folded = true;
                            }
                        }
                    }
                }
            }
            CommandBlockEvent::PromptEnd => {
                // Nothing special for now.
            }
        }
    }

    pub fn toggle_fold_at_viewport_point(
        &mut self,
        display_offset: usize,
        viewport_point: Point<usize>,
    ) -> bool {
        let total_line = display_offset + viewport_point.line;
        if let Some(block) = self
            .blocks
            .iter_mut()
            .rev()
            .find(|b| b.contains_total_line(total_line))
        {
            block.folded = !block.folded;
            block.anim_start = Some(Instant::now());
            block.anim_opening = !block.folded; // opening when unfolding
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
                    let status = block
                        .exit
                        .map(|c| if c == 0 { "✓" } else { "✗" })
                        .unwrap_or("…");
                    let cmd = block
                        .cmd
                        .clone()
                        .unwrap_or_else(|| String::from("(command)"));
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
        if let Some(block) = self
            .blocks
            .iter_mut()
            .find(|b| total_line == b.start_total_line)
        {
            block.folded = !block.folded;
            block.anim_start = Some(Instant::now());
            block.anim_opening = !block.folded; // opening when unfolding
                                                // keep existing duration
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

    /// Return the command block for a header at a viewport line (unfolded only).
    /// This returns a reference to the block when the given viewport line corresponds
    /// to the first visible line of an unfolded block that has a command.
    pub fn block_at_header_viewport_line(
        &self,
        display_offset: usize,
        viewport_line: usize,
    ) -> Option<&CommandBlock> {
        let total_line = display_offset + viewport_line;
        self.blocks
            .iter()
            .find(|b| !b.folded && b.cmd.is_some() && total_line == b.start_total_line)
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
                // Show header for blocks that have a command; include running spinner when exit is None
                if block.cmd.is_some() {
                    let cmd = block.cmd.as_ref().unwrap();

                    // Running spinner or final status
                    let status = match block.exit {
                        Some(code) => {
                            if code == 0 {
                                "✓".to_string()
                            } else {
                                "✗".to_string()
                            }
                        }
                        None => {
                            // Spinner frames
                            const FRAMES: [&str; 10] =
                                ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                            let ms = Instant::now().duration_since(block.started_at).as_millis()
                                as usize;
                            let idx = (ms / 120) % FRAMES.len();
                            FRAMES[idx].to_string()
                        }
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

                    // Note: Duration is now drawn right-aligned in the overlay; omit from header text.
                    return Some(format!("▶ {} [{}] ({})", cmd, status, cwd_str));
                }
            }
        }
        None
    }

    /// Check if the viewport line is a block header line (but not folded).
    #[allow(dead_code)]
    pub fn is_viewport_line_header(&self, display_offset: usize, viewport_line: usize) -> bool {
        let total_line = display_offset + viewport_line;
        self.blocks
            .iter()
            .any(|b| !b.folded && total_line == b.start_total_line && b.cmd.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_and_labels() {
        let mut blocks = Blocks::new();
        blocks.enabled = true;
        // A block spanning total lines 10..=20
        blocks.blocks.push(CommandBlock {
            start_total_line: 10,
            end_total_line: Some(20),
            cmd: Some("make build".to_string()),
            cwd: Some("/home/user/project".to_string()),
            exit: Some(0),
            ended_at: None,
            started_at: Instant::now(),
            folded: false,
            anim_start: None,
            anim_opening: false,
            anim_duration_ms: 140,
        });

        let display_offset = 5; // so header is at viewport line 5
                                // Initially header present
        assert!(blocks.is_viewport_line_header(display_offset, 5));
        // Toggle fold at header
        let toggled = blocks.toggle_fold_header_at_viewport_line(display_offset, 5);
        assert!(toggled);
        // Now folded label should appear at header viewport line
        let label = blocks.folded_label_at_viewport_line(display_offset, 5);
        assert!(label.is_some());
        // Unfold again
        let toggled2 = blocks.toggle_fold_header_at_viewport_line(display_offset, 5);
        assert!(toggled2);
        assert!(blocks.header_at_viewport_line(display_offset, 5).is_some());
    }

    #[test]
    fn chip_ranges_basic() {
        let header = "header"; // width = 6
        let ranges = Blocks::compute_header_chip_ranges(header);
        // [Copy] len 6, [Rerun] len 7, [Export] len 8
        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges[0], (8, 14)); // start 6+2, end 8+6
        assert_eq!(ranges[1], (15, 22)); // end prev + space = 15, len 7
        assert_eq!(ranges[2], (23, 31)); // next start + len 8
                                         // Ensure strictly increasing and non-overlapping
        assert!(ranges[0].1 < ranges[1].0 && ranges[1].1 < ranges[2].0);
    }

    #[test]
    fn chip_ranges_change_with_header_width() {
        let h1 = "";
        let h2 = "abcdefghij"; // 10 cols
        let r1 = Blocks::compute_header_chip_ranges(h1);
        let r2 = Blocks::compute_header_chip_ranges(h2);
        // Starts shift by +10 when header grows by 10
        assert_eq!(r2[0].0 - r1[0].0, 10);
        assert_eq!(r2[1].0 - r1[1].0, 10);
        assert_eq!(r2[2].0 - r1[2].0, 10);
    }

    #[test]
    fn chip_hit_truncated_ignores_offscreen() {
        // Header long enough so first chip starts at column == columns (off-screen)
        // header width 18 => first chip start = 20
        let header = "abcdefghijklmnopqr"; // width 18
        let columns = 20usize;
        // Clicking at the last visible column should not hit any chip
        assert_eq!(Blocks::chip_hit_at(header, 19, columns), None);
        // Clicking beyond columns must also not hit
        assert_eq!(Blocks::chip_hit_at(header, 20, columns), None);
    }

    #[test]
    fn chip_hit_partial_visibility_hits_visible_part() {
        // Header width 16 => first chip starts at 18, partially visible in columns=20
        let header = "abcdefghijklmnop"; // width 16
        let columns = 20usize;
        // Clicking in visible part should hit chip 0
        assert_eq!(Blocks::chip_hit_at(header, 18, columns), Some(0));
        assert_eq!(Blocks::chip_hit_at(header, 19, columns), Some(0));
        // Clicking at column == columns should not hit
        assert_eq!(Blocks::chip_hit_at(header, 20, columns), None);
    }
}
