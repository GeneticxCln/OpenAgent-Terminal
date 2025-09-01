//! AI panel for displaying command suggestions and interaction

use openagent_terminal_core::index::{Column, Point};
use openagent_terminal_core::term::LineDamageBounds;
use unicode_width::UnicodeWidthStr;

use crate::config::UiConfig;
use crate::display::Display;
use crate::display::color::Rgb;
use crate::renderer::rects::RenderRect;

/// Maximum lines to show for AI panel
const MAX_AI_PANEL_LINES: usize = 10;

/// AI panel label shown at the top
const AI_PANEL_LABEL: &str = "🤖 AI Assistant: ";

/// Loading indicator text
const LOADING_TEXT: &str = "⏳ Thinking...";

/// Error prefix
const ERROR_PREFIX: &str = "❌ Error: ";

/// Command suggestion prefix
const SUGGESTION_PREFIX: &str = "$ ";

/// Selection indicator
const SELECTION_INDICATOR: &str = "▶ ";

#[cfg(feature = "ai")]
impl Display {
    /// Draw the AI panel if it's active (legacy helper using caller-owned rect list)
    pub fn draw_ai_panel(
        &mut self,
        config: &UiConfig,
        ai_state: &crate::ai_runtime::AiUiState,
        rects: &mut Vec<RenderRect>,
    ) {
        if !ai_state.active {
            return;
        }

        let size_info = self.size_info;
        let num_cols = size_info.columns;
        let num_lines = size_info.screen_lines;
        
        // Calculate panel dimensions
        let panel_lines = std::cmp::min(MAX_AI_PANEL_LINES, num_lines / 3);
        let start_line = num_lines.saturating_sub(panel_lines);
        
        // Panel background
        let bg = config.colors.footer_bar_background();
        let fg = config.colors.footer_bar_foreground();
        
        // Draw panel background
        let panel_y = start_line as f32 * size_info.cell_height();
        let panel_height = panel_lines as f32 * size_info.cell_height();
        let panel_rect = RenderRect::new(
            0.0,
            panel_y,
            size_info.width(),
            panel_height,
            bg,
            0.8, // Semi-transparent
        );
        rects.push(panel_rect);
        
        // Damage the panel area
        for line_idx in start_line..num_lines {
            let damage = LineDamageBounds::new(line_idx, 0, num_cols);
            self.damage_tracker.frame().damage_line(damage);
            self.damage_tracker.next_frame().damage_line(damage);
        }
        
        let mut current_line = start_line;
        
        // Draw panel header
        let header_text = format!("{}{}", AI_PANEL_LABEL, ai_state.scratch);
        let header_point = Point::new(current_line, Column(0));
        self.draw_ai_text(header_point, fg, bg, &header_text, num_cols);
        current_line += 1;
        
        // Draw cursor at the end of input
        if ai_state.cursor_position > 0 {
            let cursor_col = Column(AI_PANEL_LABEL.width() + ai_state.cursor_position);
            if cursor_col.0 < num_cols {
                let cursor_point = Point::new(start_line, cursor_col);
                let cursor_bg = fg; // Inverted colors for cursor
                let cursor_fg = bg;
                self.draw_ai_text(cursor_point, cursor_fg, cursor_bg, "_", 1);
            }
        }
        
        // Draw separator line
        if current_line < num_lines {
            let separator = "─".repeat(num_cols);
            let separator_point = Point::new(current_line, Column(0));
            self.draw_ai_text(separator_point, fg, bg, &separator, num_cols);
            current_line += 1;
        }

        // Draw actions/help row
        if current_line < num_lines {
            let actions = "Actions: [Ctrl+I] Insert  [Ctrl+E] Apply (dry-run)  [Ctrl+Shift+C] Copy code  [Ctrl+Shift+A] Copy all  [Ctrl+R] Regenerate  [Ctrl+C] Stop   [? Esc] Close";
            let actions_point = Point::new(current_line, Column(2));
            // Dim slightly for hint badge
            let hint_color = Rgb::new(180, 200, 220);
            self.draw_ai_text(actions_point, hint_color, bg, actions, num_cols.saturating_sub(2));
            current_line += 1;
        }
        
        // Draw content based on state
        if ai_state.is_loading {
            // Show loading indicator
            if current_line < num_lines {
                let loading_point = Point::new(current_line, Column(2));
                self.draw_ai_text(loading_point, fg, bg, LOADING_TEXT, num_cols - 2);
                current_line += 1;
            }
            // Show streaming text (partial responses)
            if !ai_state.streaming_text.is_empty() {
                for line in ai_state.streaming_text.lines() {
                    if current_line >= num_lines { break; }
                    let text_point = Point::new(current_line, Column(2));
                    self.draw_ai_text(text_point, fg, bg, line, num_cols - 2);
                    current_line += 1;
                }
            }
        } else if let Some(ref error) = ai_state.error_message {
            // Show error message
            if current_line < num_lines {
                let error_text = format!("{}{}", ERROR_PREFIX, error);
                let error_point = Point::new(current_line, Column(2));
                let error_color = Rgb::new(255, 100, 100); // Light red
                self.draw_ai_text(error_point, error_color, bg, &error_text, num_cols - 2);
            }
        } else if !ai_state.streaming_text.is_empty() {
            // Show final streamed text when streaming done
            for line in ai_state.streaming_text.lines() {
                if current_line >= num_lines { break; }
                let text_point = Point::new(current_line, Column(2));
                self.draw_ai_text(text_point, fg, bg, line, num_cols - 2);
                current_line += 1;
            }
        } else if !ai_state.proposals.is_empty() {
            // Show proposals
            for (idx, proposal) in ai_state.proposals.iter().enumerate() {
                if current_line >= num_lines {
                    break;
                }
                
                // Add selection indicator
                let mut line_text = String::new();
                if idx == ai_state.selected_proposal {
                    line_text.push_str(SELECTION_INDICATOR);
                } else {
                    line_text.push_str("  ");
                }
                
                // Add command with prefix
                if let Some(first_cmd) = proposal.proposed_commands.first() {
                    line_text.push_str(SUGGESTION_PREFIX);
                    
                    // Truncate command if too long
                    let available_width = num_cols.saturating_sub(line_text.width());
                    if first_cmd.width() > available_width {
                        let truncated: String = first_cmd.chars()
                            .take(available_width.saturating_sub(3))
                            .collect();
                        line_text.push_str(&truncated);
                        line_text.push_str("...");
                    } else {
                        line_text.push_str(first_cmd);
                    }
                    
                    let text_point = Point::new(current_line, Column(0));
                    let text_color = if idx == ai_state.selected_proposal {
                        Rgb::new(100, 255, 100) // Light green for selected
                    } else {
                        fg
                    };
                    self.draw_ai_text(text_point, text_color, bg, &line_text, num_cols);
                    current_line += 1;
                    
                    // Show additional commands if any (indented)
                    for additional_cmd in proposal.proposed_commands.iter().skip(1) {
                        if current_line >= num_lines {
                            break;
                        }
                        
                        let indented = format!("    {}{}", SUGGESTION_PREFIX, additional_cmd);
                        let cmd_point = Point::new(current_line, Column(0));
                        self.draw_ai_text(cmd_point, fg, bg, &indented, num_cols);
                        current_line += 1;
                    }
                    
                    // Add description if present
                    if let Some(ref description) = proposal.description {
                        if current_line < num_lines {
                            let description_text = format!("    💡 {}", description);
                            let desc_point = Point::new(current_line, Column(0));
                            let desc_color = Rgb::new(200, 200, 200); // Slightly dimmed
                            self.draw_ai_text(desc_point, desc_color, bg, &description_text, num_cols);
                            current_line += 1;
                        }
                    }
                }
            }
        }
    }

    /// Draw the AI overlay immediately (background rects then text), independent of the main draw rect pipeline.
    pub fn draw_ai_overlay(
        &mut self,
        config: &UiConfig,
        ai_state: &crate::ai_runtime::AiUiState,
    ) {
        if !ai_state.active { return; }

        // Build background rect(s) just like draw_ai_panel does, then flush them.
        let size_info = self.size_info;
        let num_cols = size_info.columns;
        let num_lines = size_info.screen_lines;

        let panel_lines = std::cmp::min(MAX_AI_PANEL_LINES, num_lines / 3);
        let start_line = num_lines.saturating_sub(panel_lines);

        let bg = config.colors.footer_bar_background();
        let panel_y = start_line as f32 * size_info.cell_height();
        let panel_height = panel_lines as f32 * size_info.cell_height();
        let panel_rect = RenderRect::new(
            0.0,
            panel_y,
            size_info.width(),
            panel_height,
            bg,
            0.8,
        );
        let mut rects = Vec::with_capacity(1);
        rects.push(panel_rect);

        // Flush background rects now.
        let metrics = self.glyph_cache.font_metrics();
        let size_copy = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Now render the text using the same logic as draw_ai_panel, but in an isolated pass.
        let mut local_rects = Vec::new();
        self.draw_ai_panel(config, ai_state, &mut local_rects);
        if !local_rects.is_empty() {
            // Unlikely: currently only background used rects; handled above. Keep for completeness.
            let metrics = self.glyph_cache.font_metrics();
            let size_copy = self.size_info;
            self.renderer_draw_rects(&size_copy, &metrics, local_rects);
        }
    }
    
    /// Helper to draw text in the AI panel
    fn draw_ai_text(
        &mut self,
        point: Point<usize>,
        fg: Rgb,
        bg: Rgb,
        text: &str,
        max_width: usize,
    ) {
        let truncated_text: String = if text.width() > max_width {
            text.chars().take(max_width).collect()
        } else {
            text.to_string()
        };
        
        let size_info_copy = self.size_info;
        match &mut self.backend {
            crate::display::Backend::Gl { renderer, .. } => {
                renderer.draw_string(
                    point,
                    fg,
                    bg,
                    truncated_text.chars(),
                    &size_info_copy,
                    &mut self.glyph_cache,
                );
            },
            #[cfg(feature = "wgpu")]
            crate::display::Backend::Wgpu { renderer } => {
                renderer.draw_string(
                    point,
                    fg,
                    bg,
                    truncated_text.chars(),
                    &size_info_copy,
                    &mut self.glyph_cache,
                );
            },
        }
    }
}
