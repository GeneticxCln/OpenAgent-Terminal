#![allow(dead_code)]
// Workflows Panel UI: state and rendering
// Feature-gated under `workflow`

use unicode_width::UnicodeWidthStr;

use crate::config::UiConfig;
use crate::display::{Display, SizeInfo};
use crate::renderer::rects::RenderRect;
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Point};
use serde::{Deserialize, Serialize};

/// Source of the workflow definition (for future expansion)
#[derive(Clone, Debug)]
pub enum WorkflowSource {
    Config,
    #[allow(dead_code)]
    Engine,
}

/// Workflow history panel state
#[derive(Clone, Debug, Default)]
pub struct WorkflowHistoryPanelState {
    pub active: bool,
    pub executions: Vec<WorkflowExecutionSummary>,
    pub selected: usize,
    pub search_query: String,
    pub status_filter: Option<WorkflowExecutionStatus>,
}

/// Workflow execution summary for history display
#[derive(Clone, Debug)]
pub struct WorkflowExecutionSummary {
    pub id: String,
    pub workflow_name: String,
    pub status: WorkflowExecutionStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub duration_ms: Option<i64>,
    pub parameters_count: usize,
    pub has_outputs: bool,
    pub error_summary: Option<String>,
}

/// Workflow execution status for UI display
#[derive(Clone, Debug, PartialEq)]
pub enum WorkflowExecutionStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkflowParamsState {
    pub active: bool,
    pub workflow_id: Option<String>,
    pub workflow_name: Option<String>,
    pub fields: Vec<WorkflowParamField>,
    pub selected: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WorkflowParamType {
    String,
    Number,
    Boolean,
    Select,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowParamOption {
    pub value: serde_json::Value,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowParam {
    pub name: String,
    pub param_type: WorkflowParamType,
    pub description: String,
    pub required: bool,
    pub default: Option<serde_json::Value>,
    pub options: Option<Vec<WorkflowParamOption>>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowParamField {
    pub name: String,
    pub kind: WorkflowParamType,
    pub description: String,
    pub required: bool,
    pub value: serde_json::Value,
    pub options: Option<Vec<WorkflowParamOption>>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl WorkflowParamsState {
    pub fn setup(&mut self, id: String, name: String, params: Vec<WorkflowParam>) {
        self.active = true;
        self.workflow_id = Some(id);
        self.workflow_name = Some(name);
        self.selected = 0;
        self.fields = params
            .into_iter()
            .map(|p| WorkflowParamField {
                name: p.name,
                kind: p.param_type,
                description: p.description,
                required: p.required,
                value: p.default.unwrap_or(serde_json::Value::Null),
                options: p.options,
                min: p.min,
                max: p.max,
            })
            .collect();
    }
    pub fn open(&mut self, id: String, name: String, params: Vec<WorkflowParam>) {
        self.setup(id, name, params);
    }
    
    pub fn close(&mut self) {
        self.active = false;
        self.workflow_id = None;
        self.workflow_name = None;
        self.fields.clear();
        self.selected = 0;
    }
    #[allow(dead_code)]
    pub fn move_selection(&mut self, delta: isize) {
        if self.fields.is_empty() {
            return;
        }
        let len = self.fields.len() as isize;
        let mut idx = self.selected as isize + delta;
        if idx < 0 {
            idx = 0;
        }
        if idx >= len {
            idx = len - 1;
        }
        self.selected = idx as usize;
    }
}

impl Display {
    pub fn draw_workflows_progress_overlay(
        &mut self,
        config: &UiConfig,
        st: &WorkflowProgressState,
    ) {
        if !st.active {
            return;
        }
        let size_info = self.size_info;
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Fixed-height bar at bottom: 5 lines
        let lines = 5usize;
        let num_lines = size_info.screen_lines();
        if num_lines == 0 {
            return;
        }
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
                s if s.eq_ignore_ascii_case("running") || s.eq_ignore_ascii_case("starting") => {
                    ("● ", tokens.accent)
                }
                s if s.eq_ignore_ascii_case("success") => ("✔ ", tokens.success),
                s if s.eq_ignore_ascii_case("failed") => ("✖ ", tokens.error),
                s if s.eq_ignore_ascii_case("cancelled") || s.eq_ignore_ascii_case("canceled") => {
                    ("⚠ ", tokens.warning)
                }
                _ => ("• ", tokens.text_muted),
            };
            let status_text = format!("{}{}", badge, status);
            let col = Column(col_offset.min(max_cols));
            self.draw_ai_text(
                Point::new(start_line, col),
                color,
                bg,
                &status_text,
                max_cols.saturating_sub(col_offset),
            );
        }

        // Current step line with optional progress index/total
        let step_line = start_line + 1;
        let terminal_status = st
            .status
            .as_ref()
            .map(|s| {
                s.eq_ignore_ascii_case("success")
                    || s.eq_ignore_ascii_case("failed")
                    || s.eq_ignore_ascii_case("cancelled")
                    || s.eq_ignore_ascii_case("canceled")
            })
            .unwrap_or(false);

        if let Some(step) = &st.current_step {
            let prefix = match st.total_steps {
                Some(total) if st.step_index > 0 => format!("Step {}/{}: ", st.step_index, total),
                _ if st.step_index > 0 => format!("Step {}: ", st.step_index),
                _ => "Step: ".to_string(),
            };
            let s = format!("{}{}", prefix, step);
            self.draw_ai_text(
                Point::new(step_line, Column(1)),
                tokens.text_muted,
                bg,
                &s,
                size_info.columns().saturating_sub(2),
            );
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
            let mut line_text = label.to_string();
            if !st.seen_steps.is_empty() {
                line_text.push_str(&format!("  •  Steps: {}", st.seen_steps.len()));
            }
            self.draw_ai_text(
                Point::new(step_line, Column(1)),
                color,
                bg,
                &line_text,
                size_info.columns().saturating_sub(2),
            );
        }

        // Logs and optional footer hint on the last line when in terminal status
        let mut line = start_line + 2;
        let available_log_lines =
            if terminal_status { lines.saturating_sub(3) } else { lines.saturating_sub(2) };
        let max_log_line = start_line + 1 + available_log_lines; // exclusive upper bound
        let skip = st.logs.len().saturating_sub(available_log_lines);
        for log in st.logs.iter().skip(skip) {
            if line >= max_log_line {
                break;
            }
            self.draw_ai_text(
                Point::new(line, Column(1)),
                fg,
                bg,
                log,
                size_info.columns().saturating_sub(2),
            );
            line += 1;
        }

        if terminal_status {
            let footer_line = start_line + lines - 1;
            let hint = "Esc: Dismiss";
            self.draw_ai_text(
                Point::new(footer_line, Column(1)),
                tokens.text_muted,
                bg,
                hint,
                size_info.columns().saturating_sub(2),
            );
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

impl Default for WorkflowsPanelState {
    fn default() -> Self {
        Self::new()
    }
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
        if self.results.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.results.len() as isize;
        let mut idx = self.selected as isize + delta;
        if idx < 0 {
            idx = 0;
        }
        if idx >= len {
            idx = len - 1;
        }
        self.selected = idx as usize;
    }
}

impl WorkflowHistoryPanelState {
    pub fn new() -> Self {
        Self {
            active: false,
            executions: Vec::new(),
            selected: 0,
            search_query: String::new(),
            status_filter: None,
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.executions.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.executions.len() as isize;
        let mut idx = self.selected as isize + delta;
        if idx < 0 {
            idx = 0;
        }
        if idx >= len {
            idx = len - 1;
        }
        self.selected = idx as usize;
    }

    pub fn get_selected_execution(&self) -> Option<&WorkflowExecutionSummary> {
        self.executions.get(self.selected)
    }

    pub fn update_executions(&mut self, executions: Vec<WorkflowExecutionSummary>) {
        self.executions = executions;
        if self.selected >= self.executions.len() {
            self.selected = self.executions.len().saturating_sub(1);
        }
    }
}

impl Display {
    /// Draw the Workflows panel (bottom overlay). This draws both background rects and text.
    pub fn draw_workflows_panel_overlay(&mut self, config: &UiConfig, state: &WorkflowsPanelState) {
        if !state.active {
            return;
        }
        let size_info = self.size_info;
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Panel sizing: 35% of viewport height, min 6 lines
        let num_lines = size_info.screen_lines();
        let target_lines = ((num_lines as f32 * 0.35).round() as usize).clamp(6, num_lines);
        let start_line = num_lines.saturating_sub(target_lines);
        let panel_y = start_line as f32 * size_info.cell_height();
        let panel_h = target_lines as f32 * size_info.cell_height();

        // Backdrop dim
        let backdrop =
            RenderRect::new(0.0, 0.0, size_info.width(), size_info.height(), tokens.overlay, 0.20);
        // Panel background
        let panel_bg =
            RenderRect::new(0.0, panel_y, size_info.width(), panel_h, tokens.surface_muted, 0.95);

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
        if cursor_col >= num_cols {
            cursor_col = num_cols.saturating_sub(1);
        }
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
            if line > max_lines {
                break;
            }
            let mut row = String::new();
            if idx == state.selected {
                row.push_str("▶ ");
            } else {
                row.push_str("  ");
            }
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
        let hint = "Enter: Paste  •  Esc: Close  •  ↑/↓/PgUp/PgDn: Navigate  •  Ctrl+N/Ctrl+P: \
                    Navigate  •  Backspace: Delete";
        let hint_fg = tokens.text_muted;
        self.draw_ai_text(Point::new(footer_line, Column(2)), hint_fg, bg, hint, num_cols - 2);
    }
}

impl Display {
    /// Draw the workflow history panel overlay
    pub fn draw_workflow_history_panel_overlay(
        &mut self,
        config: &UiConfig,
        state: &WorkflowHistoryPanelState,
    ) {
        if !state.active {
            return;
        }
        let size_info = self.size_info;
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Panel sizing: 40% of viewport height, min 8 lines
        let num_lines = size_info.screen_lines();
        let target_lines = ((num_lines as f32 * 0.40).round() as usize).clamp(8, num_lines);
        let start_line = num_lines.saturating_sub(target_lines);
        let panel_y = start_line as f32 * size_info.cell_height();
        let panel_h = target_lines as f32 * size_info.cell_height();

        // Backdrop dim
        let backdrop =
            RenderRect::new(0.0, 0.0, size_info.width(), size_info.height(), tokens.overlay, 0.20);
        // Panel background
        let panel_bg =
            RenderRect::new(0.0, panel_y, size_info.width(), panel_h, tokens.surface_muted, 0.95);

        // Stage rects then draw them
        let rects = vec![backdrop, panel_bg];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Header and content
        let num_cols = size_info.columns();
        let fg = tokens.text;
        let bg = tokens.surface_muted;

        let mut line = start_line;

        // Header with execution count
        let count = state.executions.len();
        let header = if count == 1 {
            format!("Workflow History — {} execution", count)
        } else {
            format!("Workflow History — {} executions", count)
        };
        self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &header, num_cols - 2);
        line += 1;

        // Search/filter bar
        let mut search_bar = String::from("🔍 ");
        if !state.search_query.is_empty() {
            search_bar.push_str(&state.search_query);
        } else {
            search_bar.push_str("Search executions...");
        }
        
        // Add status filter indicator
        if let Some(status) = &state.status_filter {
            search_bar.push_str(&format!(" [{}]", status_to_display_string(status)));
        }
        
        self.draw_ai_text(Point::new(line, Column(0)), fg, bg, &search_bar, num_cols);
        line += 1;

        // Separator
        let sep = "─".repeat(num_cols);
        self.draw_ai_text(Point::new(line, Column(0)), fg, bg, &sep, num_cols);
        line += 1;

        // Compute footer line and results area
        let footer_line = start_line + target_lines - 1;
        let max_lines = footer_line.saturating_sub(1);

        // Execution list
        for (idx, execution) in state.executions.iter().enumerate() {
            if line > max_lines {
                break;
            }

            let mut row = String::new();
            if idx == state.selected {
                row.push_str("▶ ");
            } else {
                row.push_str("  ");
            }

            // Status indicator
            let status_indicator = match execution.status {
                WorkflowExecutionStatus::Success => "✔",
                WorkflowExecutionStatus::Failed => "✖",
                WorkflowExecutionStatus::Running => "⏳",
                WorkflowExecutionStatus::Cancelled => "⚠",
                WorkflowExecutionStatus::Pending => "⏸",
            };
            row.push_str(status_indicator);
            row.push(' ');

            // Workflow name and timing
            row.push_str(&execution.workflow_name);
            
            // Duration if available
            if let Some(duration_ms) = execution.duration_ms {
                if duration_ms < 1000 {
                    row.push_str(&format!(" ({}ms)", duration_ms));
                } else {
                    row.push_str(&format!(" ({:.1}s)", duration_ms as f64 / 1000.0));
                }
            }

            // Parameters count
            if execution.parameters_count > 0 {
                row.push_str(&format!(" [{}p]", execution.parameters_count));
            }

            // Error summary if failed
            if let Some(error) = &execution.error_summary {
                row.push_str(" — ");
                row.push_str(error);
            }

            // Truncate if too long
            let remaining = num_cols;
            if row.width() > remaining {
                let truncated: String = row.chars().take(remaining.saturating_sub(3)).collect();
                row = truncated + "...";
            }

            // Color based on status
            let row_color = match execution.status {
                WorkflowExecutionStatus::Success => tokens.success,
                WorkflowExecutionStatus::Failed => tokens.error,
                WorkflowExecutionStatus::Running => tokens.accent,
                WorkflowExecutionStatus::Cancelled => tokens.warning,
                WorkflowExecutionStatus::Pending => tokens.text_muted,
            };

            self.draw_ai_text(Point::new(line, Column(0)), row_color, bg, &row, num_cols);
            line += 1;
        }

        // Footer controls/hints
        let hint = "Enter: Re-run  •  V: View Details  •  D: Delete  •  F: Filter  •  Esc: Close  •  ↑/↓: Navigate";
        let hint_fg = tokens.text_muted;
        self.draw_ai_text(Point::new(footer_line, Column(2)), hint_fg, bg, hint, num_cols - 2);
    }

    pub fn draw_workflows_params_overlay(&mut self, config: &UiConfig, st: &WorkflowParamsState) {
        if !st.active {
            return;
        }
        let size_info = self.size_info;
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        let cols = size_info.columns();
        let total_lines = size_info.screen_lines();
        if total_lines == 0 {
            return;
        }
        let panel_cols = ((cols as f32 * 0.6).round() as usize).clamp(40, cols.saturating_sub(2));
        let panel_lines = (total_lines as f32 * 0.5).round() as usize;
        let start_line = (total_lines.saturating_sub(panel_lines)) / 2;
        let start_col = (cols.saturating_sub(panel_cols)) / 2;

        // Rects
        let panel_x = start_col as f32 * size_info.cell_width();
        let panel_y = start_line as f32 * size_info.cell_height();
        let panel_w = panel_cols as f32 * size_info.cell_width();
        let panel_h = panel_lines as f32 * size_info.cell_height();
        let rects = vec![
            RenderRect::new(0.0, 0.0, size_info.width(), size_info.height(), tokens.overlay, 0.18),
            RenderRect::new(panel_x, panel_y, panel_w, panel_h, tokens.surface, 0.98),
        ];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Header
        let title = if let Some(name) = &st.workflow_name {
            format!("Run: {}", name)
        } else {
            "Run Workflow".to_string()
        };
        self.draw_ai_text(
            Point::new(start_line, Column(start_col + 2)),
            tokens.text,
            tokens.surface,
            &title,
            panel_cols.saturating_sub(4),
        );

        // Fields area
        let mut line = start_line + 2;
        let max_line = start_line + panel_lines - 2;
        for (i, field) in st.fields.iter().enumerate() {
            if line >= max_line {
                break;
            }
            let marker = if i == st.selected { "▶ " } else { "  " };
            let label = format!("{}{}:", marker, field.name);
            self.draw_ai_text(
                Point::new(line, Column(start_col + 2)),
                tokens.text,
                tokens.surface,
                &label,
                panel_cols.saturating_sub(4),
            );
            let value_str = match &field.value {
                serde_json::Value::Null => "".to_string(),
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => "".to_string(),
            };
            let value_col = start_col + 2 + label.len() + 1;
            self.draw_ai_text(
                Point::new(line, Column(value_col)),
                tokens.text_muted,
                tokens.surface,
                &value_str,
                panel_cols.saturating_sub(6 + label.len()),
            );
            line += 1;
        }

        // Footer
        let footer = "Enter: Run    Esc: Cancel    Tab/Shift+Tab: Next/Prev    Space: Toggle";
        self.draw_ai_text(
            Point::new(start_line + panel_lines - 1, Column(start_col + 2)),
            tokens.text_muted,
            tokens.surface,
            footer,
            panel_cols.saturating_sub(4),
        );
    }
}

/// Convert workflow execution status to display string
fn status_to_display_string(status: &WorkflowExecutionStatus) -> &'static str {
    match status {
        WorkflowExecutionStatus::Pending => "Pending",
        WorkflowExecutionStatus::Running => "Running", 
        WorkflowExecutionStatus::Success => "Success",
        WorkflowExecutionStatus::Failed => "Failed",
        WorkflowExecutionStatus::Cancelled => "Cancelled",
    }
}
