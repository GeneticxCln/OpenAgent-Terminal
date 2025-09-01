//! Tab bar rendering for OpenAgent Terminal
//!
//! This module handles the visual representation of tabs, including rendering,
//! animations, and interaction with the display system.

use std::time::Instant;

use openagent_terminal_core::index::{Column, Point};
use openagent_terminal_core::term::LineDamageBounds;
use unicode_width::UnicodeWidthStr;

use crate::config::UiConfig;
use crate::display::{Display, SizeInfo};
use crate::display::color::Rgb;
use crate::renderer::rects::RenderRect;
use crate::renderer::ui::UiRoundedRect;
use crate::display::animation::compute_progress;
use crate::workspace::{TabId, TabManager, TabBarPosition};

/// Maximum width for a tab title in cells
const MAX_TAB_WIDTH: usize = 30;

/// Minimum width for a tab in cells
const MIN_TAB_WIDTH: usize = 10;

/// Tab bar height in lines
const TAB_BAR_HEIGHT: usize = 1;

/// Tab close button character
const CLOSE_BUTTON: &str = "✖";

/// Tab modified indicator
const MODIFIED_INDICATOR: &str = "●";

/// Tab bar geometry information
#[derive(Debug, Clone, Copy)]
pub struct TabBarGeometry {
    pub start_line: usize,
    pub height: usize,
    pub tab_width: usize,
    pub visible_tabs: usize,
}

impl Display {
    /// Draw the tab bar
    pub fn draw_tab_bar(
        &mut self,
        config: &UiConfig,
        tab_manager: &TabManager,
        position: TabBarPosition,
    ) -> Option<TabBarGeometry> {
        if position == TabBarPosition::Hidden {
            return None;
        }
        
        let size_info = self.size_info;
        let num_cols = size_info.columns;
        let num_lines = size_info.screen_lines;
        
        // Calculate tab bar position
        let start_line = match position {
            TabBarPosition::Top => 0,
            TabBarPosition::Bottom => num_lines.saturating_sub(TAB_BAR_HEIGHT),
            TabBarPosition::Hidden => return None,
        };
        
        // Resolve theme
        let theme = config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;
        let tui = theme.ui;
        
        // Tab bar colors
        let bg = tokens.surface;
        let fg = tokens.text;
        let active_bg = tokens.surface_muted;
        let active_fg = tokens.accent;
        let hover_bg = tokens.surface_hover;
        let modified_color = tokens.warning;
        
        // Calculate tab dimensions
        let tab_count = tab_manager.tab_count();
        if tab_count == 0 {
            return None;
        }
        
        let available_width = num_cols;
        let max_tab_width = (available_width / tab_count).min(MAX_TAB_WIDTH).max(MIN_TAB_WIDTH);
        
        // Draw tab bar background
        let bar_y = start_line as f32 * size_info.cell_height();
        let bar_height = TAB_BAR_HEIGHT as f32 * size_info.cell_height();
        
        // Stage tab bar background with subtle shadow
        if tui.shadow {
            let shadow_alpha = tui.shadow_alpha * 0.5;
            let shadow = UiRoundedRect::new(
                0.0,
                bar_y + bar_height,
                size_info.width(),
                2.0,
                0.0,
                Rgb::new(0, 0, 0),
                shadow_alpha,
            );
            self.stage_ui_rounded_rect(shadow);
        }
        
        let bar_bg = RenderRect::new(
            0.0,
            bar_y,
            size_info.width(),
            bar_height,
            bg,
            1.0,
        );
        
        // Draw the background first
        let metrics = self.glyph_cache.font_metrics();
        self.renderer_draw_rects(&size_info, &metrics, vec![bar_bg]);
        
        // Draw tabs
        let tab_order = tab_manager.tab_order();
        let active_tab_id = tab_manager.active_tab_id();
        
        let mut current_x = 0;
        for (index, &tab_id) in tab_order.iter().enumerate() {
            if current_x >= num_cols {
                break;
            }
            
            let tab = match tab_manager.get_tab(tab_id) {
                Some(tab) => tab,
                None => continue,
            };
            
            let is_active = Some(tab_id) == active_tab_id;
            let tab_width = max_tab_width.min(num_cols - current_x);
            
            // Draw tab background
            if is_active {
                let tab_x = current_x as f32 * size_info.cell_width();
                let tab_width_px = tab_width as f32 * size_info.cell_width();
                
                // Active tab background with rounded corners
                let active_rect = if tui.rounded_corners {
                    UiRoundedRect::new(
                        tab_x,
                        bar_y,
                        tab_width_px,
                        bar_height,
                        tui.corner_radius_px * 0.5,
                        active_bg,
                        1.0,
                    )
                } else {
                    UiRoundedRect::new(
                        tab_x,
                        bar_y,
                        tab_width_px,
                        bar_height,
                        0.0,
                        active_bg,
                        1.0,
                    )
                };
                self.stage_ui_rounded_rect(active_rect);
            }
            
            // Prepare tab title
            let mut tab_text = String::new();
            
            // Add modified indicator if needed
            if tab.modified {
                tab_text.push_str(MODIFIED_INDICATOR);
                tab_text.push(' ');
            }
            
            // Add tab title (truncate if necessary)
            let title_space = tab_width.saturating_sub(
                if tab.modified { 2 } else { 0 } + 
                2 // Close button space
            );
            
            if tab.title.width() > title_space {
                let truncated: String = tab.title
                    .chars()
                    .take(title_space.saturating_sub(3))
                    .collect();
                tab_text.push_str(&truncated);
                tab_text.push_str("...");
            } else {
                tab_text.push_str(&tab.title);
            }
            
            // Draw tab text
            let text_color = if is_active { active_fg } else { fg };
            let text_point = Point::new(start_line, Column(current_x + 1));
            self.draw_tab_text(text_point, text_color, bg, &tab_text, tab_width.saturating_sub(2));
            
            // Draw close button (if enabled in config)
            // TODO: Check config for show_tab_close_button
            let close_x = current_x + tab_width.saturating_sub(2);
            if close_x > current_x {
                let close_point = Point::new(start_line, Column(close_x));
                let close_color = if is_active { active_fg } else { tokens.text_muted };
                self.draw_tab_text(close_point, close_color, bg, CLOSE_BUTTON, 1);
            }
            
            // Draw separator (except after last tab)
            if index < tab_order.len() - 1 {
                let separator_x = current_x + tab_width;
                if separator_x < num_cols {
                    let separator_point = Point::new(start_line, Column(separator_x));
                    self.draw_tab_text(separator_point, tokens.text_muted, bg, "│", 1);
                }
            }
            
            current_x += tab_width + 1; // +1 for separator
        }
        
        // Damage the tab bar area
        for line_idx in start_line..(start_line + TAB_BAR_HEIGHT) {
            if line_idx < num_lines {
                let damage = LineDamageBounds::new(line_idx, 0, num_cols);
                self.damage_tracker.frame().damage_line(damage);
                self.damage_tracker.next_frame().damage_line(damage);
            }
        }
        
        Some(TabBarGeometry {
            start_line,
            height: TAB_BAR_HEIGHT,
            tab_width: max_tab_width,
            visible_tabs: tab_order.len(),
        })
    }
    
    /// Helper to draw text in the tab bar
    fn draw_tab_text(
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
    
    /// Handle mouse click on tab bar
    pub fn handle_tab_bar_click(
        &self,
        tab_manager: &mut TabManager,
        position: TabBarPosition,
        mouse_x: usize,
        mouse_y: usize,
    ) -> Option<TabBarAction> {
        let size_info = self.size_info;
        let num_lines = size_info.screen_lines;
        
        // Check if click is in tab bar area
        let tab_bar_line = match position {
            TabBarPosition::Top => 0,
            TabBarPosition::Bottom => num_lines.saturating_sub(TAB_BAR_HEIGHT),
            TabBarPosition::Hidden => return None,
        };
        
        if mouse_y != tab_bar_line {
            return None;
        }
        
        // Calculate which tab was clicked
        let tab_count = tab_manager.tab_count();
        if tab_count == 0 {
            return None;
        }
        
        let available_width = size_info.columns;
        let max_tab_width = (available_width / tab_count).min(MAX_TAB_WIDTH).max(MIN_TAB_WIDTH);
        
        let tab_order = tab_manager.tab_order();
        let mut current_x = 0;
        
        for (index, &tab_id) in tab_order.iter().enumerate() {
            let tab_width = max_tab_width.min(size_info.columns - current_x);
            
            // Check if click is within this tab
            if mouse_x >= current_x && mouse_x < current_x + tab_width {
                // Check if close button was clicked
                let close_x = current_x + tab_width.saturating_sub(2);
                if mouse_x >= close_x && mouse_x < close_x + 2 {
                    return Some(TabBarAction::CloseTab(tab_id));
                } else {
                    return Some(TabBarAction::SelectTab(tab_id));
                }
            }
            
            current_x += tab_width + 1; // +1 for separator
        }
        
        // Check if click is in empty area (create new tab)
        if current_x < size_info.columns {
            return Some(TabBarAction::CreateTab);
        }
        
        None
    }
}

/// Actions that can be performed on the tab bar
#[derive(Debug, Clone, Copy)]
pub enum TabBarAction {
    SelectTab(TabId),
    CloseTab(TabId),
    CreateTab,
}

/// Tab animation state
#[derive(Debug, Clone)]
pub struct TabAnimation {
    pub tab_id: TabId,
    pub start_time: Instant,
    pub duration_ms: u32,
    pub animation_type: TabAnimationType,
}

#[derive(Debug, Clone, Copy)]
pub enum TabAnimationType {
    Open,
    Close,
    Switch,
}
