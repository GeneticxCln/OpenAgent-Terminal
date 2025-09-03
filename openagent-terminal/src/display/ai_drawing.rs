//! Unified AI drawing module that consolidates panel and overlay rendering
//!
//! This module provides a single, consistent interface for rendering AI-related
//! UI elements, eliminating code duplication between panel and overlay modes.

use openagent_terminal_core::index::{Column, Point};
use openagent_terminal_core::term::LineDamageBounds;
use unicode_width::UnicodeWidthStr;
use tracing::{debug, trace};

use crate::config::UiConfig;
use crate::display::Display;
use crate::display::color::Rgb;
use crate::renderer::rects::RenderRect;
#[cfg(feature = "ai")]
use crate::config::ai::AiConfig;

/// Configuration for AI UI rendering
#[derive(Debug, Clone)]
pub struct AiDrawConfig {
    /// Maximum lines to show for AI panel
    pub max_panel_lines: usize,
    /// Panel label shown at the top
    pub panel_label: &'static str,
    /// Loading indicator text
    pub loading_text: &'static str,
    /// Error prefix
    pub error_prefix: &'static str,
    /// Command suggestion prefix
    pub suggestion_prefix: &'static str,
    /// Selection indicator
    pub selection_indicator: &'static str,
    /// Animation duration in milliseconds
    pub anim_duration_ms: u32,
}

impl Default for AiDrawConfig {
    fn default() -> Self {
        Self {
            max_panel_lines: 10,
            panel_label: "🤖 AI Assistant: ",
            loading_text: "⏳ Thinking...",
            error_prefix: "❌ Error: ",
            suggestion_prefix: "$ ",
            selection_indicator: "▶ ",
            anim_duration_ms: 160,
        }
    }
}

impl AiDrawConfig {
    /// Create AiDrawConfig from UiConfig, using AI settings where available
    pub fn from_ui_config(ui_config: &UiConfig) -> Self {
        #[cfg(feature = "ai")]
        {
            let ai_config = &ui_config.ai;
            Self {
                max_panel_lines: (ai_config.propose_max_commands as usize).clamp(5, 20),
                panel_label: "🤖 AI Assistant: ",
                loading_text: "⏳ Thinking...",
                error_prefix: "❌ Error: ",
                suggestion_prefix: "$ ",
                selection_indicator: "▶ ",
                anim_duration_ms: if ui_config.theme.resolve().ui.reduce_motion { 0 } else { 160 },
            }
        }
        #[cfg(not(feature = "ai"))]
        {
            let _ = ui_config; // Silence unused warning
            Self::default()
        }
    }
}

/// Render mode for AI UI
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AiRenderMode {
    /// Render as a panel (bottom sheet)
    Panel,
    /// Render as an overlay (floating)
    Overlay,
}

/// Unified AI drawing context
pub struct AiDrawContext<'a> {
    pub display: &'a mut Display,
    pub config: &'a UiConfig,
    pub draw_config: AiDrawConfig,
    pub mode: AiRenderMode,
}

impl<'a> AiDrawContext<'a> {
    /// Create a new AI drawing context
    pub fn new(
        display: &'a mut Display,
        config: &'a UiConfig,
        mode: AiRenderMode,
    ) -> Self {
        Self {
            display,
            config,
            draw_config: AiDrawConfig::from_ui_config(config),
            mode,
        }
    }

    /// Draw the AI UI with the specified state
    #[cfg(feature = "ai")]
    pub fn draw(&mut self, ai_state: &crate::ai_runtime::AiUiState) -> Vec<RenderRect> {
        let mut rects = Vec::new();
        
        // Calculate animation progress
        let progress = self.calculate_animation_progress(ai_state.active);
        
        // Skip if fully hidden
        if progress <= 0.0 && !ai_state.active {
            return rects;
        }
        
        // Draw backdrop
        self.draw_backdrop(progress, &mut rects);
        
        // Calculate panel dimensions
        let (start_line, anim_lines) = self.calculate_panel_dimensions(progress);
        
        // Draw panel background
        self.draw_panel_background(start_line, anim_lines, progress, &mut rects);
        
        // Draw panel content
        self.draw_panel_content(ai_state, start_line, anim_lines, &mut rects);
        
        // Apply damage tracking
        self.apply_damage(start_line, anim_lines);
        
        rects
    }
    
    /// Calculate animation progress
    fn calculate_animation_progress(&mut self, active: bool) -> f32 {
        // Check for state change
        if active != self.display.ai_panel_last_active {
            self.display.ai_panel_last_active = active;
            self.display.ai_panel_anim_start = Some(std::time::Instant::now());
            self.display.ai_panel_anim_opening = active;
            self.display.ai_panel_anim_duration_ms = if active {
                self.draw_config.anim_duration_ms
            } else {
                (self.draw_config.anim_duration_ms * 7) / 8 // Slightly faster close
            };
        }
        
        // Calculate eased progress
        let progress = if let Some(start) = self.display.ai_panel_anim_start {
            let elapsed = start.elapsed().as_millis() as u32;
            let dur = self.display.ai_panel_anim_duration_ms.max(1);
            let t = (elapsed as f32 / dur as f32).clamp(0.0, 1.0);
            
            // Use ease-out cubic for smooth animation
            let eased = 1.0 - (1.0 - t).powi(3);
            
            if t >= 1.0 {
                self.display.ai_panel_anim_start = None;
            }
            
            eased
        } else {
            if active { 1.0 } else { 0.0 }
        };
        
        // Invert progress for closing animation
        if !self.display.ai_panel_anim_opening {
            1.0 - progress
        } else {
            progress
        }
    }
    
    /// Draw the backdrop dim
    fn draw_backdrop(&self, progress: f32, rects: &mut Vec<RenderRect>) {
        #[cfg(feature = "ai")]
        let backdrop_alpha = (self.config.ai.backdrop_alpha * progress).clamp(0.0, 1.0);
        #[cfg(not(feature = "ai"))]
        let backdrop_alpha = 0.0;
        
        if backdrop_alpha > 0.0 {
            let size_info = self.display.size_info;
            let dim = Rgb::new(0, 0, 0);
            let backdrop = RenderRect::new(
                0.0,
                0.0,
                size_info.width(),
                size_info.height(),
                dim,
                backdrop_alpha,
            );
            rects.push(backdrop);
            trace!("Drew backdrop with alpha {}", backdrop_alpha);
        }
    }
    
    /// Calculate panel dimensions based on animation progress
    fn calculate_panel_dimensions(&self, progress: f32) -> (usize, usize) {
        let size_info = self.display.size_info;
        let num_lines = size_info.screen_lines;
        
        #[cfg(feature = "ai")]
        let fraction = self.config.ai.panel_height_fraction.clamp(0.20, 0.60);
        #[cfg(not(feature = "ai"))]
        let fraction = 0.40;
        
        let target_lines = ((num_lines as f32 * fraction).round() as usize)
            .clamp(6, self.draw_config.max_panel_lines.min(num_lines));
        
        let anim_lines = ((target_lines as f32 * progress).ceil() as usize)
            .min(target_lines)
            .max(1);
        
        let start_line = num_lines.saturating_sub(anim_lines);
        
        (start_line, anim_lines)
    }
    
    /// Draw the panel background
    fn draw_panel_background(
        &self,
        start_line: usize,
        anim_lines: usize,
        progress: f32,
        rects: &mut Vec<RenderRect>,
    ) {
        let size_info = self.display.size_info;
        let bg = self.config.colors.footer_bar_background();
        
        let panel_y = start_line as f32 * size_info.cell_height();
        let panel_height = anim_lines as f32 * size_info.cell_height();
        
        let panel_alpha = match self.mode {
            AiRenderMode::Panel => 0.95 * progress,
            AiRenderMode::Overlay => 0.85 * progress,
        };
        
        let panel_rect = RenderRect::new(
            0.0,
            panel_y,
            size_info.width(),
            panel_height,
            bg,
            panel_alpha,
        );
        rects.push(panel_rect);
        
        debug!("Drew panel background: lines {} to {}", start_line, start_line + anim_lines);
    }
    
    /// Draw the panel content
    fn draw_panel_content(
        &mut self,
        ai_state: &crate::ai_runtime::AiUiState,
        start_line: usize,
        anim_lines: usize,
        _rects: &mut Vec<RenderRect>,
    ) {
        let size_info = self.display.size_info;
        let num_cols = size_info.columns;
        let bg = self.config.colors.footer_bar_background();
        let fg = self.config.colors.footer_bar_foreground();
        
        let mut current_line = start_line;
        
        // Reserve bottom for input (Warp-like)
        let prompt_line = start_line + anim_lines - 1;
        let separator_line = prompt_line.saturating_sub(1);
        
        // Draw header
        if current_line <= separator_line {
            self.draw_header(current_line, fg, bg, num_cols);
            current_line += 1;
        }
        
        // Draw action hints
        if current_line <= separator_line {
            self.draw_action_hints(current_line, bg, num_cols);
            current_line += 1;
        }
        
        // Draw main content based on state
        current_line = self.draw_main_content(
            ai_state,
            current_line,
            separator_line,
            fg,
            bg,
            num_cols,
        );
        
        // Draw separator
        if separator_line >= start_line {
            self.draw_separator(separator_line, fg, bg, num_cols);
        }
        
        // Draw prompt line
        self.draw_prompt_line(ai_state, prompt_line, fg, bg, num_cols);
    }
    
    /// Draw the header
    fn draw_header(&mut self, line: usize, fg: Rgb, bg: Rgb, num_cols: usize) {
        let header_text = "AI Assistant";
        let header_point = Point::new(line, Column(2));
        self.display.draw_ai_text(header_point, fg, bg, header_text, num_cols - 2);
    }
    
    /// Draw action hints
    fn draw_action_hints(&mut self, line: usize, bg: Rgb, num_cols: usize) {
        let actions = match self.mode {
            AiRenderMode::Panel => {
                "[Ctrl+I] Insert  [Ctrl+E] Apply  [Ctrl+C] Copy  [Ctrl+R] Retry  [Esc] Close"
            }
            AiRenderMode::Overlay => {
                "[Tab] Accept  [Ctrl+C] Copy  [Esc] Dismiss"
            }
        };
        
        let hint_color = Rgb::new(180, 200, 220);
        let actions_point = Point::new(line, Column(2));
        self.display.draw_ai_text(actions_point, hint_color, bg, actions, num_cols - 2);
    }
    
    /// Draw main content area
    fn draw_main_content(
        &mut self,
        ai_state: &crate::ai_runtime::AiUiState,
        mut current_line: usize,
        separator_line: usize,
        fg: Rgb,
        bg: Rgb,
        num_cols: usize,
    ) -> usize {
        if ai_state.is_loading {
            // Loading state
            self.draw_loading_state(ai_state, &mut current_line, separator_line, fg, bg, num_cols);
        } else if let Some(ref error) = ai_state.error_message {
            // Error state
            self.draw_error_state(error, current_line, fg, bg, num_cols);
            current_line += 1;
        } else if !ai_state.streaming_text.is_empty() {
            // Streaming text
            self.draw_streaming_text(&ai_state.streaming_text, &mut current_line, separator_line, fg, bg, num_cols);
        } else if !ai_state.proposals.is_empty() {
            // Proposals
            self.draw_proposals(ai_state, &mut current_line, separator_line, fg, bg, num_cols);
        }
        
        current_line
    }
    
    /// Draw loading state
    fn draw_loading_state(
        &mut self,
        ai_state: &crate::ai_runtime::AiUiState,
        current_line: &mut usize,
        separator_line: usize,
        fg: Rgb,
        bg: Rgb,
        num_cols: usize,
    ) {
        if *current_line <= separator_line {
            let loading_point = Point::new(*current_line, Column(2));
            self.display.draw_ai_text(
                loading_point,
                fg,
                bg,
                self.draw_config.loading_text,
                num_cols - 2,
            );
            *current_line += 1;
        }
        
        // Show partial streaming text if available
        if !ai_state.streaming_text.is_empty() {
            self.draw_streaming_text(
                &ai_state.streaming_text,
                current_line,
                separator_line,
                fg,
                bg,
                num_cols,
            );
        }
    }
    
    /// Draw error state
    fn draw_error_state(
        &mut self,
        error: &str,
        line: usize,
        _fg: Rgb,
        bg: Rgb,
        num_cols: usize,
    ) {
        let error_text = format!("{}{}", self.draw_config.error_prefix, error);
        let error_point = Point::new(line, Column(2));
        let error_color = Rgb::new(255, 100, 100);
        self.display.draw_ai_text(error_point, error_color, bg, &error_text, num_cols - 2);
    }
    
    /// Draw streaming text
    fn draw_streaming_text(
        &mut self,
        text: &str,
        current_line: &mut usize,
        separator_line: usize,
        fg: Rgb,
        bg: Rgb,
        num_cols: usize,
    ) {
        for line in text.lines() {
            if *current_line > separator_line {
                break;
            }
            let text_point = Point::new(*current_line, Column(2));
            self.display.draw_ai_text(text_point, fg, bg, line, num_cols - 2);
            *current_line += 1;
        }
    }
    
    /// Draw proposals
    fn draw_proposals(
        &mut self,
        ai_state: &crate::ai_runtime::AiUiState,
        current_line: &mut usize,
        separator_line: usize,
        fg: Rgb,
        bg: Rgb,
        num_cols: usize,
    ) {
        for (idx, proposal) in ai_state.proposals.iter().enumerate() {
            if *current_line > separator_line {
                break;
            }
            
            // Build line with selection indicator
            let mut line_text = String::new();
            if idx == ai_state.selected_proposal {
                line_text.push_str(self.draw_config.selection_indicator);
            } else {
                line_text.push_str("  ");
            }
            
            // Add first command
            if let Some(first_cmd) = proposal.proposed_commands.first() {
                line_text.push_str(self.draw_config.suggestion_prefix);
                
                // Truncate if needed
                let available_width = num_cols.saturating_sub(line_text.width());
                if first_cmd.width() > available_width {
                    let truncated: String = first_cmd
                        .chars()
                        .take(available_width.saturating_sub(3))
                        .collect();
                    line_text.push_str(&truncated);
                    line_text.push_str("...");
                } else {
                    line_text.push_str(first_cmd);
                }
                
                let text_color = if idx == ai_state.selected_proposal {
                    Rgb::new(100, 255, 100) // Highlight selected
                } else {
                    fg
                };
                
                let text_point = Point::new(*current_line, Column(0));
                self.display.draw_ai_text(text_point, text_color, bg, &line_text, num_cols);
                *current_line += 1;
            }
        }
    }
    
    /// Draw separator line
    fn draw_separator(&mut self, line: usize, fg: Rgb, bg: Rgb, num_cols: usize) {
        let separator = "─".repeat(num_cols);
        let separator_point = Point::new(line, Column(0));
        self.display.draw_ai_text(separator_point, fg, bg, &separator, num_cols);
    }
    
    /// Draw prompt line
    fn draw_prompt_line(
        &mut self,
        ai_state: &crate::ai_runtime::AiUiState,
        line: usize,
        fg: Rgb,
        bg: Rgb,
        num_cols: usize,
    ) {
        let prefix = "🤖 ";
        let mut prompt = String::with_capacity(prefix.len() + ai_state.scratch.len());
        prompt.push_str(prefix);
        prompt.push_str(&ai_state.scratch);
        
        let prompt_point = Point::new(line, Column(0));
        self.display.draw_ai_text(prompt_point, fg, bg, &prompt, num_cols);
        
        // Draw cursor
        let cursor_col = prefix.width() + ai_state.cursor_position;
        let cursor_col = cursor_col.min(num_cols.saturating_sub(1));
        let cursor_point = Point::new(line, Column(cursor_col));
        self.display.draw_ai_text(cursor_point, bg, fg, " ", 1);
    }
    
    /// Apply damage tracking for the affected area
    fn apply_damage(&mut self, start_line: usize, anim_lines: usize) {
        let size_info = self.display.size_info;
        let num_cols = size_info.columns;
        let end_line = start_line + anim_lines;
        
        for line_idx in start_line..end_line {
            let damage = LineDamageBounds::new(line_idx, 0, num_cols);
            self.display.damage_tracker.frame().damage_line(damage);
            self.display.damage_tracker.next_frame().damage_line(damage);
        }
    }
}

#[cfg(feature = "ai")]
impl Display {
    /// Draw AI UI using the unified drawing system
    pub fn draw_ai_unified(
        &mut self,
        config: &UiConfig,
        ai_state: &crate::ai_runtime::AiUiState,
        mode: AiRenderMode,
    ) -> Vec<RenderRect> {
        let mut ctx = AiDrawContext::new(self, config, mode);
        ctx.draw(ai_state)
    }
}
