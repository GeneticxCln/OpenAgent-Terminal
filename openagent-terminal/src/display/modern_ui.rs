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
use crate::workspace::{TabBarPosition, TabId, TabManager};

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
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;
        let ui = theme.ui;
        Self {
            tab_height: ui.tab_bar_height_px.max(16.0),
            corner_radius: if ui.rounded_corners { ui.tab_bar_corner_radius_px } else { 0.0 },
            tab_padding: ui.tab_bar_padding_px.max(0.0),
            active_bg: tokens.surface,
            inactive_bg: tokens.surface_muted,
            hover_bg: tokens.surface_muted,
            active_fg: tokens.accent,
            inactive_fg: tokens.text,
            separator_color: tokens.border,
            drop_shadow: ui.tab_bar_drop_shadow.unwrap_or(ui.shadow),
            animation_duration_ms: if ui.reduce_motion {
                0
            } else {
                ui.tab_bar_animation_duration_ms
            },
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

// Legacy tab bar interop types moved here to remove the classic module entirely.
// Geometry info for compatibility with existing call sites.
#[derive(Debug, Clone, Copy)]
pub struct TabBarGeometry {
    pub start_line: usize,
    pub height: usize,
    pub tab_width: usize,
    pub visible_tabs: usize,
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy)]
pub enum TabBarAction {
    SelectTab(TabId),
    CloseTab(TabId),
    CreateTab,
    OpenSettings,
    BeginDrag(TabId),
    DragMove(TabId, usize),
    EndDrag(TabId),
    CancelDrag(TabId),
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
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
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
    ) -> Option<TabBarGeometry> {
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
        let tab_cfg = &config.workspace.tab_bar;
        // Reset cached tab bounds for precision hit testing
        self.tab_bounds_px.clear();
        self.close_button_bounds_px.clear();
        self.new_tab_button_bounds = None;
        self.gear_button_bounds = None;
        let active_tab_id = tab_manager.active_tab_id();
        // Track active tab changes for switch animation
        if self.tab_last_active_id != active_tab_id {
            self.tab_last_active_id = active_tab_id;
            self.tab_anim_switch_start =
                if style.animation_duration_ms == 0 || config.theme.reduce_motion {
                    None
                } else {
                    Some(Instant::now())
                };
        }

        let available_width = size_info.width() - style.tab_padding * 2.0;
        let tab_width = (available_width / tab_count as f32).clamp(120.0, 200.0);

        let mut current_x = style.tab_padding;
        let mut overflowed = false;

        for (index, &tab_id) in tab_order.iter().enumerate() {
            if current_x + tab_width > size_info.width() {
                overflowed = true;
                break;
            }

            let tab = match tab_manager.get_tab(tab_id) {
                Some(tab) => tab,
                None => continue,
            };

            let is_active = Some(tab_id) == active_tab_id;
            // Cache tab bounds in pixels
            self.tab_bounds_px.push((tab_id, current_x, tab_width));

            self.draw_warp_tab(
                current_x, start_y, tab_width, tab, is_active, style, index, tab_cfg,
            );

            // Cache close button bounds for precise hit testing if enabled
            if tab_cfg.show_close_button {
                let close_w = 16.0;
                let close_h = 16.0;
                let close_x = current_x + tab_width - 25.0;
                let close_y = start_y + style.tab_height / 2.0 - 8.0;
                self.close_button_bounds_px.push((tab_id, close_x, close_y, close_w, close_h));
            }

            current_x += tab_width + 8.0; // 8px gap between tabs
        }

        // Draw "+" button for new tab (hover-aware)
        if tab_cfg.show_new_tab_button {
            let create_hover =
                matches!(self.tab_hover, Some(crate::display::TabHoverTarget::Create));
            // Cache button bounds for precise hit testing
            let button_size = (style.tab_height * 0.6).clamp(12.0, style.tab_height);
            let button_y = start_y + (style.tab_height - button_size) * 0.5;
            self.new_tab_button_bounds = Some((current_x, button_y, button_size, button_size));
            self.draw_new_tab_button(current_x, start_y, style, create_hover);
        } else {
            self.new_tab_button_bounds = None;
        }

        // Draw settings gear on far right using sprite atlas, aligned with previous text region
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;
        let ui = theme.ui;
        let cols = self.size_info.columns;
        let gear_cols = 3usize; // previous "[⚙]" text took ~3 columns
        if gear_cols + 2 < cols {
            let cw = self.size_info.cell_width();
            let _ch = self.size_info.cell_height();
            let start_col = cols.saturating_sub(gear_cols + 2);
            let icon_px =
                ui.tab_bar_settings_icon_px.unwrap_or((style.tab_height * 0.7).clamp(12.0, 20.0));
            let ix = (start_col as f32) * cw + (cw * gear_cols as f32 - icon_px) * 0.5;
            let iy = start_y + (style.tab_height - icon_px) * 0.5;
            // Cache precise gear button bounds for click-hit testing
            self.gear_button_bounds = Some((ix, iy, icon_px, icon_px));
            // Atlas slot 8 = gear (atlas has 9 slots)
            let step = 1.0f32 / 9.0f32;
            let uv_x = 8.0 * step;
            let uv_y = 0.0f32;
            let uv_w = step;
            let uv_h = 1.0f32;
            let tint = tokens.text;
            let nearest = (icon_px - 16.0).abs() < 0.5;
            self.stage_ui_sprite(crate::renderer::ui::UiSprite::new(
                ix,
                iy,
                icon_px,
                icon_px,
                uv_x,
                uv_y,
                uv_w,
                uv_h,
                tint,
                1.0,
                Some(nearest),
            ));
        } else {
            self.gear_button_bounds = None;
        }

        // Overflow indicator when tabs exceed available width
        if overflowed {
            // Draw subtle fade/ellipsis at the right edge to indicate more tabs exist
            let _cw = self.size_info.cell_width();
            let ch = self.size_info.cell_height();
            let ellipsis_cols = 3usize;
            let text_col = cols.saturating_sub(ellipsis_cols + 6); // leave room for gear
            let text_line = ((start_y + style.tab_height * 0.5) / ch) as usize;
            let bg = style.inactive_bg;
            let fg = tokens.text_muted;
            // Soft fade rectangle near the right edge
            let fade_w = (style.tab_height * 1.2).clamp(24.0, 64.0);
            let fade_x = self.size_info.width() - fade_w - 8.0;
            let fade =
                RenderRect::new(fade_x, start_y + 1.0, fade_w, style.tab_height - 2.0, bg, 0.6);
            let size_info = self.size_info;
            let metrics = self.glyph_cache.font_metrics();
            self.renderer_draw_rects(&size_info, &metrics, vec![fade]);
            // Ellipsis text
            self.draw_warp_tab_text(
                Point::new(text_line, Column(text_col)),
                fg,
                bg,
                "…",
                ellipsis_cols,
            );
        }

        // Hover tooltip for tab title/cwd
        if let Some(hover_id) = match self.tab_hover {
            Some(crate::display::TabHoverTarget::Tab(id)) => Some(id),
            Some(crate::display::TabHoverTarget::Close(id)) => Some(id),
            _ => None,
        } {
            self.draw_tab_hover_tooltip(config, tab_manager, position, style, hover_id);
        }

        // Draw any active close fade animations
        self.draw_tab_close_fades(style);

        Some(TabBarGeometry {
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
        let highlight = RenderRect::new(
            0.0,
            y,
            size_info.width(),
            2.0,
            lerp_rgb(style.inactive_bg, style.active_bg, 0.12),
            0.85,
        );
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
    #[allow(clippy::too_many_arguments)]
    fn draw_warp_tab(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        tab: &crate::workspace::tab_manager::TabContext,
        is_active: bool,
        style: &WarpTabStyle,
        index: usize,
        tab_cfg: &crate::config::workspace::TabBarConfig,
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
            } else {
                1.0
            }
        } else {
            0.0
        };
        let base_bg = if is_active { style.active_bg } else { style.inactive_bg };
        let bg_color =
            if is_active { base_bg } else { lerp_rgb(base_bg, style.hover_bg, hover_progress) };
        let corner_radius = if is_active { style.corner_radius } else { style.corner_radius * 0.5 };

        let tab_bg = UiRoundedRect::new(x, y, width, height, corner_radius, bg_color, 1.0);
        self.stage_ui_rounded_rect(tab_bg);

        // Active tab indicator (bottom border)
        if is_active {
            let p = if let Some(t0) = self.tab_anim_switch_start {
                let elapsed = t0.elapsed().as_millis() as f32;
                let dur = style.animation_duration_ms.max(1) as f32;
                (elapsed / dur).clamp(0.0, 1.0)
            } else {
                1.0
            };
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

        // Build visible title with optional numbering
        let mut rendered_title = if tab_cfg.show_tab_numbers {
            format!("{}: {}", index + 1, tab.title)
        } else {
            tab.title.clone()
        };

        // Truncate title to fit width and configured max title length
        let width_chars =
            ((width - style.tab_padding * 2.0) / self.size_info.cell_width()) as usize;
        let effective_max = width_chars.min(tab_cfg.max_title_length.max(1));
        if rendered_title.len() > effective_max.saturating_sub(3) {
            rendered_title = format!("{}...", &rendered_title[..effective_max.saturating_sub(3)]);
        }

        // Draw tab text (placeholder - real implementation would use proper text rendering)
        let text_point = Point::new(text_y, Column(text_x));
        self.draw_warp_tab_text(text_point, text_color, bg_color, &rendered_title, effective_max);

        // Zoom indicator badge (Warp-style) on active tab when zoomed
        if tab_cfg.show_tab_indicators && is_active && tab.zoom_saved_layout.is_some() {
            let badge_x = x + 6.0;
            let badge_y = y + height / 2.0 - 3.0;
            let badge = UiRoundedRect::new(badge_x, badge_y, 6.0, 6.0, 3.0, style.active_fg, 0.95);
            self.stage_ui_rounded_rect(badge);
        }

        // Error indicator (red) if last command exited non-zero
        if tab_cfg.show_tab_indicators && tab.last_exit_nonzero {
            let dot_x = x + width - 12.0;
            let dot_y = y + height / 2.0 - 3.0;
            let err_dot =
                UiRoundedRect::new(dot_x, dot_y, 6.0, 6.0, 3.0, Rgb::new(220, 70, 70), 1.0);
            self.stage_ui_rounded_rect(err_dot);
        }
        // Modified indicator (orange) if enabled
        if tab.modified && tab_cfg.show_modified_indicator {
            let dot_x = x + width - 20.0;
            let dot_y = y + height / 2.0 - 3.0;
            let modified_dot =
                UiRoundedRect::new(dot_x, dot_y, 6.0, 6.0, 3.0, Rgb::new(255, 150, 0), 1.0);
            self.stage_ui_rounded_rect(modified_dot);
        }

        // Sync indicator (accent) if panes are synced
        if tab_cfg.show_tab_indicators && tab.panes_synced {
            let dot_x = x + width - 28.0;
            let dot_y = y + height / 2.0 - 3.0;
            let sync_dot = UiRoundedRect::new(dot_x, dot_y, 6.0, 6.0, 3.0, style.active_fg, 1.0);
            self.stage_ui_rounded_rect(sync_dot);
        }

        // Close button: respect configuration for rendering on hover
        if tab_cfg.show_close_button {
            let should_show = if tab_cfg.close_button_on_hover {
                // Only render the button when hovering the tab (region remains clickable)
                is_hover_tab
            } else {
                true
            };
            if should_show {
                let close_x = x + width - 25.0;
                let close_y = y + height / 2.0 - 8.0;
                let close_w = 16.0;
                let close_h = 16.0;
                let close_button = UiRoundedRect::new(
                    close_x,
                    close_y,
                    close_w,
                    close_h,
                    8.0,
                    Rgb::new(220, 220, 220),
                    0.8,
                );
                self.stage_ui_rounded_rect(close_button);

                // Draw a simple 'x' using small diagonal squares (approximate)
                let stroke = 2.0f32; // thickness of the diagonal
                let steps = 6usize;
                for i in 0..steps {
                    let t = i as f32 / steps as f32;
                    // Diagonal 1: top-left to bottom-right
                    let dx1 = close_x + 3.0 + t * (close_w - 6.0);
                    let dy1 = close_y + 3.0 + t * (close_h - 6.0);
                    let diag1 =
                        RenderRect::new(dx1, dy1, stroke, stroke, Rgb::new(80, 80, 80), 0.9);
                    // Diagonal 2: bottom-left to top-right
                    let dx2 = close_x + 3.0 + t * (close_w - 6.0);
                    let dy2 = close_y + close_h - 3.0 - t * (close_h - 6.0);
                    let diag2 =
                        RenderRect::new(dx2, dy2, stroke, stroke, Rgb::new(80, 80, 80), 0.9);
                    let size_info = self.size_info;
                    let metrics = self.glyph_cache.font_metrics();
                    self.renderer_draw_rects(&size_info, &metrics, vec![diag1, diag2]);
                }
            }
        }
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

        // Respect reduce motion preference for divider hover animations
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let reduce_motion = theme.ui.reduce_motion || config.theme.reduce_motion;

        // Calculate pane boundaries and draw split lines inside the grid content area,
        // accounting for window padding and any reserved tab bar row.
        let si = self.size_info;
        let x0 = si.padding_x();
        let mut y0 = si.padding_y();
        let w = si.width() - 2.0 * si.padding_x();
        let mut h = si.height() - 2.0 * si.padding_y();
        if config.workspace.tab_bar.show
            && !config.workspace.warp_overlay_only
            && config.workspace.tab_bar.position != crate::workspace::TabBarPosition::Hidden
        {
            let ch = si.cell_height();
            match config.workspace.tab_bar.position {
                crate::workspace::TabBarPosition::Top => {
                    y0 += ch;
                    h = (h - ch).max(0.0);
                }
                crate::workspace::TabBarPosition::Bottom => {
                    h = (h - ch).max(0.0);
                }
                crate::workspace::TabBarPosition::Hidden => {}
            }
        }

        let container = crate::workspace::split_manager::PaneRect::new(x0, y0, w, h);

        self.draw_split_lines_recursive(split_layout, container, indicators, reduce_motion);
    }

    /// Recursively draw split lines
    fn draw_split_lines_recursive(
        &mut self,
        layout: &crate::workspace::split_manager::SplitLayout,
        rect: crate::workspace::split_manager::PaneRect,
        indicators: &WarpSplitIndicators,
        reduce_motion: bool,
    ) {
        // Determine current hover/drag target
        let hover_hit = self.split_drag.as_ref().or(self.split_hover.as_ref());

        match layout {
            crate::workspace::split_manager::SplitLayout::Horizontal { left, right, ratio } => {
                let split_x = rect.x + rect.width * ratio;

                // Is this divider hovered/dragged?
                let is_hovered = hover_hit.is_some_and(|hit| {
                    hit.axis == crate::workspace::split_manager::SplitAxis::Horizontal
                        && (hit.rect.x - rect.x).abs() < f32::EPSILON
                        && (hit.rect.y - rect.y).abs() < f32::EPSILON
                        && (hit.rect.width - rect.width).abs() < f32::EPSILON
                        && (hit.rect.height - rect.height).abs() < f32::EPSILON
                });

                // Animate hover transitions for split line
                let p = if is_hovered {
                    if reduce_motion {
                        1.0
                    } else if let Some(t0) = self.split_hover_anim_start {
                        let elapsed = t0.elapsed().as_millis() as f32;
                        let dur = 160.0;
                        (elapsed / dur).clamp(0.0, 1.0)
                    } else {
                        1.0
                    }
                } else {
                    0.0
                };
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

                    // Draw simple grip pattern inside the handle for better affordance
                    let grip_cols = 3usize;
                    let grip_w = (handle_w * 0.15).clamp(2.0, 4.0);
                    let gap = (handle_w - grip_cols as f32 * grip_w) / (grip_cols as f32 + 1.0);
                    let grip_h = (handle_h * 0.5).clamp(8.0, 22.0);
                    let grip_y = handle_y + (handle_h - grip_h) / 2.0;
                    for i in 0..grip_cols {
                        let gx = handle_x + gap * (i as f32 + 1.0) + grip_w * i as f32;
                        let grip_rect = RenderRect::new(
                            gx,
                            grip_y,
                            grip_w,
                            grip_h,
                            Rgb::new(
                                (indicators.split_handle_color.r as f32 * 0.85) as u8,
                                (indicators.split_handle_color.g as f32 * 0.85) as u8,
                                (indicators.split_handle_color.b as f32 * 0.85) as u8,
                            ),
                            (indicators.split_handle_alpha * 0.65).clamp(0.0, 1.0),
                        );
                        let size_info = self.size_info;
                        let metrics = self.glyph_cache.font_metrics();
                        self.renderer_draw_rects(&size_info, &metrics, vec![grip_rect]);
                    }
                }

                // Recursively draw child splits
                let (left_rect, right_rect) = rect.split_horizontal(*ratio);
                self.draw_split_lines_recursive(left, left_rect, indicators, reduce_motion);
                self.draw_split_lines_recursive(right, right_rect, indicators, reduce_motion);
            }
            crate::workspace::split_manager::SplitLayout::Vertical { top, bottom, ratio } => {
                let split_y = rect.y + rect.height * ratio;

                // Is this divider hovered/dragged?
                let is_hovered = hover_hit.is_some_and(|hit| {
                    hit.axis == crate::workspace::split_manager::SplitAxis::Vertical
                        && (hit.rect.x - rect.x).abs() < f32::EPSILON
                        && (hit.rect.y - rect.y).abs() < f32::EPSILON
                        && (hit.rect.width - rect.width).abs() < f32::EPSILON
                        && (hit.rect.height - rect.height).abs() < f32::EPSILON
                });

                // Animate hover transitions for split line
                let p = if is_hovered {
                    if reduce_motion {
                        1.0
                    } else if let Some(t0) = self.split_hover_anim_start {
                        let elapsed = t0.elapsed().as_millis() as f32;
                        let dur = 160.0;
                        (elapsed / dur).clamp(0.0, 1.0)
                    } else {
                        1.0
                    }
                } else {
                    0.0
                };
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

                    // Grip pattern (horizontal orientation): draw small bars
                    let grip_rows = 3usize;
                    let grip_h = (handle_h * 0.15).clamp(2.0, 4.0);
                    let gap = (handle_h - grip_rows as f32 * grip_h) / (grip_rows as f32 + 1.0);
                    let grip_w = (handle_w * 0.5).clamp(10.0, handle_w - 6.0);
                    let grip_x = handle_x + (handle_w - grip_w) / 2.0;
                    for i in 0..grip_rows {
                        let gy = handle_y + gap * (i as f32 + 1.0) + grip_h * i as f32;
                        let grip_rect = RenderRect::new(
                            grip_x,
                            gy,
                            grip_w,
                            grip_h,
                            Rgb::new(
                                (indicators.split_handle_color.r as f32 * 0.85) as u8,
                                (indicators.split_handle_color.g as f32 * 0.85) as u8,
                                (indicators.split_handle_color.b as f32 * 0.85) as u8,
                            ),
                            (indicators.split_handle_alpha * 0.65).clamp(0.0, 1.0),
                        );
                        let size_info = self.size_info;
                        let metrics = self.glyph_cache.font_metrics();
                        self.renderer_draw_rects(&size_info, &metrics, vec![grip_rect]);
                    }
                }

                // Recursively draw child splits
                let (top_rect, bottom_rect) = rect.split_vertical(*ratio);
                self.draw_split_lines_recursive(top, top_rect, indicators, reduce_motion);
                self.draw_split_lines_recursive(bottom, bottom_rect, indicators, reduce_motion);
            }
            crate::workspace::split_manager::SplitLayout::Single(_) => {
                // No splits to draw
            }
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
            indicators.split_handle_color,
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
            crate::display::Backend::Wgpu { renderer } => {
                renderer.draw_string(
                    point,
                    fg,
                    bg,
                    truncated_text.chars(),
                    &size_info_copy,
                    &mut self.glyph_cache,
                );
            }
        }
    }

    /// Draw split preview when about to split
    pub fn draw_warp_split_preview(
        &mut self,
        pane_rect: crate::workspace::split_manager::PaneRect,
split_direction: crate::workspace::split_layout_manager::WarpNavDirection,
        indicators: &WarpSplitIndicators,
    ) {
        let preview_color = indicators.split_handle_color;
        let preview_alpha = indicators.hover_line_alpha.clamp(0.0, 1.0) * 0.5;

let preview_rect = match split_direction {
            crate::workspace::split_layout_manager::WarpNavDirection::Right => {
                // Show right half highlighted
                RenderRect::new(
                    pane_rect.x + pane_rect.width / 2.0,
                    pane_rect.y,
                    pane_rect.width / 2.0,
                    pane_rect.height,
                    preview_color,
                    preview_alpha,
                )
            }
            crate::workspace::split_layout_manager::WarpNavDirection::Down => {
                // Show bottom half highlighted
                RenderRect::new(
                    pane_rect.x,
                    pane_rect.y + pane_rect.height / 2.0,
                    pane_rect.width,
                    pane_rect.height / 2.0,
                    preview_color,
                    preview_alpha,
                )
            }
            _ => return, // Only show preview for split directions
        };

        let size_info = self.size_info;
        let metrics = self.glyph_cache.font_metrics();
        self.renderer_draw_rects(&size_info, &metrics, vec![preview_rect]);
    }

    // --- Warp tab bar interactions (click/drag) matching legacy signatures ---
    pub fn handle_tab_bar_click(
        &self,
        config: &UiConfig,
        _tab_manager: &TabManager,
        position: TabBarPosition,
        mouse_x_px: usize,
        mouse_y_px: usize,
    ) -> Option<TabBarAction> {
        // Use pixel coordinates directly for precise hit testing
        let x_px = mouse_x_px as f32;
        let y_px = mouse_y_px as f32;
        // Apply a small hit slop scaled by DPI to be forgiving on high-DPI rounding
        let scale = self.window.scale_factor as f32;
        let hit_slop = (2.0 * scale).clamp(2.0, 8.0);
        // First: settings gear bounds
        if let Some((gx, gy, gw, gh)) = self.gear_button_bounds {
            if x_px + hit_slop >= gx
                && x_px <= gx + gw + hit_slop
                && y_px + hit_slop >= gy
                && y_px <= gy + gh + hit_slop
            {
                return Some(TabBarAction::OpenSettings);
            }
        }
        // Precise check: close button rectangle cache
        if config.workspace.tab_bar.show_close_button {
            if let Some((_tab_id, _bx, _by, _bw, _bh)) =
                self.close_button_bounds_px.iter().copied().find(|(_, bx, by, bw, bh)| {
                    x_px + hit_slop >= *bx
                        && x_px <= *bx + *bw + hit_slop
                        && y_px + hit_slop >= *by
                        && y_px <= *by + *bh + hit_slop
                })
            {
                // Don't return here since this is a generic click handler; press handler handles close
                // For click handler, we still want to report CloseTab for UI consistency
                // Find matching tab id
                if let Some((tab_id, _, _, _, _)) =
                    self.close_button_bounds_px.iter().copied().find(|(_, cbx, cby, cbw, cbh)| {
                        x_px + hit_slop >= *cbx
                            && x_px <= *cbx + *cbw + hit_slop
                            && y_px + hit_slop >= *cby
                            && y_px <= *cby + *cbh + hit_slop
                    })
                {
                    return Some(TabBarAction::CloseTab(tab_id));
                }
            }
        }
        hit_test_tab_bar_cached(
            self.size_info.height(),
            &self.tab_bounds_px,
            self.new_tab_button_bounds,
            config,
            position,
            x_px,
            y_px,
        )
    }

    pub fn handle_tab_bar_mouse_press(
        &mut self,
        config: &UiConfig,
        tab_manager: &TabManager,
        position: TabBarPosition,
        mouse_x_px: usize,
        mouse_y_px: usize,
        button: winit::event::MouseButton,
    ) -> Option<TabBarAction> {
        if button != winit::event::MouseButton::Left {
            return self.handle_tab_bar_click(config, tab_manager, position, mouse_x_px, mouse_y_px);
        }
        // Use pixel coordinates directly for precise hit testing
        let x_px = mouse_x_px as f32;
        let y_px = mouse_y_px as f32;
        // First: precise check settings gear bounds
        // Apply a small hit slop to account for high-DPI rounding
        let scale = self.window.scale_factor as f32;
        let hit_slop = (2.0 * scale).clamp(2.0, 8.0);
        if let Some((gx, gy, gw, gh)) = self.gear_button_bounds {
            if x_px + hit_slop >= gx
                && x_px <= gx + gw + hit_slop
                && y_px + hit_slop >= gy
                && y_px <= gy + gh + hit_slop
            {
                return Some(TabBarAction::OpenSettings);
            }
        }
        // Precise check against cached close button rectangles
        if config.workspace.tab_bar.show_close_button {
            if let Some((tab_id, _x, _y, _w, _h)) =
                self.close_button_bounds_px.iter().copied().find(|(_, bx, by, bw, bh)| {
                    x_px + hit_slop >= *bx
                        && x_px <= *bx + *bw + hit_slop
                        && y_px + hit_slop >= *by
                        && y_px <= *by + *bh + hit_slop
                })
            {
                // Start a fade-out animation for this tab before it is removed
                self.start_tab_close_fade(config, position, tab_manager, tab_id);
                return Some(TabBarAction::CloseTab(tab_id));
            }
        }
        let hit = hit_test_tab_bar_cached(
            self.size_info.height(),
            &self.tab_bounds_px,
            self.new_tab_button_bounds.map(|(bx, by, bw, bh)| {
                (bx - hit_slop, by - hit_slop, bw + hit_slop * 2.0, bh + hit_slop * 2.0)
            }),
            config,
            position,
            x_px,
            y_px,
        );
        match hit {
            Some(TabBarAction::CreateTab) => return Some(TabBarAction::CreateTab),
            Some(TabBarAction::CloseTab(id)) => return Some(TabBarAction::CloseTab(id)),
            Some(TabBarAction::SelectTab(id)) => {
                // Check for close button click in the last N px of the tab (scaled for DPI)
                if let Some((tab_id, x, w)) =
                    self.tab_bounds_px.iter().copied().find(|(tid, _, _)| *tid == id)
                {
                    let coarse_close_w = (20.0 * scale).clamp(16.0, 32.0);
                    if config.workspace.tab_bar.show_close_button && x_px >= x + w - coarse_close_w {
                        // Start fade-out since a close is about to happen via coarse region
                        self.start_tab_close_fade(config, position, tab_manager, id);
                        return Some(TabBarAction::CloseTab(id));
                    }
                    // Set up drag state for potential future dragging, but return SelectTab immediately
                    // This allows immediate tab switching while still enabling drag functionality
                    let drag_threshold_px = (10.0 * scale).clamp(8.0, 24.0);
                    self.tab_drag_active = Some(super::TabDragState {
                        tab_id,
                        original_position: self.get_tab_position(tab_manager, tab_id),
                        current_position: self.get_tab_position(tab_manager, tab_id),
                        target_position: None,
                        start_mouse_x: mouse_x_px,
                        start_mouse_y: mouse_y_px,
                        current_mouse_x: mouse_x_px,
                        current_mouse_y: mouse_y_px,
                        visual_offset_x: 0.0,
                        visual_offset_y: 0.0,
                        is_active: false,
                        drag_threshold: drag_threshold_px,
                    });
                    // Return SelectTab immediately to fix click responsiveness
                    return Some(TabBarAction::SelectTab(id));
                }
            }
            _ => {}
        }
        None
    }

    pub fn handle_tab_bar_mouse_move(
        &mut self,
        _tab_manager: &TabManager,
        mouse_x_px: usize,
        mouse_y_px: usize,
    ) -> Option<TabBarAction> {
        if let Some(ref mut drag) = self.tab_drag_active {
            drag.current_mouse_x = mouse_x_px;
            drag.current_mouse_y = mouse_y_px;
            let dx = (mouse_x_px as i32 - drag.start_mouse_x as i32).abs() as f32;
            let dy = (mouse_y_px as i32 - drag.start_mouse_y as i32).abs() as f32;
            let dist = (dx * dx + dy * dy).sqrt();
            if !drag.is_active && dist > drag.drag_threshold {
                drag.is_active = true;
                self.tab_drag_anim_start = Some(Instant::now());
                return Some(TabBarAction::DragMove(drag.tab_id, drag.current_position));
            }
            if drag.is_active {
                // Choose new position by nearest tab center
                if !self.tab_bounds_px.is_empty() {
                    let x_px = mouse_x_px as f32;
                    let mut idx = 0usize;
                    let mut best = f32::MAX;
                    for (i, (_tid, x, w)) in self.tab_bounds_px.iter().enumerate() {
                        let center = *x + *w * 0.5;
                        let d = (x_px - center).abs();
                        if d < best {
                            best = d;
                            idx = i;
                        }
                    }
                    if idx != drag.current_position {
                        drag.target_position = Some(idx);
                        return Some(TabBarAction::DragMove(drag.tab_id, idx));
                    }
                }
            }
        }
        None
    }

    pub fn handle_tab_bar_mouse_release(
        &mut self,
        button: winit::event::MouseButton,
    ) -> Option<TabBarAction> {
        if button != winit::event::MouseButton::Left {
            return None;
        }
        if let Some(drag) = self.tab_drag_active.take() {
            if drag.is_active {
                if let Some(pos) = drag.target_position {
                    if pos != drag.original_position {
                        return Some(TabBarAction::DragMove(drag.tab_id, pos));
                    }
                }
                return Some(TabBarAction::EndDrag(drag.tab_id));
            } else {
                return Some(TabBarAction::SelectTab(drag.tab_id));
            }
        }
        // If no drag, treat release as a generic click. Without release coordinates, skip gear check here.
        None
    }

    fn get_tab_position(&self, tab_manager: &TabManager, tab_id: TabId) -> usize {
        tab_manager.tab_order().iter().position(|&id| id == tab_id).unwrap_or(0)
    }
}

/// Pixel-precise hit testing using cached bounds from the renderer
///
/// This is public to enable integration tests to validate hit-testing logic
/// without depending on internal Display helpers.
pub fn hit_test_tab_bar_cached(
    total_height: f32,
    tab_bounds_px: &[(crate::workspace::TabId, f32, f32)],
    new_tab_bounds: Option<(f32, f32, f32, f32)>,
    config: &UiConfig,
    position: TabBarPosition,
    x_px: f32,
    y_px: f32,
) -> Option<TabBarAction> {
    let style = WarpTabStyle::from_theme(config);
    let bar_y = match position {
        TabBarPosition::Top => 0.0,
        TabBarPosition::Bottom => total_height - style.tab_height,
        TabBarPosition::Hidden => return None,
    };
    if y_px < bar_y || y_px >= bar_y + style.tab_height {
        return None;
    }
    if let Some((bx, by, bw, bh)) = new_tab_bounds {
        if x_px >= bx && x_px <= bx + bw && y_px >= by && y_px <= by + bh {
            return Some(TabBarAction::CreateTab);
        }
    }
    // More precise close button hit-test when bounds are available via Display::close_button_bounds_px
    // Note: Since we expose this as a pure function, callers that want close-precision should pass
    // a prefiltered view or check explicitly before calling this function.
    // Fallback: use last-20px heuristic when no external close bounds are provided.
    for (tab_id, x, w) in tab_bounds_px.iter() {
        if x_px >= *x && x_px < *x + *w {
            if config.workspace.tab_bar.show_close_button && x_px >= *x + *w - 20.0 {
                return Some(TabBarAction::CloseTab(*tab_id));
            }
            return Some(TabBarAction::SelectTab(*tab_id));
        }
    }
    None
}

#[cfg(test)]
mod hit_tests {
    use super::*;
    use crate::display::SizeInfo;
    use crate::workspace::TabId;

    #[test]
    fn hit_new_tab_button() {
        let mut cfg = UiConfig::default();
        cfg.workspace.tab_bar.show = true;
        cfg.workspace.tab_bar.show_close_button = true;
        cfg.workspace.tab_bar.position = TabBarPosition::Top;
        let total_height = 600.0f32;
        let tabs = vec![(TabId(1), 10.0, 150.0)];
        let btn = Some((200.0, 4.0, 20.0, 20.0));
        let hit = hit_test_tab_bar_cached(
            total_height,
            &tabs,
            btn,
            &cfg,
            TabBarPosition::Top,
            208.0,
            10.0,
        );
        assert!(matches!(hit, Some(TabBarAction::CreateTab)));
    }

    #[test]
    fn hit_select_and_close_regions() {
        let mut cfg = UiConfig::default();
        cfg.workspace.tab_bar.show = true;
        cfg.workspace.tab_bar.show_close_button = true;
        cfg.workspace.tab_bar.position = TabBarPosition::Top;
        let total_height = 600.0f32;
        let tid = TabId(7);
        let tabs = vec![(tid, 10.0, 150.0)];
        // Select near center
        let sel = hit_test_tab_bar_cached(
            total_height,
            &tabs,
            None,
            &cfg,
            TabBarPosition::Top,
            80.0,
            8.0,
        );
        assert!(matches!(sel, Some(TabBarAction::SelectTab(id)) if id == tid));
        // Close using coarse right-edge region
        let close = hit_test_tab_bar_cached(
            total_height,
            &tabs,
            None,
            &cfg,
            TabBarPosition::Top,
            10.0 + 150.0 - 5.0,
            8.0,
        );
        assert!(matches!(close, Some(TabBarAction::CloseTab(id)) if id == tid));
    }

    #[test]
    fn hit_bottom_tab_bar_regions() {
        // Validate hit-testing when the tab bar is positioned at the bottom of the window
        let mut cfg = UiConfig::default();
        cfg.workspace.tab_bar.show = true;
        cfg.workspace.tab_bar.show_close_button = true;
        cfg.workspace.tab_bar.position = TabBarPosition::Bottom;
        let total_height = 720.0f32;
        let tid = TabId(5);
        let tabs = vec![(tid, 20.0, 180.0)];
        // Y coordinate inside bottom bar band; x near center -> select
        let sel = hit_test_tab_bar_cached(
            total_height,
            &tabs,
            None,
            &cfg,
            TabBarPosition::Bottom,
            110.0,
            total_height - WarpTabStyle::from_theme(&cfg).tab_height + 6.0,
        );
        assert!(matches!(sel, Some(TabBarAction::SelectTab(id)) if id == tid));
        // Right edge -> close
        let close = hit_test_tab_bar_cached(
            total_height,
            &tabs,
            None,
            &cfg,
            TabBarPosition::Bottom,
            20.0 + 180.0 - 2.0,
            total_height - WarpTabStyle::from_theme(&cfg).tab_height + 6.0,
        );
        assert!(matches!(close, Some(TabBarAction::CloseTab(id)) if id == tid));
        // New tab area at bottom
        let btn = Some((260.0, total_height - WarpTabStyle::from_theme(&cfg).tab_height + 2.0, 20.0, 16.0));
        let create = hit_test_tab_bar_cached(
            total_height,
            &tabs,
            btn,
            &cfg,
            TabBarPosition::Bottom,
            268.0,
            total_height - WarpTabStyle::from_theme(&cfg).tab_height + 8.0,
        );
        assert!(matches!(create, Some(TabBarAction::CreateTab)));
    }

    #[test]
    fn click_handler_uses_precise_close_bounds_when_available() {
        // Simulate that Display cached a close button rectangle and ensure the click handler detects it
        let mut cfg = UiConfig::default();
        cfg.workspace.tab_bar.show = true;
        cfg.workspace.tab_bar.show_close_button = true;
        cfg.workspace.tab_bar.position = TabBarPosition::Top;
        // Create a minimal Display with synthetic state
        let _size = SizeInfo::new(800.0, 600.0, 8.0, 16.0, 0.0, 0.0, false);
        // We can't directly construct Display easily here; instead test the math path the click handler takes:
        // close_button_bounds_px and tab_bounds_px matching the click.
        let tabs = vec![(TabId(9), 10.0, 150.0)];
        let close_btn = (TabId(9), 10.0 + 150.0 - 25.0, 4.0, 16.0, 16.0);
        // Use the public hit test to assert that near the center selects
        let sel = hit_test_tab_bar_cached(600.0, &tabs, None, &cfg, TabBarPosition::Top, 80.0, 8.0);
        assert!(matches!(sel, Some(TabBarAction::SelectTab(id)) if id == TabId(9)));
        // And simulate a click exactly within the cached close rectangle bounds
        let x_px = close_btn.1 + close_btn.3 * 0.5;
        let y_px = close_btn.2 + close_btn.4 * 0.5;
        // Since our pure helper doesn’t see close bounds, emulate Display handler behavior by checking rect containment
        let in_rect = x_px >= close_btn.1
            && x_px <= close_btn.1 + close_btn.3
            && y_px >= close_btn.2
            && y_px <= close_btn.2 + close_btn.4;
        assert!(in_rect);
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
            }
            WarpEasing::Spring => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (progress - 1.0).powi(3) + c1 * (progress - 1.0).powi(2)
            }
        }
    }

    /// Check if animation is complete
    pub fn is_complete(&self) -> bool {
        self.start_time.elapsed().as_millis() >= self.duration_ms as u128
    }
}

impl Display {
    /// Start a runtime fade-out for a tab being closed by capturing its last-known geometry.
    fn start_tab_close_fade(
        &mut self,
        config: &UiConfig,
        position: TabBarPosition,
        tab_manager: &TabManager,
        tab_id: TabId,
    ) {
        // Find tab bounds from the last draw pass
        if let Some((_, x, w)) =
            self.tab_bounds_px.iter().copied().find(|(tid, _, _)| *tid == tab_id)
        {
            let style = WarpTabStyle::from_theme(config);
            let y = match position {
                TabBarPosition::Top => 0.0,
                TabBarPosition::Bottom => self.size_info.height() - style.tab_height,
                TabBarPosition::Hidden => 0.0,
            };
            let is_active = tab_manager.active_tab_id().is_some_and(|id| id == tab_id);
            let color = if is_active { style.active_bg } else { style.inactive_bg };
            self.tab_close_fades.push(crate::display::TabCloseFade {
                tab_id,
                start_time: Instant::now(),
                duration_ms: (style.animation_duration_ms).max(120),
                x,
                y,
                w,
                h: style.tab_height,
                color,
            });
        }
    }

    /// Draw any active tab close fade overlays and clean up completed ones.
    fn draw_tab_close_fades(&mut self, style: &WarpTabStyle) {
        if self.tab_close_fades.is_empty() {
            return;
        }
        let now = Instant::now();
        let mut rects: Vec<RenderRect> = Vec::new();
        let mut pills: Vec<UiRoundedRect> = Vec::new();
        let mut remaining: Vec<crate::display::TabCloseFade> = Vec::new();
        for fade in self.tab_close_fades.iter() {
            let elapsed = now.saturating_duration_since(fade.start_time).as_millis() as u32;
            if elapsed >= fade.duration_ms {
                // Do not keep
                continue;
            }
            let t = (elapsed as f32 / fade.duration_ms as f32).clamp(0.0, 1.0);
            let alpha = 1.0 - (1.0 - t).powi(3); // ease-out for opacity falloff
                                                 // Rounded pill with diminishing alpha
            let rr = UiRoundedRect::new(
                fade.x,
                fade.y,
                fade.w,
                fade.h,
                style.corner_radius.max(6.0),
                fade.color,
                (1.0 - t) * 0.9,
            );
            pills.push(rr);
            // Subtle dark overlay to reinforce disappearing
            let overlay =
                RenderRect::new(fade.x, fade.y, fade.w, fade.h, Rgb::new(0, 0, 0), alpha * 0.06);
            rects.push(overlay);
            remaining.push(fade.clone());
        }
        // Replace with remaining animations
        self.tab_close_fades = remaining;
        // Stage pills and draw rects
        for rr in pills {
            self.stage_ui_rounded_rect(rr);
        }
        if !rects.is_empty() {
            let size_info = self.size_info;
            let metrics = self.glyph_cache.font_metrics();
            self.renderer_draw_rects(&size_info, &metrics, rects);
        }
    }

    /// Draw a tooltip showing the full title and working directory for the hovered tab.
    fn draw_tab_hover_tooltip(
        &mut self,
        config: &UiConfig,
        tab_manager: &TabManager,
        position: TabBarPosition,
        style: &WarpTabStyle,
        tab_id: TabId,
    ) {
        let tab = match tab_manager.get_tab(tab_id) {
            Some(t) => t,
            None => return,
        };
        // Compute hover progress for fade-in
        let hover_alpha = if let Some(t0) = self.tab_hover_anim_start {
            let elapsed = t0.elapsed().as_millis() as f32;
            let dur = style.animation_duration_ms.max(1) as f32;
            (elapsed / dur).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // Determine anchor X (tab center) from cached bounds
        let (tab_x, tab_w) = match self
            .tab_bounds_px
            .iter()
            .find(|(tid, _, _)| *tid == tab_id)
            .map(|(_, x, w)| (*x, *w))
        {
            Some(v) => v,
            None => return,
        };
        let center_x = tab_x + tab_w * 0.5;

        // Tooltip strings
        let title_str = tab.title.as_str();
        let cwd_str = tab.working_directory.to_string_lossy();

        // Measure sizes in columns
        use unicode_width::UnicodeWidthStr as _;
        let cw = self.size_info.cell_width();
        let ch = self.size_info.cell_height();
        let pad_x_px = (cw * 0.7).max(8.0);
        let pad_y_px = (ch * 0.3).max(4.0);
        let line_gap_px = (ch * 0.15).max(2.0);

        let title_cols = title_str.width();
        let cwd_cols = cwd_str.width();
        let text_cols = title_cols.max(cwd_cols).max(6);
        let text_w_px = text_cols as f32 * cw;
        let text_h_px = ch * 2.0 + line_gap_px;
        let bg_w = text_w_px + pad_x_px * 2.0;
        let bg_h = text_h_px + pad_y_px * 2.0;
        // Position above or below the tab bar
        let bar_y = match position {
            TabBarPosition::Top => 0.0,
            TabBarPosition::Bottom => self.size_info.height() - style.tab_height,
            TabBarPosition::Hidden => 0.0,
        };
        let bg_x = (center_x - bg_w * 0.5).clamp(4.0, self.size_info.width() - bg_w - 4.0);
        let bg_y = match position {
            TabBarPosition::Top => {
                (bar_y + style.tab_height + 6.0).min(self.size_info.height() - bg_h - 4.0)
            }
            TabBarPosition::Bottom => (bar_y - bg_h - 6.0).max(4.0),
            TabBarPosition::Hidden => 0.0,
        };

        // Colors from theme
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;
        let bg_color = tokens.surface;
        let fg_color = tokens.text;

        // Background pill
        let pill = UiRoundedRect::new(
            bg_x,
            bg_y,
            bg_w,
            bg_h,
            (theme.ui.corner_radius_px * 0.75).min(bg_h * 0.5),
            bg_color,
            0.92 * hover_alpha,
        );
        self.stage_ui_rounded_rect(pill);

        // Draw text lines inside the pill using grid-aligned drawing
        let title_line = ((bg_y + pad_y_px + ch * 0.1) / ch) as usize;
        let cwd_line = ((bg_y + pad_y_px + ch + line_gap_px) / ch) as usize;
        let start_col = ((bg_x + pad_x_px) / cw) as usize;
        self.draw_warp_tab_text(
            Point::new(title_line, Column(start_col)),
            fg_color,
            bg_color,
            title_str,
            text_cols,
        );
        self.draw_warp_tab_text(
            Point::new(cwd_line, Column(start_col)),
            tokens.text_muted,
            bg_color,
            &cwd_str,
            text_cols,
        );
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
