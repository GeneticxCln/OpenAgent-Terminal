//! Tab bar rendering for OpenAgent Terminal
//!
//! This module handles the visual representation of tabs, including rendering,
//! animations, and interaction with the display system.

#![allow(dead_code)]

use std::time::Instant;

use openagent_terminal_core::index::{Column, Point};
use openagent_terminal_core::term::LineDamageBounds;
use unicode_width::UnicodeWidthStr;
use winit::event::MouseButton;

use crate::config::UiConfig;
use crate::display::color::Rgb;
use crate::display::Display;
use crate::renderer::rects::RenderRect;
use crate::renderer::ui::UiRoundedRect;
use crate::workspace::{TabBarPosition, TabId, TabManager};

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
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;
        let tui = theme.ui;

        // Tab bar colors
        let bg = tokens.surface;
        let fg = tokens.text;
        let active_bg = tokens.surface_muted;
        let active_fg = tokens.accent;
        let _hover_bg = tokens.surface_muted;
        let _modified_color = tokens.warning;

        // Calculate tab dimensions
        let tab_count = tab_manager.tab_count();
        if tab_count == 0 {
            return None;
        }

        let available_width = num_cols;
        let max_tab_width = (available_width / tab_count).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);

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

        let bar_bg = RenderRect::new(0.0, bar_y, size_info.width(), bar_height, bg, 1.0);

        // Draw the background first
        let metrics = self.glyph_cache.font_metrics();
        self.renderer_draw_rects(&size_info, &metrics, vec![bar_bg]);

        // Draw tabs with animation and drag-and-drop support
        let tab_order = tab_manager.tab_order();
        let active_tab_id = tab_manager.active_tab_id();
        let hover = self.tab_hover;
        let now = std::time::Instant::now();
        
        // Check for active drag state
        let drag_state = self.tab_drag_active.as_ref();
        
        // Calculate animation states
        let mut tab_positions = Vec::new();
        let mut current_x = 0;
        
        for (index, &tab_id) in tab_order.iter().enumerate() {
            if current_x >= num_cols {
                break;
            }
            
            let tab_width = max_tab_width.min(num_cols - current_x);
            let mut visual_x = current_x as f32;
            let mut alpha = 1.0;
            let mut scale = 1.0;
            
            // Apply drag animation effects
            if let Some(drag) = drag_state {
                if drag.tab_id == tab_id && drag.is_active {
                    // Tab being dragged - apply visual offset
                    visual_x += drag.visual_offset_x;
                    alpha = 0.8; // Slightly transparent
                    scale = 1.02; // Slightly larger
                    
                    // Add subtle shadow effect for dragged tab
                } else if drag.is_active {
                    // Other tabs - slight animation based on position
                    let distance_from_drag = (index as i32 - drag.original_position as i32).abs();
                    let influence = (5.0 - distance_from_drag as f32 * 0.5).max(0.0) / 5.0;
                    
                    // Subtle squeeze effect for nearby tabs
                    scale = 1.0 - influence * 0.02;
                    alpha = 1.0 - influence * 0.1;
                }
            }
            
            // Store position for rendering
            tab_positions.push((tab_id, visual_x, tab_width, alpha, scale));
            current_x += tab_width + 1; // +1 for separator
        }
        
        // Render tabs with calculated positions and effects
        for (tab_id, visual_x, tab_width, alpha, scale) in tab_positions {
            let tab = match tab_manager.get_tab(tab_id) {
                Some(tab) => tab,
                None => continue,
            };
            
            let is_active = Some(tab_id) == active_tab_id;
            let current_x = visual_x as usize;

            // Determine hover state for this tab
            let is_hover_tab = matches!(hover, Some(crate::display::TabHoverTarget::Tab(id)) if id == tab_id)
                || matches!(hover, Some(crate::display::TabHoverTarget::Close(id)) if id == tab_id);
            let is_hover_close =
                matches!(hover, Some(crate::display::TabHoverTarget::Close(id)) if id == tab_id);

            // Draw tab background with animation effects
            if is_active || is_hover_tab {
                let tab_x = visual_x * size_info.cell_width();
                let tab_width_px = tab_width as f32 * size_info.cell_width() * scale;
                let tab_height_px = bar_height * scale;
                
                // Adjust position for scaling (keep centered)
                let scaled_tab_x = tab_x - (tab_width_px - tab_width as f32 * size_info.cell_width()) / 2.0;
                let scaled_tab_y = bar_y - (tab_height_px - bar_height) / 2.0;

                // Active tab background with rounded corners and animation
                let bg_color = if is_active { active_bg } else { tokens.surface_muted };
                let base_alpha = if is_active { 1.0 } else { 0.85 };
                let final_alpha = base_alpha * alpha;
                
                let active_rect = if tui.rounded_corners {
                    UiRoundedRect::new(
                        scaled_tab_x,
                        scaled_tab_y,
                        tab_width_px,
                        tab_height_px,
                        tui.corner_radius_px * 0.5 * scale,
                        bg_color,
                        final_alpha,
                    )
                } else {
                    UiRoundedRect::new(
                        scaled_tab_x, 
                        scaled_tab_y, 
                        tab_width_px, 
                        tab_height_px, 
                        0.0, 
                        bg_color, 
                        final_alpha
                    )
                };
                self.stage_ui_rounded_rect(active_rect);
                
                // Add subtle shadow for dragged tabs
                if let Some(drag) = drag_state {
                    if drag.tab_id == tab_id && drag.is_active {
                        let shadow_offset = 3.0;
                        let shadow_rect = UiRoundedRect::new(
                            scaled_tab_x + shadow_offset,
                            scaled_tab_y + shadow_offset,
                            tab_width_px,
                            tab_height_px,
                            tui.corner_radius_px * 0.5 * scale,
                            Rgb::new(0, 0, 0),
                            0.2 * alpha,
                        );
                        self.stage_ui_rounded_rect(shadow_rect);
                    }
                }
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
                if tab.modified { 2 } else { 0 } + 2, // Close button space
            );

            if tab.title.width() > title_space {
                let truncated: String =
                    tab.title.chars().take(title_space.saturating_sub(3)).collect();
                tab_text.push_str(&truncated);
                tab_text.push_str("...");
            } else {
                tab_text.push_str(&tab.title);
            }

            // Draw tab text
            let text_color = if is_active {
                active_fg
            } else if is_hover_tab {
                tokens.accent
            } else {
                fg
            };
            let text_point = Point::new(start_line, Column(current_x + 1));
            self.draw_tab_text(text_point, text_color, bg, &tab_text, tab_width.saturating_sub(2));

            // Draw close button (if enabled in config)
            // TODO: Check config for show_tab_close_button
            let close_x = current_x + tab_width.saturating_sub(2);
            if close_x > current_x {
                let close_point = Point::new(start_line, Column(close_x));
                let close_color = if is_hover_close {
                    tokens.accent
                } else if is_active {
                    active_fg
                } else {
                    tokens.text_muted
                };
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

        // Draw new tab button ("[+]") in the remaining area
        if current_x < num_cols {
            let plus_label = "[+]";
            let plus_point = Point::new(start_line, Column(current_x + 1));
            let hovered_create = matches!(hover, Some(crate::display::TabHoverTarget::Create));
            let plus_color = if hovered_create { tokens.accent } else { tokens.text_muted };
            self.draw_tab_text(plus_point, plus_color, bg, plus_label, plus_label.len());
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
        tab_manager: &TabManager,
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
        let max_tab_width = (available_width / tab_count).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);

        let tab_order = tab_manager.tab_order();
        let mut current_x = 0;

        for &tab_id in tab_order.iter() {
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

    /// Handle mouse press (potential drag start) on tab bar
    pub fn handle_tab_bar_mouse_press(
        &mut self,
        tab_manager: &TabManager,
        position: TabBarPosition,
        mouse_x: usize,
        mouse_y: usize,
        button: MouseButton,
    ) -> Option<TabBarAction> {
        // Only handle left mouse button for dragging
        if button != MouseButton::Left {
            return self.handle_tab_bar_click(tab_manager, position, mouse_x, mouse_y);
        }

        let size_info = self.size_info;
        let num_lines = size_info.screen_lines;

        // Check if press is in tab bar area
        let tab_bar_line = match position {
            TabBarPosition::Top => 0,
            TabBarPosition::Bottom => num_lines.saturating_sub(TAB_BAR_HEIGHT),
            TabBarPosition::Hidden => return None,
        };

        if mouse_y != tab_bar_line {
            return None;
        }

        // Find which tab was pressed
        if let Some(tab_id) = self.get_tab_at_position(tab_manager, mouse_x) {
            // Check if close button was clicked
            if self.is_close_button_at_position(tab_manager, tab_id, mouse_x) {
                return Some(TabBarAction::CloseTab(tab_id));
            } else {
                // Initialize potential drag operation
                self.tab_drag_active = Some(super::TabDragState {
                    tab_id,
                    original_position: self.get_tab_position(tab_manager, tab_id),
                    current_position: self.get_tab_position(tab_manager, tab_id),
                    target_position: None,
                    start_mouse_x: mouse_x,
                    start_mouse_y: mouse_y,
                    current_mouse_x: mouse_x,
                    current_mouse_y: mouse_y,
                    visual_offset_x: 0.0,
                    visual_offset_y: 0.0,
                    is_active: false, // Not active until threshold exceeded
                    drag_threshold: 10.0, // pixels
                });
                return Some(TabBarAction::BeginDrag(tab_id));
            }
        }

        // Check if click is in empty area (create new tab)
        let tab_count = tab_manager.tab_count();
        if tab_count > 0 {
            let available_width = size_info.columns;
            let max_tab_width = (available_width / tab_count).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);
            let tab_area_width = tab_count * (max_tab_width + 1); // +1 for separator
            
            if mouse_x >= tab_area_width {
                return Some(TabBarAction::CreateTab);
            }
        }

        None
    }

    /// Handle mouse move (potential drag) on tab bar
    pub fn handle_tab_bar_mouse_move(
        &mut self,
        tab_manager: &TabManager,
        mouse_x: usize,
        mouse_y: usize,
    ) -> Option<TabBarAction> {
        if let Some(ref mut drag_state) = self.tab_drag_active {
            drag_state.current_mouse_x = mouse_x;
            drag_state.current_mouse_y = mouse_y;

            let distance_x = (mouse_x as i32 - drag_state.start_mouse_x as i32).abs() as f32;
            let distance_y = (mouse_y as i32 - drag_state.start_mouse_y as i32).abs() as f32;
            let total_distance = (distance_x.powi(2) + distance_y.powi(2)).sqrt();

            // Check if we've exceeded the drag threshold
            if !drag_state.is_active && total_distance > drag_state.drag_threshold {
                drag_state.is_active = true;
                self.tab_drag_anim_start = Some(std::time::Instant::now());
                return Some(TabBarAction::DragMove(drag_state.tab_id, drag_state.current_position));
            }

            // If drag is active, calculate new position
            if drag_state.is_active {
                let new_position = self.calculate_drop_position(tab_manager, mouse_x);
                if new_position != drag_state.current_position {
                    drag_state.target_position = Some(new_position);
                    return Some(TabBarAction::DragMove(drag_state.tab_id, new_position));
                }
            }
        }

        None
    }

    /// Handle mouse release (end drag) on tab bar
    pub fn handle_tab_bar_mouse_release(
        &mut self,
        button: MouseButton,
    ) -> Option<TabBarAction> {
        if button != MouseButton::Left {
            return None;
        }

        if let Some(drag_state) = self.tab_drag_active.take() {
            if drag_state.is_active {
                // Complete the drag operation
                if let Some(target_pos) = drag_state.target_position {
                    if target_pos != drag_state.original_position {
                        return Some(TabBarAction::DragMove(drag_state.tab_id, target_pos));
                    }
                }
                return Some(TabBarAction::EndDrag(drag_state.tab_id));
            } else {
                // This was just a click, not a drag
                return Some(TabBarAction::SelectTab(drag_state.tab_id));
            }
        }

        None
    }

    /// Get tab ID at given mouse position
    fn get_tab_at_position(&self, tab_manager: &TabManager, mouse_x: usize) -> Option<crate::workspace::TabId> {
        let size_info = self.size_info;
        let tab_count = tab_manager.tab_count();
        
        if tab_count == 0 {
            return None;
        }

        let available_width = size_info.columns;
        let max_tab_width = (available_width / tab_count).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);
        let tab_order = tab_manager.tab_order();
        let mut current_x = 0;

        for &tab_id in tab_order.iter() {
            let tab_width = max_tab_width.min(size_info.columns - current_x);
            
            if mouse_x >= current_x && mouse_x < current_x + tab_width {
                return Some(tab_id);
            }
            
            current_x += tab_width + 1; // +1 for separator
        }
        
        None
    }

    /// Check if mouse position is over close button for given tab
    fn is_close_button_at_position(
        &self, 
        tab_manager: &TabManager, 
        tab_id: crate::workspace::TabId, 
        mouse_x: usize
    ) -> bool {
        // Get tab bounds and check if close button area
        if let Some(tab_bounds) = self.get_tab_bounds(tab_manager, tab_id) {
            let close_x = tab_bounds.1.saturating_sub(2); // Close button is 2 chars from end
            mouse_x >= close_x && mouse_x < tab_bounds.1
        } else {
            false
        }
    }

    /// Get tab position in tab order
    fn get_tab_position(&self, tab_manager: &TabManager, tab_id: crate::workspace::TabId) -> usize {
        tab_manager.tab_order().iter().position(|&id| id == tab_id).unwrap_or(0)
    }

    /// Calculate drop position based on mouse X coordinate
    fn calculate_drop_position(&self, tab_manager: &TabManager, mouse_x: usize) -> usize {
        let size_info = self.size_info;
        let tab_count = tab_manager.tab_count();
        
        if tab_count == 0 {
            return 0;
        }

        let available_width = size_info.columns;
        let max_tab_width = (available_width / tab_count).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);
        
        // Calculate which position the mouse is closest to
        let position = (mouse_x + max_tab_width / 2) / (max_tab_width + 1);
        position.min(tab_count.saturating_sub(1))
    }

    /// Get tab bounds (start_x, end_x)
    fn get_tab_bounds(
        &self, 
        tab_manager: &TabManager, 
        tab_id: crate::workspace::TabId
    ) -> Option<(usize, usize)> {
        let size_info = self.size_info;
        let tab_count = tab_manager.tab_count();
        
        if tab_count == 0 {
            return None;
        }

        let available_width = size_info.columns;
        let max_tab_width = (available_width / tab_count).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);
        let tab_order = tab_manager.tab_order();
        let mut current_x = 0;

        for &id in tab_order.iter() {
            let tab_width = max_tab_width.min(size_info.columns - current_x);
            
            if id == tab_id {
                return Some((current_x, current_x + tab_width));
            }
            
            current_x += tab_width + 1;
        }
        
        None
    }
}

/// Actions that can be performed on the tab bar
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy)]
pub enum TabBarAction {
    SelectTab(TabId),
    CloseTab(TabId),
    CreateTab,
    // Drag and drop operations
    BeginDrag(TabId),
    DragMove(TabId, usize), // tab_id, new_position
    EndDrag(TabId),
    CancelDrag(TabId),
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
    // Drag and drop animations
    DragStart,
    DragMove,
    DragEnd,
    // Visual feedback
    Hover,
    Focus,
    // Insertion/deletion
    SlideIn,
    SlideOut,
}
