//! Warp-style UI enhancements for OpenAgent Terminal
//!
//! This module implements visual design elements that match Warp Terminal:
//! - Modern tab bar styling with rounded corners and gradients
//! - Split pane indicators and resize handles
//! - Smooth animations for tab/pane operations
//! - Hover states and visual feedback

#![allow(dead_code)]

use std::time::Instant;

use openagent_terminal_core::index::{Column, Point};

use crate::config::UiConfig;
use crate::display::color::Rgb;
use crate::display::Display;
use crate::renderer::rects::RenderRect;
use crate::renderer::ui::UiRoundedRect;
use crate::workspace::{TabBarPosition, TabManager};

/// Enhanced tab styling for Warp-like appearance
#[derive(Debug, Clone, PartialEq)]
pub struct WarpTabStyle {
    /// Tab height in pixels
    pub tab_height: f32,

    /// Corner radius for tabs
    pub corner_radius: f32,

    /// Tab padding (horizontal)
    pub tab_padding: f32,

    /// Active tab background color
    pub active_bg: Rgb,

    /// Inactive tab background color  
    pub inactive_bg: Rgb,

    /// Hover tab background color
    pub hover_bg: Rgb,

    /// Active tab text color
    pub active_fg: Rgb,

    /// Inactive tab text color
    pub inactive_fg: Rgb,

    /// Tab separator color
    pub separator_color: Rgb,

    /// Drop shadow enabled
    pub drop_shadow: bool,

    /// Animation duration for tab state changes
    pub animation_duration_ms: u32,
}

/// Linear interpolation between two RGB colors
fn lerp_rgb(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let r = (a.r as f32 + (b.r as f32 - a.r as f32) * t).round().clamp(0.0, 255.0) as u8;
    let g = (a.g as f32 + (b.g as f32 - a.g as f32) * t).round().clamp(0.0, 255.0) as u8;
    let bb = (a.b as f32 + (b.b as f32 - a.b as f32) * t).round().clamp(0.0, 255.0) as u8;
    Rgb::new(r, g, bb)
}

impl Default for WarpTabStyle {
    fn default() -> Self {
        // Fallback defaults (theme-aware builder provided by from_theme)
        Self {
            tab_height: 36.0,
            corner_radius: 8.0,
            tab_padding: 12.0,
            active_bg: Rgb::new(30, 30, 30),
            inactive_bg: Rgb::new(24, 24, 24),
            hover_bg: Rgb::new(40, 40, 40),
            active_fg: Rgb::new(230, 230, 230),
            inactive_fg: Rgb::new(160, 160, 160),
            separator_color: Rgb::new(60, 60, 60),
            drop_shadow: true,
            animation_duration_ms: 180,
        }
    }
}

impl WarpTabStyle {
    /// Build from current theme tokens and ThemeUi parameters
    pub fn from_theme(config: &UiConfig) -> Self {
        let theme = config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;
        let ui = theme.ui;
        Self {
            tab_height: 36.0,
            corner_radius: if ui.rounded_corners { ui.corner_radius_px } else { 0.0 },
            tab_padding: 12.0,
            active_bg: tokens.surface,
            inactive_bg: tokens.surface_muted,
            hover_bg: tokens.surface_muted,
            active_fg: tokens.accent,
            inactive_fg: tokens.text,
            separator_color: tokens.border,
            drop_shadow: ui.shadow,
            animation_duration_ms: if ui.reduce_motion { 0 } else { 180 },
        }
    }
}

/// Split pane visual indicators
#[derive(Debug, Clone, PartialEq)]
pub struct WarpSplitIndicators {
    /// Show split preview when hovering
    pub show_split_preview: bool,

    /// Split line width in pixels
    pub split_line_width: f32,

    /// Base line alpha
    pub split_line_alpha: f32,

    /// Split line color
    pub split_line_color: Rgb,

    /// Split handle size for resizing
    pub split_handle_size: f32,

    /// Split handle alpha
    pub split_handle_alpha: f32,

    /// Split handle color (when visible)
    pub split_handle_color: Rgb,

    /// Show resize handles on hover
    pub show_resize_handles: bool,

    /// Hover line width scale
    pub hover_line_scale: f32,

    /// Hover line alpha
    pub hover_line_alpha: f32,

    /// Zoom overlay transparency
    pub zoom_overlay_alpha: f32,

    /// Zoom overlay color
    pub zoom_overlay_color: Rgb,
}

impl Default for WarpSplitIndicators {
    fn default() -> Self {
        Self {
            show_split_preview: true,
            split_line_width: 2.0,
            split_line_alpha: 0.6,
            split_line_color: Rgb::new(200, 200, 200),
            split_handle_size: 6.0,
            split_handle_alpha: 0.9,
            split_handle_color: Rgb::new(100, 150, 250),
            show_resize_handles: true,
            hover_line_scale: 1.75,
            hover_line_alpha: 0.9,
            zoom_overlay_alpha: 0.1,
            zoom_overlay_color: Rgb::new(100, 150, 250),
        }
    }
}

/// Animation state for smooth transitions
#[derive(Debug, Clone)]
pub struct WarpAnimation {
    pub start_time: Instant,
    pub duration_ms: u32,
    pub animation_type: WarpAnimationType,
    pub easing: WarpEasing,
}

#[derive(Debug, Clone, Copy)]
pub enum WarpAnimationType {
    TabOpen,
    TabClose,
    TabSwitch,
    PaneSplit,
    PaneClose,
    PaneZoom,
    PaneUnzoom,
}

#[derive(Debug, Clone, Copy)]
pub enum WarpEasing {
    Linear,
    EaseInOut,
    EaseOut,
    Spring,
}

impl Display {
    /// Build Warp split indicators from config and theme
    pub fn warp_split_indicators_from_config(&self, config: &UiConfig) -> WarpSplitIndicators {
        let theme = config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;
        let s = &config.workspace.splits;
        let line_color = s.indicator_line_color.unwrap_or(tokens.border);
        let handle_color = s.handle_color.unwrap_or(tokens.accent);
        let overlay_color = s.overlay_color.unwrap_or(tokens.overlay);
        WarpSplitIndicators {
            show_split_preview: s.preview_enabled,
            split_line_width: s.indicator_line_width,
            split_line_alpha: s.indicator_line_alpha,
            split_line_color: line_color,
            split_handle_size: s.handle_size,
            split_handle_alpha: s.handle_alpha,
            split_handle_color: handle_color,
            show_resize_handles: s.show_resize_handles,
            hover_line_scale: s.indicator_hover_scale,
            hover_line_alpha: s.indicator_hover_alpha,
            zoom_overlay_alpha: s.zoom_overlay_alpha,
            zoom_overlay_color: overlay_color,
        }
    }

    /// Draw Warp-style tab bar with enhanced styling
    pub fn draw_warp_tab_bar(
        &mut self,
        config: &UiConfig,
        tab_manager: &TabManager,
        position: TabBarPosition,
        style: &WarpTabStyle,
    ) -> Option<crate::display::tab_bar::TabBarGeometry> {
        if position == TabBarPosition::Hidden {
            return None;
        }

        let size_info = self.size_info;
        let tab_count = tab_manager.tab_count();

        if tab_count == 0 {
            return None;
        }

        // Calculate tab bar position
        let start_y = match position {
            TabBarPosition::Top => 0.0,
            TabBarPosition::Bottom => size_info.height() - style.tab_height,
            TabBarPosition::Hidden => return None,
        };

        // Draw background with gradient
        self.draw_warp_tab_background(start_y, style.tab_height, style);

        // Draw tabs with Warp styling
        let tab_order = tab_manager.tab_order();
        let active_tab_id = tab_manager.active_tab_id();
        // Track active tab changes for switch animation
        if self.tab_last_active_id != active_tab_id {
            self.tab_last_active_id = active_tab_id;
            self.tab_anim_switch_start = if style.animation_duration_ms == 0 || config.theme.reduce_motion {
                None
            } else {
                Some(Instant::now())
            };
        }

        let available_width = size_info.width() - style.tab_padding * 2.0;
        let tab_width = (available_width / tab_count as f32).min(200.0).max(120.0);

        let mut current_x = style.tab_padding;

        for &tab_id in tab_order.iter() {
            if current_x + tab_width > size_info.width() {
                break;
            }

            let tab = match tab_manager.get_tab(tab_id) {
                Some(tab) => tab,
                None => continue,
            };

            let is_active = Some(tab_id) == active_tab_id;
            self.draw_warp_tab(current_x, start_y, tab_width, tab, is_active, style);

            current_x += tab_width + 8.0; // 8px gap between tabs
        }

        // Draw "+" button for new tab (hover-aware)
        let create_hover = matches!(self.tab_hover, Some(crate::display::TabHoverTarget::Create));
        self.draw_new_tab_button(current_x, start_y, style, create_hover);

        Some(crate::display::tab_bar::TabBarGeometry {
            start_line: (start_y / size_info.cell_height()) as usize,
            height: (style.tab_height / size_info.cell_height()) as usize,
            tab_width: (tab_width / size_info.cell_width()) as usize,
            visible_tabs: tab_count,
        })
    }

    /// Draw Warp-style tab background
    fn draw_warp_tab_background(&mut self, y: f32, height: f32, style: &WarpTabStyle) {
        let size_info = self.size_info;

        // Main background with subtle top highlight to simulate gradient
        let bg_rect = RenderRect::new(0.0, y, size_info.width(), height, style.inactive_bg, 1.0);
        // Top highlight strip
        let highlight = RenderRect::new(0.0, y, size_info.width(), 2.0, lerp_rgb(style.inactive_bg, style.active_bg, 0.12), 0.85);
        let metrics = self.glyph_cache.font_metrics();
        self.renderer_draw_rects(&size_info, &metrics, vec![bg_rect, highlight]);

        // Drop shadow if enabled
        if style.drop_shadow {
            let shadow = UiRoundedRect::new(
                0.0,
                y + height,
                size_info.width(),
                4.0,
                0.0,
                Rgb::new(0, 0, 0),
                0.1,
            );
            self.stage_ui_rounded_rect(shadow);
        }
    }

    /// Draw individual Warp-style tab
    fn draw_warp_tab(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        tab: &crate::workspace::tab_manager::TabContext,
        is_active: bool,
        style: &WarpTabStyle,
    ) {
        let height = style.tab_height;

        // Tab background with hover animation
        let is_hover_tab = matches!(self.tab_hover, Some(crate::display::TabHoverTarget::Tab(id)) if id == tab.id)
            || matches!(self.tab_hover, Some(crate::display::TabHoverTarget::Close(id)) if id == tab.id);
        let hover_progress = if is_hover_tab {
            if let Some(t0) = self.tab_hover_anim_start {
                let elapsed = t0.elapsed().as_millis() as f32;
                let dur = style.animation_duration_ms.max(1) as f32;
                (elapsed / dur).clamp(0.0, 1.0)
            } else { 1.0 }
        } else { 0.0 };
        let base_bg = if is_active { style.active_bg } else { style.inactive_bg };
        let bg_color = if is_active { base_bg } else { lerp_rgb(base_bg, style.hover_bg, hover_progress) };
        let corner_radius = if is_active { style.corner_radius } else { style.corner_radius * 0.5 };

        let tab_bg = UiRoundedRect::new(x, y, width, height, corner_radius, bg_color, 1.0);
        self.stage_ui_rounded_rect(tab_bg);

        // Active tab indicator (bottom border)
        if is_active {
            let p = if let Some(t0) = self.tab_anim_switch_start {
                let elapsed = t0.elapsed().as_millis() as f32;
                let dur = style.animation_duration_ms.max(1) as f32;
                (elapsed / dur).clamp(0.0, 1.0)
            } else { 1.0 };
            let ind_w = width * p;
            let indicator = RenderRect::new(
                x,
                y + height - 3.0,
                ind_w,
                3.0,
                Rgb::new(100, 150, 250), // Accent color
                1.0,
            );
            let size_info = self.size_info;
            let metrics = self.glyph_cache.font_metrics();
            self.renderer_draw_rects(&size_info, &metrics, vec![indicator]);
        }

        // Tab title (simplified - would need proper text rendering)
        let text_color = if is_active { style.active_fg } else { style.inactive_fg };
        let text_y = ((y + height / 2.0) / self.size_info.cell_height()) as usize;
        let text_x = ((x + style.tab_padding) / self.size_info.cell_width()) as usize;

        // Truncate title to fit
        let max_chars = ((width - style.tab_padding * 2.0) / self.size_info.cell_width()) as usize;
        let title = if tab.title.len() > max_chars.saturating_sub(3) {
            format!("{}...", &tab.title[..max_chars.saturating_sub(3)])
        } else {
            tab.title.clone()
        };

        // Draw tab text (placeholder - real implementation would use proper glyph rendering)
        let text_point = Point::new(text_y, Column(text_x));
        self.draw_warp_tab_text(text_point, text_color, bg_color, &title, max_chars);

        // Zoom indicator badge (Warp-style) on active tab when zoomed
        if is_active && tab.zoom_saved_layout.is_some() {
            let badge_x = x + 6.0;
            let badge_y = y + height / 2.0 - 3.0;
            let badge = UiRoundedRect::new(
                badge_x,
                badge_y,
                6.0,
                6.0,
                3.0,
                style.active_fg,
                0.95,
            );
            self.stage_ui_rounded_rect(badge);
        }

        // Error indicator (red) if last command exited non-zero
        if tab.last_exit_nonzero {
            let dot_x = x + width - 12.0;
            let dot_y = y + height / 2.0 - 3.0;
            let err_dot = UiRoundedRect::new(dot_x, dot_y, 6.0, 6.0, 3.0, Rgb::new(220, 70, 70), 1.0);
            self.stage_ui_rounded_rect(err_dot);
        }
        // Modified indicator (orange)
        if tab.modified {
            let dot_x = x + width - 20.0;
            let dot_y = y + height / 2.0 - 3.0;
            let modified_dot = UiRoundedRect::new(
                dot_x,
                dot_y,
                6.0,
                6.0,
                3.0,
                Rgb::new(255, 150, 0),
                1.0,
            );
            self.stage_ui_rounded_rect(modified_dot);
        }

        // Sync indicator (accent) if panes are synced
        if tab.panes_synced {
            let dot_x = x + width - 28.0;
            let dot_y = y + height / 2.0 - 3.0;
            let sync_dot = UiRoundedRect::new(dot_x, dot_y, 6.0, 6.0, 3.0, style.active_fg, 1.0);
            self.stage_ui_rounded_rect(sync_dot);
        }

        // Close button (when hovering)
        let close_x = x + width - 25.0;
        let close_y = y + height / 2.0 - 8.0;
        let close_button =
            UiRoundedRect::new(close_x, close_y, 16.0, 16.0, 8.0, Rgb::new(220, 220, 220), 0.8);
        self.stage_ui_rounded_rect(close_button);
    }

    /// Draw "+" button for creating new tabs
    fn draw_new_tab_button(&mut self, x: f32, y: f32, style: &WarpTabStyle, hovered: bool) {
        let button_size = style.tab_height * 0.8;
        let button_y = y + (style.tab_height - button_size) / 2.0;

        let bg_color = if hovered { style.hover_bg } else { style.inactive_bg };
        let bg_alpha = if hovered { 1.0 } else { 0.9 };
        let button_bg = UiRoundedRect::new(
            x,
            button_y,
            button_size,
            button_size,
            button_size / 2.0,
            bg_color,
            bg_alpha,
        );
        self.stage_ui_rounded_rect(button_bg);

        // Plus icon (simplified)
        let icon_size = button_size * 0.4;
        let icon_x = x + (button_size - icon_size) / 2.0;
        let icon_y = button_y + (button_size - 2.0) / 2.0;

        // Horizontal line
        let plus_color = if hovered { style.active_fg } else { style.inactive_fg };
        let h_line = RenderRect::new(icon_x, icon_y, icon_size, 2.0, plus_color, 1.0);
        // Vertical line
        let v_line = RenderRect::new(
            icon_x + (icon_size - 2.0) / 2.0,
            button_y + (button_size - icon_size) / 2.0,
            2.0,
            icon_size,
            plus_color,
            1.0,
        );

        let size_info = self.size_info;
        let metrics = self.glyph_cache.font_metrics();
        self.renderer_draw_rects(&size_info, &metrics, vec![h_line, v_line]);
    }

    /// Draw split pane indicators
    pub fn draw_warp_split_indicators(
        &mut self,
        config: &UiConfig,
        split_layout: &crate::workspace::split_manager::SplitLayout,
        indicators: &WarpSplitIndicators,
    ) {
        if !indicators.show_split_preview {
            return;
        }

        // Calculate pane boundaries and draw split lines inside the grid content area,
        // accounting for window padding and any reserved tab bar row.
        let si = self.size_info;
        let x0 = si.padding_x();
        let mut y0 = si.padding_y();
        let w = si.width() - 2.0 * si.padding_x();
        let mut h = si.height() - 2.0 * si.padding_y();
        if config.workspace.tab_bar.show
            && config.workspace.tab_bar.reserve_row
            && config.workspace.tab_bar.position
                != crate::workspace::TabBarPosition::Hidden
        {
            let ch = si.cell_height();
            match config.workspace.tab_bar.position {
                crate::workspace::TabBarPosition::Top => {
                    y0 += ch;
                    h = (h - ch).max(0.0);
                },
                crate::workspace::TabBarPosition::Bottom => {
                    h = (h - ch).max(0.0);
                },
                crate::workspace::TabBarPosition::Hidden => {},
            }
        }

        let container = crate::workspace::split_manager::PaneRect::new(x0, y0, w, h);

        self.draw_split_lines_recursive(split_layout, container, indicators);
    }

    /// Recursively draw split lines
    fn draw_split_lines_recursive(
        &mut self,
        layout: &crate::workspace::split_manager::SplitLayout,
        rect: crate::workspace::split_manager::PaneRect,
        indicators: &WarpSplitIndicators,
    ) {
        // Determine current hover/drag target
        let hover_hit = self
            .split_drag
            .as_ref()
            .or(self.split_hover.as_ref());

        match layout {
            crate::workspace::split_manager::SplitLayout::Horizontal { left, right, ratio } => {
                let split_x = rect.x + rect.width * ratio;

                // Is this divider hovered/dragged?
                let is_hovered = hover_hit.map_or(false, |hit| {
                    hit.axis == crate::workspace::split_manager::SplitAxis::Horizontal
                        && (hit.rect.x - rect.x).abs() < f32::EPSILON
                        && (hit.rect.y - rect.y).abs() < f32::EPSILON
                        && (hit.rect.width - rect.width).abs() < f32::EPSILON
                        && (hit.rect.height - rect.height).abs() < f32::EPSILON
                });

                // Animate hover transitions for split line
                let p = if is_hovered {
                    if let Some(t0) = self.split_hover_anim_start {
                        let elapsed = t0.elapsed().as_millis() as f32;
                        let dur = 160.0;
                        (elapsed / dur).clamp(0.0, 1.0)
                    } else { 1.0 }
                } else { 0.0 };
                let base_w = indicators.split_line_width;
                let target_w = indicators.split_line_width * indicators.hover_line_scale;
                let line_width = base_w + (target_w - base_w) * p;
                let base_a = indicators.split_line_alpha;
                let target_a = indicators.hover_line_alpha;
                let line_alpha = base_a + (target_a - base_a) * p;
                let line_color = if is_hovered {
                    indicators.split_handle_color
                } else {
                    indicators.split_line_color
                };

                // Draw vertical split line
                let split_line = RenderRect::new(
                    split_x - line_width / 2.0,
                    rect.y,
                    line_width,
                    rect.height,
                    line_color,
                    line_alpha,
                );

                let size_info = self.size_info;
                let metrics = self.glyph_cache.font_metrics();
                self.renderer_draw_rects(&size_info, &metrics, vec![split_line]);

                // Draw grab handle when hovered
                if is_hovered && indicators.show_resize_handles {
                    let handle_h = (rect.height * 0.18).clamp(18.0, 48.0);
                    let handle_w = indicators.split_handle_size.max(line_width + 2.0);
                    let handle_x = split_x - handle_w / 2.0;
                    let handle_y = rect.y + (rect.height - handle_h) / 2.0;
                    let handle = UiRoundedRect::new(
                        handle_x,
                        handle_y,
                        handle_w,
                        handle_h,
                        handle_w.min(handle_h) / 3.0,
                        indicators.split_handle_color,
                        indicators.split_handle_alpha,
                    );
                    self.stage_ui_rounded_rect(handle);
                }

                // Recursively draw child splits
                let (left_rect, right_rect) = rect.split_horizontal(*ratio);
                self.draw_split_lines_recursive(left, left_rect, indicators);
                self.draw_split_lines_recursive(right, right_rect, indicators);
            },
            crate::workspace::split_manager::SplitLayout::Vertical { top, bottom, ratio } => {
                let split_y = rect.y + rect.height * ratio;

                // Is this divider hovered/dragged?
                let is_hovered = hover_hit.map_or(false, |hit| {
                    hit.axis == crate::workspace::split_manager::SplitAxis::Vertical
                        && (hit.rect.x - rect.x).abs() < f32::EPSILON
                        && (hit.rect.y - rect.y).abs() < f32::EPSILON
                        && (hit.rect.width - rect.width).abs() < f32::EPSILON
                        && (hit.rect.height - rect.height).abs() < f32::EPSILON
                });

                // Animate hover transitions for split line
                let p = if is_hovered {
                    if let Some(t0) = self.split_hover_anim_start {
                        let elapsed = t0.elapsed().as_millis() as f32;
                        let dur = 160.0;
                        (elapsed / dur).clamp(0.0, 1.0)
                    } else { 1.0 }
                } else { 0.0 };
                let base_w = indicators.split_line_width;
                let target_w = indicators.split_line_width * indicators.hover_line_scale;
                let line_width = base_w + (target_w - base_w) * p;
                let base_a = indicators.split_line_alpha;
                let target_a = indicators.hover_line_alpha;
                let line_alpha = base_a + (target_a - base_a) * p;
                let line_color = if is_hovered {
                    indicators.split_handle_color
                } else {
                    indicators.split_line_color
                };

                // Draw horizontal split line
                let split_line = RenderRect::new(
                    rect.x,
                    split_y - line_width / 2.0,
                    rect.width,
                    line_width,
                    line_color,
                    line_alpha,
                );

                let size_info = self.size_info;
                let metrics = self.glyph_cache.font_metrics();
                self.renderer_draw_rects(&size_info, &metrics, vec![split_line]);

                // Draw grab handle when hovered
                if is_hovered && indicators.show_resize_handles {
                    let handle_w = (rect.width * 0.18).clamp(18.0, 48.0);
                    let handle_h = indicators.split_handle_size.max(line_width + 2.0);
                    let handle_x = rect.x + (rect.width - handle_w) / 2.0;
                    let handle_y = split_y - handle_h / 2.0;
                    let handle = UiRoundedRect::new(
                        handle_x,
                        handle_y,
                        handle_w,
                        handle_h,
                        handle_w.min(handle_h) / 3.0,
                        indicators.split_handle_color,
                        indicators.split_handle_alpha,
                    );
                    self.stage_ui_rounded_rect(handle);
                }

                // Recursively draw child splits
                let (top_rect, bottom_rect) = rect.split_vertical(*ratio);
                self.draw_split_lines_recursive(top, top_rect, indicators);
                self.draw_split_lines_recursive(bottom, bottom_rect, indicators);
            },
            crate::workspace::split_manager::SplitLayout::Single(_) => {
                // No splits to draw
            },
        }
    }

    /// Draw zoom overlay when a pane is zoomed
    pub fn draw_warp_zoom_overlay(
        &mut self,
        _zoomed_pane_id: crate::workspace::split_manager::PaneId,
        indicators: &WarpSplitIndicators,
    ) {
        // Draw subtle overlay to indicate zoom state
        let overlay = RenderRect::new(
            0.0,
            0.0,
            self.size_info.width(),
            self.size_info.height(),
            indicators.zoom_overlay_color,
            indicators.zoom_overlay_alpha,
        );

        let size_info = self.size_info;
        let metrics = self.glyph_cache.font_metrics();
        self.renderer_draw_rects(&size_info, &metrics, vec![overlay]);

        // Draw zoom indicator in corner
        let indicator_size = 24.0;
        let indicator_x = self.size_info.width() - indicator_size - 10.0;
        let indicator_y = 10.0;

        let zoom_indicator = UiRoundedRect::new(
            indicator_x,
            indicator_y,
            indicator_size,
            indicator_size,
            4.0,
            Rgb::new(100, 150, 250),
            0.9,
        );
        self.stage_ui_rounded_rect(zoom_indicator);
    }

    /// Helper to draw text in Warp-style tabs
    fn draw_warp_tab_text(
        &mut self,
        point: Point<usize>,
        fg: Rgb,
        bg: Rgb,
        text: &str,
        max_width: usize,
    ) {
        // This is similar to existing draw_tab_text but with Warp-style adjustments
        let truncated_text: String = if text.len() > max_width {
            text.chars().take(max_width.saturating_sub(3)).collect::<String>() + "..."
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

    /// Draw split preview when about to split
    pub fn draw_warp_split_preview(
        &mut self,
        pane_rect: crate::workspace::split_manager::PaneRect,
        split_direction: crate::workspace::warp_split_manager::WarpNavDirection,
        indicators: &WarpSplitIndicators,
    ) {
        let preview_color = indicators.split_handle_color;
        let preview_alpha = indicators.hover_line_alpha.min(1.0).max(0.0) * 0.5;

        let preview_rect = match split_direction {
            crate::workspace::warp_split_manager::WarpNavDirection::Right => {
                // Show right half highlighted
                RenderRect::new(
                    pane_rect.x + pane_rect.width / 2.0,
                    pane_rect.y,
                    pane_rect.width / 2.0,
                    pane_rect.height,
                    preview_color,
                    preview_alpha,
                )
            },
            crate::workspace::warp_split_manager::WarpNavDirection::Down => {
                // Show bottom half highlighted
                RenderRect::new(
                    pane_rect.x,
                    pane_rect.y + pane_rect.height / 2.0,
                    pane_rect.width,
                    pane_rect.height / 2.0,
                    preview_color,
                    preview_alpha,
                )
            },
            _ => return, // Only show preview for split directions
        };

        let size_info = self.size_info;
        let metrics = self.glyph_cache.font_metrics();
        self.renderer_draw_rects(&size_info, &metrics, vec![preview_rect]);
    }
}

/// Animation helper functions
impl WarpAnimation {
    /// Create new animation
    pub fn new(animation_type: WarpAnimationType, duration_ms: u32) -> Self {
        Self {
            start_time: Instant::now(),
            duration_ms,
            animation_type,
            easing: WarpEasing::EaseOut,
        }
    }

    /// Get animation progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        let elapsed = self.start_time.elapsed().as_millis() as f32;
        let progress = (elapsed / self.duration_ms as f32).min(1.0);

        match self.easing {
            WarpEasing::Linear => progress,
            WarpEasing::EaseOut => 1.0 - (1.0 - progress).powi(3),
            WarpEasing::EaseInOut => {
                if progress < 0.5 {
                    2.0 * progress * progress
                } else {
                    1.0 - (-2.0 * progress + 2.0).powi(2) / 2.0
                }
            },
            WarpEasing::Spring => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (progress - 1.0).powi(3) + c1 * (progress - 1.0).powi(2)
            },
        }
    }

    /// Check if animation is complete
    pub fn is_complete(&self) -> bool {
        self.start_time.elapsed().as_millis() >= self.duration_ms as u128
    }
}

/// Overall Warp UI style configuration
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WarpUiStyle {
    /// Tab styling
    pub tab_style: WarpTabStyle,

    /// Split indicators styling
    pub split_indicators: WarpSplitIndicators,

    /// Animation settings
    pub animations_enabled: bool,

    /// Default animation duration
    pub animation_duration_ms: u32,
}
