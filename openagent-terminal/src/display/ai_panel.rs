//! AI panel for displaying command suggestions and interaction

use openagent_terminal_core::index::{Column, Point};
use openagent_terminal_core::term::LineDamageBounds;
use unicode_width::UnicodeWidthStr;

use crate::config::UiConfig;
use crate::display::animation::compute_progress;
use crate::display::color::Rgb;
use crate::display::Display;
use crate::renderer::rects::RenderRect;
use crate::renderer::ui::UiRoundedRect;

/// Maximum lines to show for AI panel
const MAX_AI_PANEL_LINES: usize = 10;

/// AI panel label shown at the top
#[allow(dead_code)]
const AI_PANEL_LABEL: &str = "🤖 AI Assistant: ";

/// Loading indicator text
#[allow(dead_code)]
const LOADING_TEXT: &str = "⏳ Thinking...";

/// Error prefix
#[allow(dead_code)]
const ERROR_PREFIX: &str = "❌ Error: ";

/// Command suggestion prefix
#[allow(dead_code)]
const SUGGESTION_PREFIX: &str = "$ ";

/// Selection indicator
#[allow(dead_code)]
const SELECTION_INDICATOR: &str = "▶ ";

#[cfg(feature = "ai")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiHeaderControl {
    Stop,
    Regenerate,
    Close,
}

#[cfg(feature = "ai")]
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct AiPanelGeometry {
    pub start_line: usize,
    pub anim_lines: usize,
    pub header_line: usize,
    pub separator_line: usize,
    pub prompt_line: usize,
    pub controls_col_start: usize,
    pub controls_col_end: usize,
}

#[cfg(feature = "ai")]
impl Display {
    /// Draw the AI panel if it's active (legacy helper using caller-owned rect list)
    #[allow(dead_code)]
    pub fn draw_ai_panel(
        &mut self,
        config: &UiConfig,
        ai_state: &crate::ai_runtime::AiUiState,
        rects: &mut Vec<RenderRect>,
    ) {
        // Detect visibility toggle to start animation.
        if ai_state.active != self.ai_panel_last_active {
            self.ai_panel_last_active = ai_state.active;
            self.ai_panel_anim_start = Some(std::time::Instant::now());
            self.ai_panel_anim_opening = ai_state.active;
            // Theme-aware duration; respect reduce motion
            let reduce_motion =
                config.resolved_theme.as_ref().map(|t| t.ui.reduce_motion).unwrap_or(false);
            self.ai_panel_anim_duration_ms = if reduce_motion {
                0
            } else if ai_state.active {
                160
            } else {
                140
            };
        }

        // Compute animation progress (0..1) with an ease-out curve using the shared animation util.
        let progress = compute_progress(
            self.ai_panel_anim_start,
            self.ai_panel_anim_duration_ms.max(1),
            self.ai_panel_anim_opening,
            ai_state.active,
        );
        if progress >= 1.0 || (!ai_state.active && progress <= 0.0) {
            // End animation.
            self.ai_panel_anim_start = None;
        }

        // If panel fully hidden and not active, skip drawing.
        if progress <= 0.0 && !ai_state.active {
            return;
        }

        let size_info = self.size_info;
        let num_cols = size_info.columns;
        let num_lines = size_info.screen_lines;

        // Resolve theme tokens/ui for panel visuals
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;
        let tui = theme.ui;

        // Backdrop dim behind the panel using theme overlay color with configurable alpha
        #[allow(unused_variables)]
        let backdrop_alpha = {
            #[cfg(feature = "ai")]
            {
                (config.ai.backdrop_alpha * progress).clamp(0.0, 1.0)
            }
            #[cfg(not(feature = "ai"))]
            {
                0.0
            }
        };
        if backdrop_alpha > 0.0 {
            let full = RenderRect::new(
                0.0,
                0.0,
                size_info.width(),
                size_info.height(),
                tokens.overlay,
                backdrop_alpha,
            );
            rects.push(full);
        }

        // Calculate panel dimensions (animated height) using fraction of viewport.
        let fraction = {
            #[cfg(feature = "ai")]
            {
                config.ai.panel_height_fraction.clamp(0.20, 0.60)
            }
            #[cfg(not(feature = "ai"))]
            {
                0.40
            }
        };
        let target_lines = ((num_lines as f32 * fraction).round() as usize)
            .clamp(6, MAX_AI_PANEL_LINES.max(6).min(num_lines));
        let anim_lines =
            ((target_lines as f32 * progress).ceil() as usize).min(target_lines).max(1);
        let start_line = num_lines.saturating_sub(anim_lines);

        // Panel background/foreground from theme
        let bg = tokens.surface_muted;
        let fg = tokens.text;

        // Compute panel geometry (pixels)
        let panel_y = start_line as f32 * size_info.cell_height();
        let panel_height = anim_lines as f32 * size_info.cell_height();
        let panel_alpha = 0.95_f32.clamp(0.0, 1.0) * progress;

        // Stage shadow as a separate rounded rect (simple soft shadow)
        if tui.shadow {
            let spread = tui.shadow_size_px.max(1) as f32;
            let offset_y = (tui.shadow_size_px as f32 * 0.5).round();
            let shadow_alpha = (tui.shadow_alpha * progress).min(1.0);
            if shadow_alpha > 0.0 {
                let shadow = UiRoundedRect::new(
                    -spread,
                    panel_y + offset_y - spread,
                    size_info.width() + spread * 2.0,
                    panel_height + spread * 2.0,
                    if tui.rounded_corners { tui.corner_radius_px + spread } else { 0.0 },
                    Rgb::new(0, 0, 0),
                    shadow_alpha,
                );
                self.stage_ui_rounded_rect(shadow);
            }
        }

        // Stage main rounded panel
        let radius = if tui.rounded_corners { tui.corner_radius_px } else { 0.0 };
        let panel = UiRoundedRect::new(
            0.0,
            panel_y,
            size_info.width(),
            panel_height,
            radius,
            bg,
            panel_alpha,
        );
        self.stage_ui_rounded_rect(panel);

        // Damage the panel area
        for line_idx in start_line..num_lines {
            let damage = LineDamageBounds::new(line_idx, 0, num_cols);
            self.damage_tracker.frame().damage_line(damage);
            self.damage_tracker.next_frame().damage_line(damage);
        }

        let mut current_line = start_line;

        // Reserve bottom input row like Warp: separator + prompt at the bottom line of the sheet.
        let prompt_line = start_line + anim_lines - 1;
        let separator_line = prompt_line.saturating_sub(1);

        // Content area is start_line..separator_line
        // We will render content first, then draw separator and prompt last.

        // Draw content header (small, optional)
        if current_line <= separator_line {
            // Left header label
            let header_text = "AI".to_string();
            let header_point = Point::new(current_line, Column(1));
            self.draw_ai_text(header_point, fg, bg, &header_text, num_cols.saturating_sub(1));
            // Right controls
            // Draw each control separately so we can color/disable/hover them individually
            let ctrl_len = "⏹ ⟳ ✖".chars().count();
            // Warp-like: keep a 3-column right margin for the control group
            let ctrl_col = num_cols.saturating_sub(ctrl_len + 3);
            let stop_col = ctrl_col;
            let regen_col = ctrl_col + 2;
            let close_col = ctrl_col + 4;

            // Determine enabled state (Warp-like: Stop enabled while streaming; Regenerate enabled
            // when idle)
            let (stop_enabled, regen_enabled) = {
                // We cannot access ai_state fields here directly; use passed ai_state
                if ai_state.is_loading || ai_state.streaming_active {
                    (true, false)
                } else {
                    (false, true)
                }
            };

            let ctrl_color_enabled = tokens.accent;
            let ctrl_color_disabled = tokens.text_muted;

            // Hover highlight background (subtle rounded capsule behind the glyph)
            if let Some(hovered) = self.ai_hover_control {
                let cell_w = size_info.cell_width();
                let cell_h = size_info.cell_height();
                let y = current_line as f32 * cell_h;
                let (x_col, enabled) = match hovered {
                    AiHeaderControl::Stop => (stop_col, stop_enabled),
                    AiHeaderControl::Regenerate => (regen_col, regen_enabled),
                    AiHeaderControl::Close => (close_col, true),
                };
                if enabled {
                    let x = x_col as f32 * cell_w;
                    // Slightly larger than a cell to look like a pill
                    let hover_rect = UiRoundedRect::new(
                        x - cell_w * 0.15,
                        y + cell_h * 0.18,
                        cell_w * 1.30,
                        cell_h * 0.64,
                        cell_h * 0.28,
                        Rgb::new(255, 255, 255),
                        0.10,
                    );
                    self.stage_ui_rounded_rect(hover_rect);
                }
            }

            // Draw glyphs
            let stop_fg = if stop_enabled { ctrl_color_enabled } else { ctrl_color_disabled };
            let regen_fg = if regen_enabled { ctrl_color_enabled } else { ctrl_color_disabled };
            let close_fg = ctrl_color_enabled;

            self.draw_ai_text(Point::new(current_line, Column(stop_col)), stop_fg, bg, "⏹", 1);
            self.draw_ai_text(Point::new(current_line, Column(regen_col)), regen_fg, bg, "⟳", 1);
            self.draw_ai_text(Point::new(current_line, Column(close_col)), close_fg, bg, "✖", 1);

            current_line += 1;
        }

        // Draw actions/help row or hover tooltip
        if current_line < num_lines {
            let actions_point = Point::new(current_line, Column(2));
            if let Some(control) = self.ai_hover_control {
                let tooltip = match control {
                    AiHeaderControl::Stop => "Stop (Ctrl+C)",
                    AiHeaderControl::Regenerate => "Regenerate (Ctrl+R)",
                    AiHeaderControl::Close => "Close (Esc)",
                };
                let tip_color = tokens.text_muted;
                self.draw_ai_text(
                    actions_point,
                    tip_color,
                    bg,
                    tooltip,
                    num_cols.saturating_sub(2),
                );
            } else {
                let actions = "Actions: [Ctrl+I] Insert  [Ctrl+E] Apply (dry-run)  [Ctrl+Shift+C] \
                               Copy code  [Ctrl+Shift+T] Copy all  [Ctrl+X] Explain  [Ctrl+F] Fix  \
                               [Ctrl+R] Regenerate  [Ctrl+C] Stop  [Esc] Close";
                // Dim slightly for hint badge
                let hint_color = tokens.text_muted;
                self.draw_ai_text(
                    actions_point,
                    hint_color,
                    bg,
                    actions,
                    num_cols.saturating_sub(2),
                );
            }
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
                    if current_line >= num_lines {
                        break;
                    }
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
                let error_color = tokens.error;
                self.draw_ai_text(error_point, error_color, bg, &error_text, num_cols - 2);
            }
        } else if !ai_state.streaming_text.is_empty() {
            // Show final streamed text when streaming done
            for line in ai_state.streaming_text.lines() {
                if current_line >= num_lines {
                    break;
                }
                let text_point = Point::new(current_line, Column(2));
                self.draw_ai_text(text_point, fg, bg, line, num_cols - 2);
                current_line += 1;
            }
        } else if !ai_state.proposals.is_empty() {
            // Show proposals
            for (idx, proposal) in ai_state.proposals.iter().enumerate() {
                if current_line >= separator_line {
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
                        let truncated: String =
                            first_cmd.chars().take(available_width.saturating_sub(3)).collect();
                        line_text.push_str(&truncated);
                        line_text.push_str("...");
                    } else {
                        line_text.push_str(first_cmd);
                    }

                    let text_point = Point::new(current_line, Column(0));
                    let text_color =
                        if idx == ai_state.selected_proposal { tokens.success } else { fg };
                    self.draw_ai_text(text_point, text_color, bg, &line_text, num_cols);
                    current_line += 1;

                    // Show additional commands if any (indented)
                    for additional_cmd in proposal.proposed_commands.iter().skip(1) {
                        if current_line >= separator_line {
                            break;
                        }

                        let indented = format!("    {}{}", SUGGESTION_PREFIX, additional_cmd);
                        let cmd_point = Point::new(current_line, Column(0));
                        self.draw_ai_text(cmd_point, fg, bg, &indented, num_cols);
                        current_line += 1;
                    }

                    // Add description if present
                    if let Some(ref description) = proposal.description {
                        if current_line <= separator_line {
                            let description_text = format!("    💡 {}", description);
                            let desc_point = Point::new(current_line, Column(0));
                            let desc_color = tokens.text_muted;
                            self.draw_ai_text(
                                desc_point,
                                desc_color,
                                bg,
                                &description_text,
                                num_cols,
                            );
                            current_line += 1;
                        }
                    }
                }
            }
        }

        // Draw separator just above the prompt
        if separator_line >= start_line {
            let separator = "─".repeat(num_cols);
            let separator_point = Point::new(separator_line, Column(0));
            self.draw_ai_text(separator_point, fg, bg, &separator, num_cols);
        }

        // Draw bottom prompt row (Warp-like)
        let prefix = "🤖 ";
        let mut prompt = String::with_capacity(prefix.len() + ai_state.scratch.len());
        prompt.push_str(prefix);
        prompt.push_str(&ai_state.scratch);
        let prompt_point = Point::new(prompt_line, Column(0));
        self.draw_ai_text(prompt_point, fg, bg, &prompt, num_cols);

        // Draw cursor at prompt line
        let base = unicode_width::UnicodeWidthStr::width(prefix);
        let mut cursor_col = base + ai_state.cursor_position;
        if cursor_col >= num_cols {
            cursor_col = num_cols.saturating_sub(1);
        }
        let cursor_point = Point::new(prompt_line, Column(cursor_col));
        self.draw_ai_text(cursor_point, bg, fg, " ", 1);
    }

    /// Compute current AI panel geometry for hit testing (returns None if panel fully hidden).
    pub fn ai_panel_geometry(
        &mut self,
        config: &UiConfig,
        ai_state: &crate::ai_runtime::AiUiState,
    ) -> Option<AiPanelGeometry> {
        // Animation progress like in draw_ai_panel/draw_ai_overlay.
        let progress = if let Some(start) = self.ai_panel_anim_start {
            let elapsed = start.elapsed().as_millis() as u32;
            let dur = self.ai_panel_anim_duration_ms.max(1);
            let t = (elapsed as f32 / dur as f32).clamp(0.0, 1.0);
            let eased = 1.0 - (1.0 - t).powi(3);
            if t >= 1.0 {
                self.ai_panel_anim_start = None;
            }
            if self.ai_panel_anim_opening {
                eased
            } else {
                1.0 - eased
            }
        } else if ai_state.active {
            1.0
        } else {
            0.0
        };
        if progress <= 0.0 {
            return None;
        }

        let size_info = self.size_info;
        let num_lines = size_info.screen_lines;
        let num_cols = size_info.columns;

        let fraction = config.ai.panel_height_fraction.clamp(0.20, 0.60);
        let target_lines = ((num_lines as f32 * fraction).round() as usize)
            .clamp(6, MAX_AI_PANEL_LINES.min(num_lines));
        let anim_lines =
            ((target_lines as f32 * progress).ceil() as usize).min(target_lines).max(1);
        let start_line = num_lines.saturating_sub(anim_lines);

        let prompt_line = start_line + anim_lines - 1;
        let separator_line = prompt_line.saturating_sub(1);
        let header_line = start_line;

        // Controls position matches draw_ai_panel.
        let controls = "⏹ ⟳ ✖";
        let ctrl_len = controls.chars().count();
        // Warp-like: keep a 3-column right margin for the control group
        let ctrl_col = num_cols.saturating_sub(ctrl_len + 3);
        let controls_col_start = ctrl_col;
        let controls_col_end = ctrl_col + ctrl_len.saturating_sub(1);

        Some(AiPanelGeometry {
            start_line,
            anim_lines,
            header_line,
            separator_line,
            prompt_line,
            controls_col_start,
            controls_col_end,
        })
    }

    /// Draw the AI overlay immediately (background rects then text), independent of the main draw
    /// rect pipeline.
    #[allow(dead_code)]
    pub fn draw_ai_overlay(&mut self, config: &UiConfig, ai_state: &crate::ai_runtime::AiUiState) {
        // Allow drawing during closing animation even if not active.
        let progress = if let Some(start) = self.ai_panel_anim_start {
            let elapsed = start.elapsed().as_millis() as u32;
            let dur = self.ai_panel_anim_duration_ms.max(1);
            let t = (elapsed as f32 / dur as f32).clamp(0.0, 1.0);
            let eased = 1.0 - (1.0 - t).powi(3);
            if t >= 1.0 {
                self.ai_panel_anim_start = None;
            }
            if self.ai_panel_anim_opening {
                eased
            } else {
                1.0 - eased
            }
        } else if ai_state.active {
            1.0
        } else {
            0.0
        };
        if progress <= 0.0 {
            return;
        }

        // Render both background and content via draw_ai_panel into one batch.
        let mut rects = Vec::new();
        self.draw_ai_panel(config, ai_state, &mut rects);
        if !rects.is_empty() {
            let metrics = self.glyph_cache.font_metrics();
            let size_copy = self.size_info;
            self.renderer_draw_rects(&size_copy, &metrics, rects);
        }
    }
}
