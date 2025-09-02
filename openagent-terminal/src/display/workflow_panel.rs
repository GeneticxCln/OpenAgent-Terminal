// Workflows Panel UI: state and rendering
// Feature-gated under `workflow`

#![cfg(feature = "workflow")]

use unicode_width::UnicodeWidthStr;

use crate::config::UiConfig;
use crate::display::color::Rgb;
use crate::display::{Display, SizeInfo};
use crate::renderer::rects::RenderRect;
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Point};

/// Source of the workflow definition (for future expansion)
#[derive(Clone, Debug)]
pub enum WorkflowSource {
    Config,
    #[allow(dead_code)]
    Engine,
}

#[derive(Clone, Debug, Default)]
pub struct WorkflowProgressState {
    pub active: bool,
    pub execution_id: Option<String>,
    pub workflow_name: Option<String>,
    pub status: Option<String>,
    pub current_step: Option<String>,
    pub logs: Vec<String>,
    // Progress metadata (best-effort): step index and optional total
    pub step_index: usize,
    pub total_steps: Option<usize>,
    pub seen_steps: Vec<String>,
}

impl Display {
    pub fn draw_workflows_progress_overlay(
        &mut self,
        config: &UiConfig,
        st: &WorkflowProgressState,
    ) {
        if !st.active { return; }
        let size_info = self.size_info;
        let theme = config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Fixed-height bar at bottom: 5 lines
        let lines = 5usize;
        let num_lines = size_info.screen_lines();
        if num_lines == 0 { return; }
        let start_line = num_lines.saturating_sub(lines);
        let y = start_line as f32 * size_info.cell_height();
        let h = (lines as f32) * size_info.cell_height();

        // Background
        let rects = vec![
            RenderRect::new(0.0, 0.0, size_info.width(), size_info.height(), tokens.overlay, 0.12),
            RenderRect::new(0.0, y, size_info.width(), h, tokens.surface, 0.96),
        ];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Header with status badge and colored status
        let fg = tokens.text;
        let bg = tokens.surface;
        let mut base = String::from("Workflow: ");
        let name_or_id = if let Some(name) = &st.workflow_name {
            name.clone()
        } else if let Some(exec) = &st.execution_id {
            exec.clone()
        } else {
            "(running)".into()
        };
        base.push_str(&name_or_id);
        base.push_str(" — ");

        // Draw base header portion
        let header_point = Point::new(start_line, Column(1));
        let max_cols = size_info.columns().saturating_sub(2);
        self.draw_ai_text(header_point, fg, bg, &base, max_cols);

        // Draw colored status next to it
        if let Some(status) = &st.status {
            let col_offset = 1 + base.width();
            let (badge, color) = match status.as_str() {
                s if s.eq_ignore_ascii_case("running") || s.eq_ignore_ascii_case("starting") => ("● ", tokens.accent),
                s if s.eq_ignore_ascii_case("success") => ("✔ ", tokens.success),
                s if s.eq_ignore_ascii_case("failed") => ("✖ ", tokens.error),
                s if s.eq_ignore_ascii_case("cancelled") || s.eq_ignore_ascii_case("canceled") => ("⚠ ", tokens.warning),
                _ => ("• ", tokens.text_muted),
            };
            let status_text = format!("{}{}", badge, status);
            let col = Column(col_offset.min(max_cols));
            self.draw_ai_text(Point::new(start_line, col), color, bg, &status_text, max_cols.saturating_sub(col_offset));
        }

        // Current step line with optional progress index/total
        let step_line = start_line + 1;
        let terminal_status = st
            .status
            .as_ref()
            .map(|s| s.eq_ignore_ascii_case("success")
                || s.eq_ignore_ascii_case("failed")
                || s.eq_ignore_ascii_case("cancelled")
                || s.eq_ignore_ascii_case("canceled"))
            .unwrap_or(false);

        if let Some(step) = &st.current_step {
            let prefix = match st.total_steps {
                Some(total) if st.step_index > 0 => format!("Step {}/{}: ", st.step_index, total),
                _ if st.step_index > 0 => format!("Step {}: ", st.step_index),
                _ => "Step: ".to_string(),
            };
            let s = format!("{}{}", prefix, step);
            self.draw_ai_text(Point::new(step_line, Column(1)), tokens.text_muted, bg, &s, size_info.columns().saturating_sub(2));
        } else if terminal_status {
            // If completed without a current_step, show final status prominently
            let status = st.status.as_ref().unwrap();
            let (label, color) = if status.eq_ignore_ascii_case("success") {
                ("Completed successfully", tokens.success)
            } else if status.eq_ignore_ascii_case("failed") {
                ("Completed with errors", tokens.error)
            } else {
                ("Cancelled", tokens.warning)
            };
            self.draw_ai_text(Point::new(step_line, Column(1)), color, bg, label, size_info.columns().saturating_sub(2));
        }

        // Logs and optional footer hint on the last line when in terminal status
        let mut line = start_line + 2;
        let available_log_lines = if terminal_status { lines.saturating_sub(3) } else { lines.saturating_sub(2) };
        let max_log_line = start_line + 1 + available_log_lines; // exclusive upper bound
        let skip = st.logs.len().saturating_sub(available_log_lines);
        for log in st.logs.iter().skip(skip) {
            if line >= max_log_line { break; }
            self.draw_ai_text(Point::new(line, Column(1)), fg, bg, log, size_info.columns().saturating_sub(2));
            line += 1;
        }

        if terminal_status {
            let footer_line = start_line + lines - 1;
            let hint = "Esc: Dismiss";
            self.draw_ai_text(Point::new(footer_line, Column(1)), tokens.text_muted, bg, hint, size_info.columns().saturating_sub(2));
        }
    }
}

/// One item in the workflows list (UI summary)
#[derive(Clone, Debug)]
pub struct WorkflowItem {
    pub name: String,
    pub description: Option<String>,
    pub source: WorkflowSource,
}

#[derive(Clone, Debug)]
pub struct WorkflowsPanelState {
    pub active: bool,
    pub query: String,
    pub results: Vec<WorkflowItem>,
    pub selected: usize,
}

impl WorkflowsPanelState {
    pub fn new() -> Self {
        Self { active: false, query: String::new(), results: Vec::new(), selected: 0 }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.results.is_empty() { self.selected = 0; return; }
        let len = self.results.len() as isize;
        let mut idx = self.selected as isize + delta;
        if idx < 0 { idx = 0; }
        if idx >= len { idx = len - 1; }
        self.selected = idx as usize;
    }
}

impl Display {
    /// Draw the Workflows panel (bottom overlay). This draws both background rects and text.
    pub fn draw_workflows_panel_overlay(
        &mut self,
        config: &UiConfig,
        state: &WorkflowsPanelState,
    ) {
        if !state.active { return; }
        let size_info = self.size_info;
        let theme = config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Panel sizing: 35% of viewport height, min 6 lines
        let num_lines = size_info.screen_lines();
        let target_lines = ((num_lines as f32 * 0.35).round() as usize).clamp(6, num_lines);
        let start_line = num_lines.saturating_sub(target_lines);
        let panel_y = start_line as f32 * size_info.cell_height();
        let panel_h = target_lines as f32 * size_info.cell_height();

        // Backdrop dim
        let backdrop = RenderRect::new(0.0, 0.0, size_info.width(), size_info.height(), tokens.overlay, 0.20);
        // Panel background
        let panel_bg = RenderRect::new(0.0, panel_y, size_info.width(), panel_h, tokens.surface_muted, 0.95);

        // Stage rects then draw them
        let rects = vec![backdrop, panel_bg];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy: SizeInfo = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Header and content
        let num_cols = size_info.columns();
        let fg = tokens.text;
        let bg = tokens.surface_muted;

        let mut line = start_line;
        // Header with result count
        let count = state.results.len();
        let header = if count == 1 {
            format!("Workflows — {} result", count)
        } else {
            format!("Workflows — {} results", count)
        };
        self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &header, num_cols - 2);
        line += 1;

        // Query prompt
        let prompt_prefix = "🔧 ";
        let mut prompt = String::with_capacity(prompt_prefix.len() + state.query.len());
        prompt.push_str(prompt_prefix);
        prompt.push_str(&state.query);
        self.draw_ai_text(Point::new(line, Column(0)), fg, bg, &prompt, num_cols);
        let mut cursor_col = prompt_prefix.width() + state.query.width();
        if cursor_col >= num_cols { cursor_col = num_cols.saturating_sub(1); }
        self.draw_ai_text(Point::new(line, Column(cursor_col)), bg, fg, " ", 1);
        line += 1;

        // Separator
        let sep = "─".repeat(num_cols);
        self.draw_ai_text(Point::new(line, Column(0)), fg, bg, &sep, num_cols);
        line += 1;

        // Compute footer line and results area
        let footer_line = start_line + target_lines - 1;
        // Results list (reserve one line for footer)
        let max_lines = footer_line.saturating_sub(1);
        for (idx, item) in state.results.iter().enumerate() {
            if line > max_lines { break; }
            let mut row = String::new();
            if idx == state.selected { row.push_str("▶ "); } else { row.push_str("  "); }
            // Build display text: name — description (truncated)
            row.push_str(&item.name);
            if let Some(desc) = &item.description {
                row.push_str(" — ");
                row.push_str(desc);
            }
            let remaining = num_cols;
            if row.width() > remaining {
                let truncated: String = row.chars().take(remaining.saturating_sub(3)).collect();
                row = truncated + "...";
            }
            self.draw_ai_text(Point::new(line, Column(0)), fg, bg, &row, num_cols);
            line += 1;
        }

        // Footer controls/hints
        let hint = "Enter: Paste  •  Esc: Close  •  ↑/↓/PgUp/PgDn: Navigate  •  Ctrl+N/Ctrl+P: Navigate  •  Backspace: Delete";
        let hint_fg = tokens.text_muted;
        self.draw_ai_text(Point::new(footer_line, Column(2)), hint_fg, bg, hint, num_cols - 2);
    }
}

