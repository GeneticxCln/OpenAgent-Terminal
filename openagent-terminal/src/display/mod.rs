#![allow(unexpected_cfgs)]
//! The display subsystem including window management, font rasterization, and
//! GPU drawing.

use std::fmt::{self, Formatter};
use std::num::NonZeroU32;
use std::time::{Duration, Instant};
use std::{cmp, mem};

use log::{debug, info};
use parking_lot::MutexGuard;
use serde::{Deserialize, Serialize};
use winit::dpi::PhysicalSize;
use winit::keyboard::ModifiersState;
use winit::raw_window_handle::RawWindowHandle;
use winit::window::CursorIcon;

use crossfont::{Metrics, Rasterize, Rasterizer, Size as FontSize};
use unicode_width::UnicodeWidthChar;

use openagent_terminal_core::event::{EventListener, OnResize, WindowSize};
use openagent_terminal_core::grid::Dimensions as TermDimensions;
use openagent_terminal_core::index::{Column, Direction, Line, Point};
use openagent_terminal_core::selection::Selection;
use openagent_terminal_core::term::cell::Flags;
use openagent_terminal_core::term::{
    self, LineDamageBounds, TermDamage, TermMode, MIN_COLUMNS, MIN_SCREEN_LINES,
};
use openagent_terminal_core::vte::ansi::{CursorShape, NamedColor};
use openagent_terminal_core::{self, Term};

use crate::config::debug::SubpixelOrientation;
use crate::config::font::Font;
use crate::config::window::Dimensions;
#[cfg(not(windows))]
use crate::config::window::StartupMode;
use crate::config::UiConfig;
use crate::display::bell::VisualBell;
use crate::display::color::{List, Rgb};
use crate::display::content::{RenderableCell, RenderableContent, RenderableCursor};
use crate::display::cursor::IntoRects;
use crate::display::damage::{damage_y_to_viewport_y, DamageTracker};
use crate::display::hint::{HintMatch, HintState};
use crate::display::meter::Meter;
use crate::display::window::Window;
use crate::event::{Event, EventType, Mouse, SearchState};
use crate::message_bar::{MessageBuffer, MessageType};
use crate::renderer::rects::{RenderLine, RenderLines, RenderRect};
use crate::renderer::ui::{UiRoundedRect, UiSprite};
use crate::renderer::{self, GlyphCache, LoaderApi};
use crate::scheduler::{Scheduler, TimerId, Topic};
use crate::string::{ShortenDirection, StrShortener};

#[cfg(feature = "ai")]
pub mod ai_drawing;
#[cfg(feature = "ai")]
pub mod ai_panel;
pub mod animation;
pub mod blocks;
#[cfg(feature = "blocks")]
pub mod blocks_search_actions;
pub mod blocks_search_panel;
pub mod color;
#[cfg(feature = "completions")]
pub mod completions;
pub mod confirm_overlay;
pub mod content;
pub mod cursor;
#[cfg(feature = "dap")]
pub mod dap_overlay;
#[cfg(feature = "editor")]
pub mod editor_overlay;
pub mod hint;
#[cfg(feature = "blocks")]
pub mod notebook_panel;
pub mod palette;
pub mod pane_drag_drop;
pub mod settings_panel;
pub mod tab_bar;
pub mod warp_ui;
pub mod window;
#[cfg(feature = "workflow")]
pub mod workflow_panel;
pub mod workspace_animations;

/// Decide whether the overlay tab bar should be shown based on configuration and mouse position.
///
/// Behavior:
/// - When visibility is Always, always show.
/// - When visibility is Hover, show only when the cursor is within the tab bar band near the
///   configured edge (Top/Bottom). A small tolerance is applied to ease acquisition.
/// - When visibility is Auto, behave like Always unless the window is fullscreen; in fullscreen,
///   behave like Hover.
#[inline]
pub(crate) fn should_show_tab_bar_overlay(
    size_info: SizeInfo,
    last_mouse_y_px: usize,
    tab_cfg: &crate::config::workspace::TabBarConfig,
    is_fullscreen: bool,
    style: &crate::display::warp_ui::WarpTabStyle,
) -> bool {
    use crate::config::workspace::TabBarVisibility as Vis;

    // Map Auto to Always/Hover based on fullscreen state.
    let vis = match tab_cfg.visibility {
        Vis::Always => Vis::Always,
        Vis::Hover => Vis::Hover,
        Vis::Auto => {
            if is_fullscreen {
                Vis::Hover
            } else {
                Vis::Always
            }
        }
    };

    match vis {
        Vis::Always => true,
        Vis::Hover => {
            // Reveal when the cursor is near the bar edge.
            let h = size_info.height();
            let y = last_mouse_y_px as f32;
            let band = (style.tab_height * 1.25).clamp(8.0, 64.0);
            match tab_cfg.position {
                crate::workspace::TabBarPosition::Top => y < band,
                crate::workspace::TabBarPosition::Bottom => y > (h - band),
                crate::workspace::TabBarPosition::Hidden => false,
            }
        }
        Vis::Auto => unreachable!(),
    }
}

mod bell;
mod damage;
mod meter;

/// Hover target kinds for the tab bar
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabHoverTarget {
    Tab(crate::workspace::TabId),
    Close(crate::workspace::TabId),
    Create,
}

/// State for active tab drag operations
#[derive(Debug, Clone)]
pub struct TabDragState {
    /// The tab being dragged
    pub tab_id: crate::workspace::TabId,
    /// Original position of the tab
    pub original_position: usize,
    /// Current position during drag
    pub current_position: usize,
    /// Target position for drop
    pub target_position: Option<usize>,
    /// Mouse position when drag started
    pub start_mouse_x: usize,
    pub start_mouse_y: usize,
    /// Current mouse position
    pub current_mouse_x: usize,
    pub current_mouse_y: usize,
    /// Visual offset for drag preview
    #[allow(dead_code)]
    pub visual_offset_x: f32,
    #[allow(dead_code)]
    pub visual_offset_y: f32,
    /// Whether the drag is currently active
    pub is_active: bool,
    /// Minimum distance needed to start drag (to distinguish from clicks)
    pub drag_threshold: f32,
}

/// Label for the forward terminal search bar.
const FORWARD_SEARCH_LABEL: &str = "Search: ";

/// Label for the backward terminal search bar.
const BACKWARD_SEARCH_LABEL: &str = "Backward Search: ";

/// The character used to shorten the visible text like uri preview or search regex.
const SHORTENER: char = '…';

/// Color which is used to highlight damaged rects when debugging.
const DAMAGE_RECT_COLOR: Rgb = Rgb::new(255, 0, 255);

#[derive(Debug)]
pub enum Error {
    /// Error with window management.
    Window(window::Error),

    /// Error dealing with fonts.
    Font(crossfont::Error),

    /// Error in renderer.
    Render(renderer::Error),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::Window(err) => err.fmt(f),
            Error::Font(err) => err.fmt(f),
            Error::Render(err) => err.fmt(f),
        }
    }
}

impl From<window::Error> for Error {
    fn from(val: window::Error) -> Self {
        Error::Window(val)
    }
}

impl From<crossfont::Error> for Error {
    fn from(val: crossfont::Error) -> Self {
        Error::Font(val)
    }
}

impl From<renderer::Error> for Error {
    fn from(val: renderer::Error) -> Self {
        Error::Render(val)
    }
}

/// Terminal size info.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
pub struct SizeInfo<T = f32> {
    /// Terminal window width.
    width: T,

    /// Terminal window height.
    height: T,

    /// Width of individual cell.
    cell_width: T,

    /// Height of individual cell.
    cell_height: T,

    /// Horizontal window padding.
    padding_x: T,

    /// Vertical window padding.
    padding_y: T,

    /// Number of lines in the viewport.
    screen_lines: usize,

    /// Number of columns in the viewport.
    columns: usize,
}

impl From<SizeInfo<f32>> for SizeInfo<u32> {
    fn from(size_info: SizeInfo<f32>) -> Self {
        Self {
            width: size_info.width as u32,
            height: size_info.height as u32,
            cell_width: size_info.cell_width as u32,
            cell_height: size_info.cell_height as u32,
            padding_x: size_info.padding_x as u32,
            padding_y: size_info.padding_y as u32,
            screen_lines: size_info.screen_lines,
            columns: size_info.columns,
        }
    }
}

impl From<SizeInfo<f32>> for WindowSize {
    fn from(size_info: SizeInfo<f32>) -> Self {
        Self {
            num_cols: size_info.columns() as u16,
            num_lines: size_info.screen_lines() as u16,
            cell_width: size_info.cell_width() as u16,
            cell_height: size_info.cell_height() as u16,
        }
    }
}

impl<T: Clone + Copy> SizeInfo<T> {
    #[inline]
    pub fn width(&self) -> T {
        self.width
    }

    #[inline]
    pub fn height(&self) -> T {
        self.height
    }

    #[inline]
    pub fn cell_width(&self) -> T {
        self.cell_width
    }

    #[inline]
    pub fn cell_height(&self) -> T {
        self.cell_height
    }

    #[inline]
    pub fn padding_x(&self) -> T {
        self.padding_x
    }

    #[inline]
    pub fn padding_y(&self) -> T {
        self.padding_y
    }
}

impl SizeInfo<f32> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        width: f32,
        height: f32,
        cell_width: f32,
        cell_height: f32,
        mut padding_x: f32,
        mut padding_y: f32,
        dynamic_padding: bool,
    ) -> SizeInfo {
        if dynamic_padding {
            padding_x = Self::dynamic_padding(padding_x.floor(), width, cell_width);
            padding_y = Self::dynamic_padding(padding_y.floor(), height, cell_height);
        }

        let lines = (height - 2. * padding_y) / cell_height;
        let screen_lines = cmp::max(lines as usize, MIN_SCREEN_LINES);

        let columns = (width - 2. * padding_x) / cell_width;
        let columns = cmp::max(columns as usize, MIN_COLUMNS);

        SizeInfo {
            width,
            height,
            cell_width,
            cell_height,
            padding_x: padding_x.floor(),
            padding_y: padding_y.floor(),
            screen_lines,
            columns,
        }
    }

    #[inline]
    pub fn reserve_lines(&mut self, count: usize) {
        self.screen_lines = cmp::max(self.screen_lines.saturating_sub(count), MIN_SCREEN_LINES);
    }

    /// Check if coordinates are inside the terminal grid.
    ///
    /// The padding, message bar or search are not counted as part of the grid.
    #[inline]
    pub fn contains_point(&self, x: usize, y: usize) -> bool {
        x <= (self.padding_x + self.columns as f32 * self.cell_width) as usize
            && x > self.padding_x as usize
            && y <= (self.padding_y + self.screen_lines as f32 * self.cell_height) as usize
            && y > self.padding_y as usize
    }

    /// Calculate padding to spread it evenly around the terminal content.
    #[inline]
    fn dynamic_padding(padding: f32, dimension: f32, cell_dimension: f32) -> f32 {
        padding + ((dimension - 2. * padding) % cell_dimension) / 2.
    }
}

impl TermDimensions for SizeInfo {
    #[inline]
    fn columns(&self) -> usize {
        self.columns
    }

    #[inline]
    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    #[inline]
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct DisplayUpdate {
    pub dirty: bool,

    dimensions: Option<PhysicalSize<u32>>,
    cursor_dirty: bool,
    font: Option<Font>,
}

impl DisplayUpdate {
    pub fn dimensions(&self) -> Option<PhysicalSize<u32>> {
        self.dimensions
    }

    pub fn font(&self) -> Option<&Font> {
        self.font.as_ref()
    }

    pub fn cursor_dirty(&self) -> bool {
        self.cursor_dirty
    }

    pub fn set_dimensions(&mut self, dimensions: PhysicalSize<u32>) {
        self.dimensions = Some(dimensions);
        self.dirty = true;
    }

    pub fn set_font(&mut self, font: Font) {
        self.font = Some(font);
        self.dirty = true;
    }

    pub fn set_cursor_dirty(&mut self) {
        self.cursor_dirty = true;
        self.dirty = true;
    }
}

/// The display wraps a window, font rasterizer, and GPU renderer.
pub struct Display {
    pub window: Window,

    pub size_info: SizeInfo,

    /// Clean startup mode: suppress overlays until first terminal content is visible
    pub startup_clean_mode: bool,
    /// Set once non-empty terminal content is observed (end of clean-startup phase)
    pub startup_nonempty_seen: bool,

    // Debug overlay to visualize split panes (horizontal/vertical) before full pane
    // implementation. None = off; Some(false) = horizontal split (left/right); Some(true) =
    // vertical split (top/bottom).
    pub debug_split_overlay: Option<bool>,

    #[cfg(feature = "ai")]
    /// Tracks last-known AI panel visibility to trigger open/close animations.
    pub(crate) ai_panel_last_active: bool,
    #[cfg(feature = "ai")]
    /// Animation start time for AI panel transitions.
    pub(crate) ai_panel_anim_start: Option<Instant>,
    #[cfg(feature = "ai")]
    /// True when animating opening, false when closing.
    pub(crate) ai_panel_anim_opening: bool,
    #[cfg(feature = "ai")]
    /// Animation duration in milliseconds.
    pub(crate) ai_panel_anim_duration_ms: u32,
    #[cfg(feature = "ai")]
    /// Hovered AI header control (for hover tooltips and cursor), if any.
    pub(crate) ai_hover_control: Option<ai_panel::AiHeaderControl>,

    /// Hint highlighted by the mouse.
    pub highlighted_hint: Option<HintMatch>,
    /// Frames since hint highlight was created.
    highlighted_hint_age: usize,

    /// Hint highlighted by the vi mode cursor.
    pub vi_highlighted_hint: Option<HintMatch>,
    /// Frames since hint highlight was created.
    vi_highlighted_hint_age: usize,

    pub raw_window_handle: RawWindowHandle,

    /// UI cursor visibility for blinking.
    pub cursor_hidden: bool,

    pub visual_bell: VisualBell,

    /// Mapped RGB values for each terminal color.
    pub colors: List,

    /// State of the keyboard hints.
    pub hint_state: HintState,

    /// Unprocessed display updates.
    pub pending_update: DisplayUpdate,

    /// The renderer update that takes place only once before the actual rendering.
    pub pending_renderer_update: Option<RendererUpdate>,

    /// Focus state for Warp-like bottom composer.
    pub composer_focused: bool,
    pub composer_text: String,
    pub composer_cursor: usize,
    /// Optional selection anchor for the composer (cursor is active end). None => no selection
    pub composer_sel_anchor: Option<usize>,
    /// Horizontal scroll offset in character columns for the composer text view
    pub composer_view_col_offset: usize,
    /// Caret blink state for composer
    pub composer_caret_visible: bool,
    pub composer_caret_last_toggle: Option<Instant>,
    /// Composer history (most-recent-first) and navigation index
    pub composer_history: std::collections::VecDeque<String>,
    pub composer_history_index: Option<usize>,
    pub composer_history_stash: Option<String>,

    /// Inline provider/model state for bottom composer UI
    pub ai_current_provider: String,
    pub ai_current_model: String,
    pub ai_provider_dropdown_open: bool,

    /// The ime on the given display.
    pub ime: Ime,

    /// The state of the timer for frame scheduling.
    pub frame_timer: FrameTimer,

    /// Damage tracker for the given display.
    pub damage_tracker: DamageTracker,

    /// Font size used by the window.
    pub font_size: FontSize,

    // Mouse point position when highlighting hints.
    hint_mouse_point: Option<Point>,

    /// Last mouse position in window pixels (for hover-only tab bar, near-edge detection)
    pub last_mouse_x: usize,
    pub last_mouse_y: usize,

    backend: Backend,

    glyph_cache: GlyphCache,
    meter: Meter,

    /// Command block tracking state.
    pub blocks: blocks::Blocks,

    /// Blocks Search panel UI state.
    #[cfg(feature = "blocks")]
    pub blocks_search: blocks_search_panel::BlocksSearchState,

    /// Confirmation overlay state.
    pub confirm_overlay: confirm_overlay::ConfirmOverlayState,

    /// Command Palette state.
    pub palette: palette::PaletteState,

    // Active notebook edit session (temporary file based), if any
    #[cfg(feature = "blocks")]
    #[allow(dead_code)]
    pub notebooks_edit_session: Option<crate::display::notebook_panel::NotebookEditSession>,

    /// Native code editor overlay state (feature="editor").
    #[cfg(feature = "editor")]
    pub editor_overlay: editor_overlay::EditorOverlayState,

    /// DAP overlay state (feature="dap").
    #[cfg(feature = "dap")]
    pub dap_overlay: dap_overlay::DapOverlayState,

    /// Always-on completions state (experimental).
    #[cfg(feature = "completions")]
    pub completions: completions::CompletionsState,

    /// Workflows panel state.
    #[cfg(feature = "workflow")]
    pub workflows_panel: workflow_panel::WorkflowsPanelState,
    /// Workflows progress overlay state.
    #[cfg(feature = "workflow")]
    pub workflows_progress: workflow_panel::WorkflowProgressState,
    /// Workflows parameter form overlay state.
    #[cfg(feature = "workflow")]
    pub workflows_params: workflow_panel::WorkflowParamsState,
    /// Notebooks panel state (feature="blocks")
    #[cfg(feature = "blocks")]
    pub notebooks_panel: notebook_panel::NotebookPanelState,

    /// Settings panel state (for in-app configuration like API keys)
    pub settings_panel: settings_panel::SettingsPanelState,

    /// Short press flash effect for Quick Actions capsules
    pub quick_actions_press_flash_until: Option<Instant>,
    /// Short press flash effect for Composer capsules
    pub composer_press_flash_until: Option<Instant>,

    /// Hover state for tab bar interactions
    pub tab_hover: Option<TabHoverTarget>,
    /// Animation start for tab hover transitions
    pub tab_hover_anim_start: Option<Instant>,
    /// Last active tab id for switch animation tracking
    pub tab_last_active_id: Option<crate::workspace::TabId>,
    /// Animation start for tab switch indicator
    pub tab_anim_switch_start: Option<Instant>,

    /// Tab drag-and-drop state
    pub tab_drag_active: Option<TabDragState>,
    /// Animation start for drag operations
    pub tab_drag_anim_start: Option<Instant>,
    /// List of active tab animations (opening, closing, moving)
    #[allow(dead_code)]
    pub tab_animations: Vec<tab_bar::TabAnimation>,
    /// Cached tab bounds in pixels for precise hit testing and drop targeting
    pub tab_bounds_px: Vec<(crate::workspace::TabId, f32, f32)>,
    /// Cached per-tab close button bounds in pixels (tab_id, x, y, w, h)
    pub close_button_bounds_px: Vec<(crate::workspace::TabId, f32, f32, f32, f32)>,
    /// Cached "+" new-tab button bounds in pixels (x, y, w, h)
    pub new_tab_button_bounds: Option<(f32, f32, f32, f32)>,
    /// Workspace animation manager for smooth UI transitions
    #[allow(dead_code)]
    pub workspace_animations: workspace_animations::WorkspaceAnimationManager,
    /// Pane drag-and-drop manager
    #[allow(dead_code)]
    pub pane_drag_manager: pane_drag_drop::PaneDragManager,

    /// Hovered split divider (if any)
    pub split_hover: Option<crate::workspace::split_manager::SplitDividerHit>,
    /// Active split drag (if any)
    pub split_drag: Option<crate::workspace::split_manager::SplitDividerHit>,
    /// Animation start for split hover transitions
    pub split_hover_anim_start: Option<Instant>,

    /// Blocks header hover line (viewport line index) for showing action chips
    pub blocks_header_hover_line: Option<usize>,
    /// Short press flash effect for block header chips
    pub blocks_press_flash_until: Option<Instant>,
    /// Hovered chip index in the blocks header, if any
    pub blocks_header_hover_chip: Option<usize>,
    /// Last pressed chip index (for flash)
    pub blocks_press_flash_chip: Option<usize>,

    /// Palette animation state
    #[allow(dead_code)]
    pub(crate) palette_last_active: bool,
    pub(crate) palette_anim_start: Option<Instant>,
    pub(crate) palette_anim_opening: bool,
    pub(crate) palette_anim_duration_ms: u32,
    /// Palette selection animation state
    pub(crate) palette_sel_last_index: Option<usize>,
    pub(crate) palette_sel_anim_start: Option<Instant>,
}

enum Backend {
    Wgpu {
        renderer: crate::renderer::wgpu::WgpuRenderer,
    },
}

impl Display {
    /// Returns true if the active backend is WGPU.
    #[allow(dead_code)]
    pub fn is_wgpu_backend(&self) -> bool {
        true
    }

    /// Return true when clean-startup suppression of overlays should be active.
    #[inline]
    pub fn clean_startup_active(&self) -> bool {
        // Environment override takes precedence if provided.
        if let Ok(v) = std::env::var("OPENAGENT_CLEAN_STARTUP") {
            if v.eq_ignore_ascii_case("0") || v.eq_ignore_ascii_case("false") {
                return false;
            }
            if v == "1" || v.eq_ignore_ascii_case("true") {
                return true;
            }
        }
        self.startup_clean_mode && !self.startup_nonempty_seen
    }

    /// Update all workspace animations and return whether any updates occurred
    #[allow(dead_code)]
    pub fn update_workspace_animations(&mut self) -> bool {
        let mut needs_redraw = false;

        // Update workspace-level animations (tabs, etc.)
        if self.workspace_animations.update_animations() {
            needs_redraw = true;
        }

        // Update pane drag animations
        if self.pane_drag_manager.update_animations() {
            needs_redraw = true;
        }

        // Legacy tab animation updates (will be migrated to workspace_animations)
        let now = Instant::now();
        let mut completed_animations = Vec::new();

        for (i, anim) in self.tab_animations.iter().enumerate() {
            let elapsed = now.duration_since(anim.start_time).as_millis() as u32;
            if elapsed >= anim.duration_ms {
                completed_animations.push(i);
            } else {
                needs_redraw = true;
            }
        }

        // Remove completed animations (in reverse order to maintain indices)
        for &i in completed_animations.iter().rev() {
            self.tab_animations.remove(i);
        }

        needs_redraw
    }

    /// Enable or disable reduced motion for accessibility
    #[allow(dead_code)]
    pub fn set_reduce_motion(&mut self, reduce_motion: bool) {
        self.workspace_animations.set_reduce_motion(reduce_motion);
        self.pane_drag_manager.set_reduce_motion(reduce_motion);
    }

    /// Draw a persistent Quick Actions bar on the bottom line with clickable entries
    /// for Workflows, Blocks, Palette, and optional AI. This improves discoverability
    /// for mouse-first users and reduces reliance on keybindings.
    /// Draw a persistent top toolbar (simple tab strip placeholder): [+] [Tab] [×]
    /// This is a visual affordance; actual multi-tab titles and close buttons
    /// will integrate with the workspace manager in a future pass.
    #[allow(dead_code)]
    #[cfg(feature = "preview_ui")]
    pub fn draw_top_toolbar(&mut self, config: &UiConfig) {
        let size_info = self.size_info;
        let cols = size_info.columns();
        let lines = size_info.screen_lines();
        if lines == 0 {
            return;
        }
        let theme = config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        let line = 0usize;
        let y = line as f32 * size_info.cell_height();
        let h = 1.0_f32 * size_info.cell_height();

        // Background band with subtle shadow underneath
        let bg = tokens.surface;
        let rects = vec![
            RenderRect::new(0.0, y, size_info.width(), h, bg, 0.98),
            // Shadow below
            RenderRect::new(
                0.0,
                y + h,
                size_info.width(),
                2.0,
                tokens.surface_muted,
                0.15,
            ),
        ];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        let fg = tokens.text;
        let accent = tokens.accent;

        // Labels: [+] [Tab] [×]
        let labels = ["[+]", "[Tab]", "[×]"];
        let mut col = 1usize;
        for (i, label) in labels.iter().enumerate() {
            let color = if i == 1 { accent } else { fg };
            self.draw_ai_text(
                Point::new(line, Column(col)),
                color,
                bg,
                label,
                cols.saturating_sub(col),
            );
            col += label.len() + 2;
            if col >= cols {
                break;
            }
        }
    }

    pub fn draw_quick_actions_bar(&mut self, config: &UiConfig) {
        let size_info = self.size_info;
        let cols = size_info.columns();
        let lines = size_info.screen_lines();
        if lines == 0 {
            return;
        }

        // Theme tokens
        let theme = config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Determine reserved rows for tab bar using effective visibility
        let tab_cfg = &config.workspace.tab_bar;
        let is_fs = self.window.is_fullscreen();
        // Effective mode for visibility: Auto -> Always unless fullscreen
        let _effective_visibility = match tab_cfg.visibility {
            crate::config::workspace::TabBarVisibility::Always => {
                crate::config::workspace::TabBarVisibility::Always
            }
            crate::config::workspace::TabBarVisibility::Hover => {
                crate::config::workspace::TabBarVisibility::Hover
            }
            crate::config::workspace::TabBarVisibility::Auto => {
                if is_fs {
                    crate::config::workspace::TabBarVisibility::Hover
                } else {
                    crate::config::workspace::TabBarVisibility::Always
                }
            }
        };
        // Warp-only layout: never reserve a grid row; overlay-only UI
        let reserve_top = false;
        let reserve_bottom = false;

        // Determine line based on quick actions position
        let mut line = match config.workspace.quick_actions.position {
            crate::config::workspace::QuickActionsPosition::Top => {
                if reserve_top {
                    1
                } else {
                    0
                }
            }
            crate::config::workspace::QuickActionsPosition::Bottom => {
                let base = lines.saturating_sub(1);
                if reserve_bottom {
                    base.saturating_sub(1)
                } else {
                    base
                }
            }
            crate::config::workspace::QuickActionsPosition::Auto => {
                let base = lines.saturating_sub(1);
                if reserve_bottom {
                    base.saturating_sub(1)
                } else {
                    base
                }
            }
        };
        if line >= lines {
            line = lines.saturating_sub(1);
        }
        let y = line as f32 * size_info.cell_height();
        let h = 1.0_f32 * size_info.cell_height();

        // Backdrop strip
        let bg = tokens.surface_muted;
        let rects = vec![RenderRect::new(
            0.0,
            y,
            size_info.width(),
            h,
            bg,
            theme.ui.quick_actions_band_alpha,
        )];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Labels
        let fg = tokens.text;

        // Build labels dynamically to respect configuration and features
        let mut labels: Vec<&str> = vec!["[Workflows]", "[Blocks]"];
        if config.workspace.quick_actions.show_palette {
            labels.push("[Palette]");
        }
        #[cfg(feature = "ai")]
        if config.ai.enabled {
            labels.push("[AI]");
        }

        // Compute hover state based on last mouse position
        let cw = size_info.cell_width();
        let px = self.last_mouse_x as f32;
        let py = self.last_mouse_y as f32;
        let inside_line = py >= y && py < y + h;
        let mut hover_col: Option<usize> = None;
        if inside_line {
            let col_from_px = ((px - size_info.padding_x()) / cw).floor() as isize;
            if col_from_px >= 0 {
                hover_col = Some(col_from_px as usize);
            }
        }
        let now = Instant::now();
        let press_flash = self
            .quick_actions_press_flash_until
            .is_some_and(|t| now < t);

        let mut col = 1usize;
        for label in labels.iter() {
            // For AI, dim if the build doesn't enable AI feature
            let _is_ai = *label == "[AI]";
            #[allow(unused_mut)]
            let mut color = fg;
            #[cfg(not(feature = "ai"))]
            if _is_ai {
                color = tokens.text_muted;
            }

            let label_cols = label.chars().count();
            let start_col = col;
            let end_col = col + label_cols;
            let is_hovered = hover_col.is_some_and(|c| c >= start_col && c < end_col);

            // Optional capsule background for Quick Actions labels
            if theme.ui.quick_actions_chip_capsules {
                let pad = theme
                    .ui
                    .quick_actions_chip_pad_px
                    .unwrap_or(theme.ui.palette_chip_pad_px)
                    .max(1.0);
                let pill_radius = theme
                    .ui
                    .quick_actions_pill_radius_px
                    .unwrap_or(theme.ui.palette_pill_radius_px);
                let x_px = (start_col as f32) * cw - pad;
                let w_px = (label_cols as f32) * cw + pad * 2.0;
                let y_px = y + (h - h * 0.8) * 0.5;
                let h_px = h * 0.8;
                let mut alpha = theme.ui.quick_actions_chip_alpha;
                if is_hovered {
                    alpha = (alpha + theme.ui.quick_actions_chip_alpha_hover_delta).min(1.0);
                }
                if press_flash && is_hovered {
                    alpha = (alpha + theme.ui.quick_actions_chip_alpha_press_delta).min(1.0);
                }
                let pill = UiRoundedRect::new(
                    x_px,
                    y_px,
                    w_px,
                    h_px,
                    pill_radius.min(h_px * 0.5),
                    tokens.surface,
                    alpha,
                );
                self.stage_ui_rounded_rect(pill);
                // Hovered text color accent
                if is_hovered {
                    color = tokens.accent;
                }
            } else if is_hovered {
                // No capsule; still accent the text color on hover
                color = tokens.accent;
            }

            self.draw_ai_text(
                Point::new(line, Column(start_col)),
                color,
                bg,
                label,
                cols.saturating_sub(start_col),
            );
            col = end_col + theme.ui.quick_actions_chip_gap_cols as usize;
            if col >= cols {
                break;
            }
        }

        // Settings quick access on the far right: gear sprite
        // Reserve a similar width region as the text label previously used
        let gear_cols = 3usize;
        if gear_cols + 2 < cols {
            let start_col = cols.saturating_sub(gear_cols + 2);
            let cw = size_info.cell_width();
            let ch = size_info.cell_height();
            let icon_px = theme
                .ui
                .quick_actions_settings_icon_px
                .unwrap_or((ch * 0.9).clamp(12.0, 18.0));
            let ix = (start_col as f32) * cw + (cw * gear_cols as f32 - icon_px) * 0.5;
            let iy = y + (h - icon_px) * 0.5;
            // Atlas slot 8 = gear
            let step = 1.0f32 / 9.0f32;
            let uv_x = 8.0 * step;
            let uv_y = 0.0f32;
            let uv_w = step;
            let uv_h = 1.0f32;
            let nearest = (icon_px - 16.0).abs() < 0.5;
            // Hover tint: accent when mouse over gear
            let mx_col_opt = hover_col;
            let gear_hover =
                mx_col_opt.is_some_and(|c| c >= start_col && c < start_col + gear_cols);
            let tint = if gear_hover { tokens.accent } else { fg };
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
        }
    }

    // Fallback helper: draw text using the renderer. This is available when the `blocks` feature is
    // disabled to satisfy calls from overlays/palette that rely on a common text drawing helper.
    // When `blocks` is enabled, a similar helper exists in the blocks_search_panel module.
    #[cfg(not(feature = "blocks"))]
    pub(crate) fn draw_ai_text(
        &mut self,
        point: Point<usize>,
        fg: Rgb,
        bg: Rgb,
        text: &str,
        max_width: usize,
    ) {
        use unicode_width::UnicodeWidthStr;
        let truncated_text: String = if text.width() > max_width {
            text.chars().take(max_width).collect()
        } else {
            text.to_string()
        };

        let size_info_copy = self.size_info;
        match &mut self.backend {
            Backend::Wgpu { renderer } => {
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

    #[cfg(feature = "wgpu")]
    pub fn new_wgpu(
        mut window: Window,
        config: &UiConfig,
        _tabbed: bool,
    ) -> Result<Display, Error> {
        let raw_window_handle = window.raw_window_handle();

        let scale_factor = window.scale_factor as f32;
        let rasterizer = Rasterizer::new()?;

        let font_size = config.font.size().scale(scale_factor);
        debug!("Loading \"{}\" font", &config.font.normal().family);
        let font = config.font.clone().with_size(font_size);
        let mut glyph_cache = GlyphCache::new(rasterizer, &font)?;

        let metrics = glyph_cache.font_metrics();
        let (cell_width, cell_height) = compute_cell_size(config, &metrics);

        // Resize the window to account for the user configured size.
        if let Some(dimensions) = config.window.dimensions() {
            let size = window_size(config, dimensions, cell_width, cell_height, scale_factor);
            window.request_inner_size(size);
        }

        // WGPU renderer init.
        let mut wgpu_renderer = pollster::block_on(crate::renderer::wgpu::WgpuRenderer::new(
            window.winit_window(),
            window.inner_size(),
            config.debug.renderer,
            config.debug.srgb_swapchain,
            config.debug.subpixel_text,
            config.debug.subpixel_orientation,
            config.debug.zero_evicted_atlas_layer,
            config.debug.atlas_eviction_policy,
            config.debug.atlas_report_interval_frames,
            config.debug.renderer_report_interval_frames,
        ))
        .map_err(|e| Error::Render(renderer::Error::Other(format!("wgpu init failed: {:?}", e))))?;

        // Apply initial debug runtime options to renderer.
        if (config.debug.subpixel_gamma - 2.2).abs() > f32::EPSILON {
            wgpu_renderer.set_subpixel_gamma(config.debug.subpixel_gamma);
        }
        if config.debug.renderer_perf_hud {
            wgpu_renderer.toggle_perf_hud();
        }

        // Load font common glyphs to accelerate rendering using the WGPU atlas.
        debug!("Filling glyph cache with common glyphs (wgpu)");
        wgpu_renderer.preload_glyphs(&mut glyph_cache);

        let padding = config.window.padding(window.scale_factor as f32);
        let viewport_size = window.inner_size();

        // Create new size with at least one column and row.
        let size_info = SizeInfo::new(
            viewport_size.width as f32,
            viewport_size.height as f32,
            cell_width,
            cell_height,
            padding.0,
            padding.1,
            config.window.dynamic_padding && config.window.dimensions().is_none(),
        );

        info!("Cell size: {cell_width} x {cell_height}");
        info!(
            "Padding: {} x {}",
            size_info.padding_x(),
            size_info.padding_y()
        );
        info!(
            "Width: {}, Height: {}",
            size_info.width(),
            size_info.height()
        );

        // Clear screen.
        let background_color = config.colors.primary.background;
        wgpu_renderer.clear(background_color, config.window_opacity());

        // Disable shadows for transparent windows on macOS.
        #[cfg(target_os = "macos")]
        window.set_has_shadow(config.window_opacity() >= 1.0);

        // Set resize increments for the newly created window.
        if config.window.resize_increments {
            window.set_resize_increments(PhysicalSize::new(cell_width, cell_height));
        }

        window.set_visible(true);

        // Some compositors (Wayland) won't show a window until the first draw commits.
        // Request an immediate redraw so the first frame is presented.
        window.request_redraw();

        #[cfg(target_os = "macos")]
        window.focus_window();

        let is_wayland = matches!(raw_window_handle, RawWindowHandle::Wayland(_));
        #[allow(clippy::single_match)]
        #[cfg(not(windows))]
        if !_tabbed {
            match config.window.startup_mode {
                #[cfg(target_os = "macos")]
                StartupMode::SimpleFullscreen => window.set_simple_fullscreen(true),
                StartupMode::Maximized if !is_wayland => window.set_maximized(true),
                _ => (),
            }
        }

        let hint_state = HintState::new(config.hints.alphabet());
        let mut damage_tracker = DamageTracker::new(size_info.screen_lines(), size_info.columns());
        damage_tracker.debug = config.debug.highlight_damage;

        // Initialize command blocks manager.
        let mut blocks = blocks::Blocks::new();
        blocks.enabled = config.debug.blocks;

        Ok(Self {
            backend: Backend::Wgpu {
                renderer: wgpu_renderer,
            },
            visual_bell: VisualBell::from(&config.bell),
            colors: List::from(&config.colors),
            frame_timer: FrameTimer::new(),
            raw_window_handle,
            damage_tracker,
            glyph_cache,
            hint_state,
            size_info,
            font_size,
            window,
            // Clean startup tracking
            startup_clean_mode: config.workspace.clean_startup,
            startup_nonempty_seen: false,
            #[cfg(feature = "blocks")]
            notebooks_panel: notebook_panel::NotebookPanelState::new(),
            pending_renderer_update: Default::default(),
            composer_focused: false,
            composer_press_flash_until: None,
            composer_text: String::new(),
            composer_cursor: 0,
            composer_sel_anchor: None,
            composer_view_col_offset: 0,
            composer_caret_visible: true,
            composer_caret_last_toggle: None,
            composer_history: std::collections::VecDeque::new(),
            composer_history_index: None,
            composer_history_stash: None,
            ai_current_provider: {
                #[cfg(feature = "ai")]
                {
                    config
                        .ai
                        .provider
                        .as_deref()
                        .unwrap_or("openrouter")
                        .to_string()
                }
                #[cfg(not(feature = "ai"))]
                {
                    "null".to_string()
                }
            },
            ai_current_model: {
                #[cfg(feature = "ai")]
                {
                    let pname = config.ai.provider.as_deref().unwrap_or("openrouter");
                    let prov_cfg = config
                        .ai
                        .providers
                        .get(pname)
                        .cloned()
                        .or_else(|| {
                            crate::config::ai_providers::get_default_provider_configs()
                                .get(pname)
                                .cloned()
                        })
                        .unwrap_or_default();
                    prov_cfg.default_model.unwrap_or_else(|| String::new())
                }
                #[cfg(not(feature = "ai"))]
                {
                    String::new()
                }
            },
            ai_provider_dropdown_open: false,
            vi_highlighted_hint_age: Default::default(),
            highlighted_hint_age: Default::default(),
            vi_highlighted_hint: Default::default(),
            highlighted_hint: Default::default(),
            hint_mouse_point: Default::default(),
            pending_update: Default::default(),
            cursor_hidden: Default::default(),
            meter: Default::default(),
            ime: Default::default(),
            blocks,
            #[cfg(feature = "blocks")]
            blocks_search: blocks_search_panel::BlocksSearchState::new(),
            #[cfg(feature = "completions")]
            completions: completions::CompletionsState::new(),
            debug_split_overlay: None,
            #[cfg(feature = "ai")]
            ai_panel_last_active: false,
            #[cfg(feature = "ai")]
            ai_panel_anim_start: None,
            #[cfg(feature = "ai")]
            ai_panel_anim_opening: false,
            #[cfg(feature = "ai")]
            ai_panel_anim_duration_ms: 0,
            #[cfg(feature = "ai")]
            ai_hover_control: None,
            #[cfg(feature = "workflow")]
            workflows_panel: workflow_panel::WorkflowsPanelState::new(),
            #[cfg(feature = "workflow")]
            workflows_progress: Default::default(),
            #[cfg(feature = "workflow")]
            workflows_params: Default::default(),
            settings_panel: settings_panel::SettingsPanelState::new(),
            quick_actions_press_flash_until: None,
            #[cfg(feature = "editor")]
            editor_overlay: editor_overlay::EditorOverlayState::new(),
            #[cfg(feature = "dap")]
            dap_overlay: dap_overlay::DapOverlayState::new(),
            // Palette animation init (wgpu)
            palette_last_active: false,
            palette_anim_start: None,
            palette_anim_opening: false,
            palette_anim_duration_ms: 0,
            palette_sel_last_index: None,
            palette_sel_anim_start: None,
            // Additional UI state defaults
            palette: {
                let mut p = palette::PaletteState::new();
                p.load_mru_from_config(config);
                p
            },
            #[cfg(feature = "blocks")]
            notebooks_edit_session: None,
            confirm_overlay: confirm_overlay::ConfirmOverlayState::new(),
            last_mouse_x: 0,
            last_mouse_y: 0,
            tab_hover: None,
            tab_hover_anim_start: None,
            tab_last_active_id: None,
            tab_anim_switch_start: None,
            tab_drag_active: None,
            tab_drag_anim_start: None,
            tab_animations: Vec::new(),
            workspace_animations: workspace_animations::WorkspaceAnimationManager::new(),
            pane_drag_manager: pane_drag_drop::PaneDragManager::new(),
            tab_bounds_px: Vec::new(),
            close_button_bounds_px: Vec::new(),
            new_tab_button_bounds: None,
            split_hover: None,
            split_drag: None,
            split_hover_anim_start: None,
            blocks_header_hover_line: None,
            blocks_press_flash_until: None,
            blocks_header_hover_chip: None,
            blocks_press_flash_chip: None,
        })
    }

    /// Update font size and cell dimensions.
    ///
    /// This will return a tuple of the cell width and height.
    fn update_font_size(
        glyph_cache: &mut GlyphCache,
        config: &UiConfig,
        font: &Font,
    ) -> (f32, f32) {
        let _ = glyph_cache.update_font_size(font);

        // Compute new cell sizes.
        compute_cell_size(config, &glyph_cache.font_metrics())
    }

    /// Reset glyph cache.
    fn reset_glyph_cache(&mut self) {
        match &mut self.backend {
            Backend::Wgpu { renderer } => {
                // Reset the WGPU atlas pages and reload common glyphs using the real loader.
                renderer.reset_atlas();
                renderer.preload_glyphs(&mut self.glyph_cache);
            }
        }
    }

    /// Draw overlay visuals for active pane drag (preview + drop zone).
    pub fn draw_pane_drag_overlay(
        &mut self,
        config: &UiConfig,
        active_tab: &crate::workspace::tab_manager::TabContext,
    ) {
        use crate::renderer::ui::UiRoundedRect;
        if let Some(effects) = self.pane_drag_manager.get_visual_effects() {
            let theme = config
                .resolved_theme
                .as_ref()
                .cloned()
                .unwrap_or_else(|| config.theme.resolve());
            let tokens = theme.tokens;

            // Compute content container rect (padding + reserved tab row)
            let si = self.size_info;
            let x0 = si.padding_x();
            let mut y0 = si.padding_y();
            let w = si.width() - 2.0 * si.padding_x();
            let mut h = si.height() - 2.0 * si.padding_y();
            let tab_cfg = &config.workspace.tab_bar;
            let is_fs = self.window.is_fullscreen();
            let effective_visibility = match tab_cfg.visibility {
                crate::config::workspace::TabBarVisibility::Always => {
                    crate::config::workspace::TabBarVisibility::Always
                }
                crate::config::workspace::TabBarVisibility::Hover => {
                    crate::config::workspace::TabBarVisibility::Hover
                }
                crate::config::workspace::TabBarVisibility::Auto => {
                    if is_fs {
                        crate::config::workspace::TabBarVisibility::Hover
                    } else {
                        crate::config::workspace::TabBarVisibility::Always
                    }
                }
            };
            // Suppress row reservation entirely when using warp overlay-only mode
            let overlay_only = config.workspace.warp_style && config.workspace.warp_overlay_only;
            if !overlay_only
                && tab_cfg.show
                && matches!(
                    effective_visibility,
                    crate::config::workspace::TabBarVisibility::Always
                )
                && tab_cfg.position != crate::workspace::TabBarPosition::Hidden
            {
                let cell_h = si.cell_height();
                match tab_cfg.position {
                    crate::workspace::TabBarPosition::Top => {
                        y0 += cell_h;
                        h = (h - cell_h).max(0.0);
                    }
                    crate::workspace::TabBarPosition::Bottom => {
                        h = (h - cell_h).max(0.0);
                    }
                    crate::workspace::TabBarPosition::Hidden => {}
                }
            }
            let container = crate::workspace::split_manager::PaneRect::new(x0, y0, w, h);

            // Compute pane rects to find the source pane rect and to draw ghost effect
            let rects = crate::workspace::SplitManager::calculate_pane_rects(
                &crate::workspace::SplitManager::new(),
                &active_tab.split_layout,
                container,
            );

            // Drop-zone highlight
            if let Some(dz) = effects.drop_zone {
                match dz {
                    crate::display::pane_drag_drop::PaneDropZone::Split {
                        direction,
                        target_split,
                        before,
                        ..
                    } => {
                        // Find target split rect
                        if let Some((_, target_rect)) =
                            rects.iter().find(|(pid, _)| Some(*pid) == target_split)
                        {
                            let (hx, hy, hw, hh) = match direction {
                                crate::display::pane_drag_drop::SplitDirection::Vertical => {
                                    // left/right region
                                    let half = target_rect.width * 0.5;
                                    if before {
                                        (target_rect.x, target_rect.y, half, target_rect.height)
                                    } else {
                                        (
                                            target_rect.x + half,
                                            target_rect.y,
                                            half,
                                            target_rect.height,
                                        )
                                    }
                                }
                                crate::display::pane_drag_drop::SplitDirection::Horizontal => {
                                    // top/bottom region
                                    let half = target_rect.height * 0.5;
                                    if before {
                                        (target_rect.x, target_rect.y, target_rect.width, half)
                                    } else {
                                        (
                                            target_rect.x,
                                            target_rect.y + half,
                                            target_rect.width,
                                            half,
                                        )
                                    }
                                }
                            };
                            // Use configurable highlight color / alpha with theme fallback
                            let dcfg = &config.workspace.drag;
                            let mut hl_color = dcfg.highlight_color.get().unwrap_or(tokens.accent);
                            let mut alpha_base = dcfg.highlight_alpha_base;
                            let mut alpha_hover = dcfg.highlight_alpha_hover.max(alpha_base);
                            // Light theme tweak: ensure minimum alpha and blend towards surface_muted
                            let is_light = {
                                let (r, g, b) = tokens.surface.as_tuple();
                                let luminance =
                                    0.2126 * (r as f32) + 0.7152 * (g as f32) + 0.0722 * (b as f32);
                                luminance > 140.0
                            };
                            let min_alpha = dcfg.highlight_min_alpha.clamp(0.0, 1.0);
                            if is_light {
                                alpha_base = alpha_base.max(min_alpha);
                                alpha_hover = alpha_hover.max((min_alpha + 0.05).min(1.0));
                                // Nudge fill color towards surface_muted for softer highlight on light bg
                                hl_color = (hl_color * 0.6) + (tokens.surface_muted * 0.4);
                            }
                            let alpha = (alpha_base
                                + (alpha_hover - alpha_base) * effects.drop_zone_highlight_alpha)
                                .clamp(0.0, 1.0);
                            let rect = RenderRect::new(hx, hy, hw, hh, hl_color, alpha);
                            let metrics = self.glyph_cache.font_metrics();
                            let size_copy = self.size_info;
                            self.renderer_draw_rects(&size_copy, &metrics, vec![rect]);
                        }
                    }
                    crate::display::pane_drag_drop::PaneDropZone::Tab { tab_id, .. } => {
                        // Highlight hovered tab region in the overlay for feedback
                        let style = crate::display::warp_ui::WarpTabStyle::from_theme(config);
                        // Find tab bounds
                        if let Some((_, x, w)) = self
                            .tab_bounds_px
                            .iter()
                            .copied()
                            .find(|(tid, _, _)| *tid == tab_id)
                        {
                            let bar_y = match config.workspace.tab_bar.position {
                                crate::workspace::TabBarPosition::Top => 0.0,
                                crate::workspace::TabBarPosition::Bottom => {
                                    self.size_info.height() - style.tab_height
                                }
                                crate::workspace::TabBarPosition::Hidden => 0.0,
                            };
                            let dcfg = &config.workspace.drag;
                            let mut hl_color = dcfg.highlight_color.get().unwrap_or(tokens.accent);
                            let mut alpha_base = dcfg.tab_highlight_alpha_base;
                            let mut alpha_hover = dcfg.tab_highlight_alpha_hover.max(alpha_base);
                            // Light theme tweak
                            let is_light = {
                                let (r, g, b) = tokens.surface.as_tuple();
                                let luminance =
                                    0.2126 * (r as f32) + 0.7152 * (g as f32) + 0.0722 * (b as f32);
                                luminance > 140.0
                            };
                            let min_alpha = dcfg.highlight_min_alpha.clamp(0.0, 1.0);
                            if is_light {
                                alpha_base = alpha_base.max(min_alpha);
                                alpha_hover = alpha_hover.max((min_alpha + 0.05).min(1.0));
                                hl_color = (hl_color * 0.6) + (tokens.surface_muted * 0.4);
                            }
                            let alpha = (alpha_base
                                + (alpha_hover - alpha_base) * effects.drop_zone_highlight_alpha)
                                .clamp(0.0, 1.0);
                            let rect =
                                RenderRect::new(x, bar_y, w, style.tab_height, hl_color, alpha);
                            let metrics = self.glyph_cache.font_metrics();
                            let size_copy = self.size_info;
                            self.renderer_draw_rects(&size_copy, &metrics, vec![rect]);
                        }
                    }
                    crate::display::pane_drag_drop::PaneDropZone::NewTab { .. } => {
                        // Highlight the new-tab area to the right of the last tab within the tab bar band
                        let style = crate::display::warp_ui::WarpTabStyle::from_theme(config);
                        let bar_y = match config.workspace.tab_bar.position {
                            crate::workspace::TabBarPosition::Top => 0.0,
                            crate::workspace::TabBarPosition::Bottom => {
                                self.size_info.height() - style.tab_height
                            }
                            crate::workspace::TabBarPosition::Hidden => 0.0,
                        };
                        let max_tab_x = self
                            .tab_bounds_px
                            .iter()
                            .map(|(_, x, w)| x + w)
                            .fold(0.0, f32::max);
                        let start_x = max_tab_x.min(self.size_info.width());
                        let w = (self.size_info.width() - start_x).max(0.0);
                        if w > 3.0 {
                            let dcfg = &config.workspace.drag;
                            let mut hl_color = dcfg.highlight_color.get().unwrap_or(tokens.accent);
                            let mut alpha_base = dcfg.new_tab_highlight_alpha_base;
                            let mut alpha_hover =
                                dcfg.new_tab_highlight_alpha_hover.max(alpha_base);
                            // Light theme tweak
                            let is_light = {
                                let (r, g, b) = tokens.surface.as_tuple();
                                let luminance =
                                    0.2126 * (r as f32) + 0.7152 * (g as f32) + 0.0722 * (b as f32);
                                luminance > 140.0
                            };
                            let min_alpha = dcfg.highlight_min_alpha.clamp(0.0, 1.0);
                            if is_light {
                                alpha_base = alpha_base.max(min_alpha);
                                alpha_hover = alpha_hover.max((min_alpha + 0.05).min(1.0));
                                hl_color = (hl_color * 0.6) + (tokens.surface_muted * 0.4);
                            }
                            let alpha = (alpha_base
                                + (alpha_hover - alpha_base) * effects.drop_zone_highlight_alpha)
                                .clamp(0.0, 1.0);
                            let rect = RenderRect::new(
                                start_x,
                                bar_y,
                                w,
                                style.tab_height,
                                hl_color,
                                alpha,
                            );
                            let metrics = self.glyph_cache.font_metrics();
                            let size_copy = self.size_info;
                            self.renderer_draw_rects(&size_copy, &metrics, vec![rect]);
                        }
                    }
                }
            }

            // Ghost the source pane area slightly (only once drag is actually active)
            if effects.is_active {
                if let Some((_, src_rect)) =
                    rects.iter().find(|(pid, _)| *pid == effects.source_split)
                {
                    let ghost = RenderRect::new(
                        src_rect.x,
                        src_rect.y,
                        src_rect.width,
                        src_rect.height,
                        tokens.overlay,
                        effects.ghost_pane_alpha.clamp(0.0, 1.0),
                    );
                    let metrics = self.glyph_cache.font_metrics();
                    let size_copy = self.size_info;
                    self.renderer_draw_rects(&size_copy, &metrics, vec![ghost]);
                }
            }

            // Drag preview rectangle around current cursor (scaled)
            if effects.is_active {
                let base_w = (self.size_info.cell_width() * 12.0).clamp(120.0, 260.0);
                let base_h = (self.size_info.cell_height() * 6.0).clamp(80.0, 200.0);
                let w = base_w * effects.drag_preview_scale.max(0.9);
                let h = base_h * effects.drag_preview_scale.max(0.9);
                let x = (effects.current_pos.0 - w * 0.5).clamp(0.0, self.size_info.width() - w);
                let y = (effects.current_pos.1 - h * 0.5).clamp(0.0, self.size_info.height() - h);
                let pill = UiRoundedRect::new(
                    x,
                    y,
                    w,
                    h,
                    (theme.ui.corner_radius_px * 0.75).min(h * 0.5),
                    tokens.surface,
                    effects.drag_preview_alpha.clamp(0.0, 1.0),
                );
                self.stage_ui_rounded_rect(pill);
            }
        }
    }

    // XXX: this function must not call any renderer tasks outside the scheduled update hooks.
    // performed in [`Self::process_renderer_update`] right before drawing.
    //
    /// Process update events.
    pub fn handle_update<T>(
        &mut self,
        terminal: &mut Term<T>,
        pty_resize_handle: &mut dyn OnResize,
        message_buffer: &MessageBuffer,
        search_state: &mut SearchState,
        config: &UiConfig,
    ) where
        T: EventListener,
    {
        let pending_update = mem::take(&mut self.pending_update);

        let (mut cell_width, mut cell_height) =
            (self.size_info.cell_width(), self.size_info.cell_height());

        if pending_update.font().is_some() || pending_update.cursor_dirty() {
            let renderer_update = self
                .pending_renderer_update
                .get_or_insert(Default::default());
            renderer_update.clear_font_cache = true
        }

        // Update font size and cell dimensions.
        if let Some(font) = pending_update.font() {
            let cell_dimensions = Self::update_font_size(&mut self.glyph_cache, config, font);
            cell_width = cell_dimensions.0;
            cell_height = cell_dimensions.1;

            info!("Cell size: {cell_width} x {cell_height}");

            // Mark entire terminal as damaged since glyph size could change without cell size
            // changes.
            self.damage_tracker.frame().mark_fully_damaged();
        }

        let (mut width, mut height) = (self.size_info.width(), self.size_info.height());
        if let Some(dimensions) = pending_update.dimensions() {
            width = dimensions.width as f32;
            height = dimensions.height as f32;
        }

        let padding = config.window.padding(self.window.scale_factor as f32);

        let mut new_size = SizeInfo::new(
            width,
            height,
            cell_width,
            cell_height,
            padding.0,
            padding.1,
            config.window.dynamic_padding,
        );

        // Update number of column/lines in the viewport.
        let search_active = search_state.history_index.is_some();
        let message_bar_lines = message_buffer
            .message()
            .map_or(0, |m| m.text(&new_size).len());
        let search_lines = usize::from(search_active);
        new_size.reserve_lines(message_bar_lines + search_lines);

        // Update resize increments.
        if config.window.resize_increments {
            self.window
                .set_resize_increments(PhysicalSize::new(cell_width, cell_height));
        }

        // Resize when terminal when its dimensions have changed.
        if self.size_info.screen_lines() != new_size.screen_lines
            || self.size_info.columns() != new_size.columns()
        {
            // Resize PTY.
            pty_resize_handle.on_resize(new_size.into());

            // Resize terminal.
            terminal.resize(new_size);

            // Resize damage tracking.
            self.damage_tracker
                .resize(new_size.screen_lines(), new_size.columns());
        }

        // Check if dimensions have changed.
        if new_size != self.size_info {
            // Queue renderer update.
            let renderer_update = self
                .pending_renderer_update
                .get_or_insert(Default::default());
            renderer_update.resize = true;

            // Clear focused search match.
            search_state.clear_focused_match();
        }
        self.size_info = new_size;
    }

    // NOTE: Renderer updates are split off, since platforms like Wayland require resize and other
    // Renderer operations to be performed right before rendering. Otherwise they could lock the
    // back buffer and render with the previous state. This also solves flickering during resizes.
    //
    /// Update the state of the renderer.
    pub fn process_renderer_update(&mut self) {
        // Merge any pending renderer update with runtime signals (like WGPU atlas eviction).
        #[allow(unused_mut)]
        let mut renderer_update = self.pending_renderer_update.take().unwrap_or_default();
        #[cfg(feature = "wgpu")]
        let Backend::Wgpu { renderer } = &mut self.backend;
        if renderer.take_atlas_evicted() {
            // Clear CPU glyph cache; then evict a single page in the WGPU renderer.
            renderer_update.clear_font_cache = true;
            if !renderer.evict_one_page() {
                // Fallback to full reset if no pending eviction was set.
                renderer.reset_atlas();
            }
        }

        // Resize renderer.
        if renderer_update.resize {
            let Backend::Wgpu { renderer } = &mut self.backend;
            renderer.resize(&self.size_info);
        }

        // Ensure we're modifying the correct backend.

        if renderer_update.clear_font_cache {
            self.reset_glyph_cache();
        }

        self.renderer_resize();

        info!(
            "Padding: {} x {}",
            self.size_info.padding_x(),
            self.size_info.padding_y()
        );
        info!(
            "Width: {}, Height: {}",
            self.size_info.width(),
            self.size_info.height()
        );
    }

    /// Draw performance HUD text using the text pipeline (WGPU backend only)
    fn draw_perf_hud_text(&mut self, config: &UiConfig) {
        let show = config.debug.renderer_perf_hud;
        let Backend::Wgpu { renderer } = &mut self.backend;
        let enabled = show || renderer.perf_hud_enabled();
        if !enabled {
            return;
        }
        let m = renderer.metrics();
        let last_ms = renderer.last_frame_ms();
        let fps = if last_ms > 0.0 { 1000.0 / last_ms } else { 0.0 };
        let stats = renderer.frame_ms_stats();
        let copy_kb = (m.rect_bytes_copied as f32) / 1024.0;

        // Optionally compute damage metrics for the HUD
        let mut damage_suffix = String::new();
        if config.debug.renderer_perf_hud_damage_metrics {
            let rects = self
                .damage_tracker
                .shape_frame_damage(self.size_info.into());
            let mut total_px: i64 = 0;
            for r in &rects {
                total_px += (r.width.max(0) as i64) * (r.height.max(0) as i64);
            }
            damage_suffix = format!(" | dmg_rects={} dmg_px={}", rects.len(), total_px);
        }

        let s = if let Some((avg, min, max)) = stats {
            format!(
                "{last_ms:.1} ms ({fps:.1} fps) | avg {avg:.1} min {min:.1} max {max:.1} | draws={} verts={} copy={copy_kb:.1}KB flush={} batch={}{}",
                m.draw_calls, m.vertices_submitted, m.rect_flush_count, m.primitives_batched, damage_suffix
            )
        } else {
            format!(
                "{last_ms:.1} ms ({fps:.1} fps) | draws={} verts={} copy={copy_kb:.1}KB flush={} batch={}{}",
                m.draw_calls, m.vertices_submitted, m.rect_flush_count, m.primitives_batched, damage_suffix
            )
        };

        // Theme-aware colors
        let theme = config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| config.theme.resolve());
        let (bg_color, fg_color, alpha) = match config.debug.render_timer_style {
            crate::config::debug::RenderTimerStyle::LowContrast => (
                theme.tokens.surface_muted,
                theme.tokens.text,
                0.65,
            ),
            crate::config::debug::RenderTimerStyle::Warning => (
                theme.tokens.warning,
                theme.tokens.surface,
                0.80,
            ),
        };

        // DPI-friendly sizing using cell metrics
        use unicode_width::UnicodeWidthStr;
        let cw = self.size_info.cell_width();
        let ch = self.size_info.cell_height();
        let pad_x = (cw * 0.5).max(6.0);
        let pad_y = (ch * 0.2).max(4.0);
        let text_cells = s.width();
        let bg_w = (text_cells as f32) * cw + pad_x * 2.0;
        let bg_h = ch + pad_y * 2.0;
        let bg_x = self.size_info.padding_x();
        let bg_y = self.size_info.padding_y();

        // Stage rounded background before drawing text
        let pill = crate::renderer::ui::UiRoundedRect::new(bg_x, bg_y, bg_w, bg_h, 6.0, bg_color, alpha);
        let size_info_copy = self.size_info;
        renderer.stage_ui_rounded_rect(&size_info_copy, pill);

        // Draw text at top-left inside the pill (start at first visible column)
        let point = Point::new(0, Column(0));
        renderer.draw_string(
            point,
            fg_color,
            bg_color,
            s.chars(),
            &size_info_copy,
            &mut self.glyph_cache,
        );
    }

    /// Dump atlas stats to the debug log.
    pub fn dump_atlas_stats(&mut self) {
        let Backend::Wgpu { renderer } = &mut self.backend;
        renderer.dump_atlas_stats();
    }

    /// Toggle subpixel text rendering (WGPU backend only).
    pub fn toggle_subpixel_text(&mut self) -> bool {
        let Backend::Wgpu { renderer } = &mut self.backend;
        let _now = renderer.toggle_subpixel_enabled();
        self.damage_tracker.frame().mark_fully_damaged();
        true
    }

    /// Cycle LCD subpixel orientation between RGB and BGR (WGPU backend only).
    pub fn cycle_subpixel_orientation(&mut self) -> Option<SubpixelOrientation> {
        let Backend::Wgpu { renderer } = &mut self.backend;
        let next = renderer.cycle_subpixel_orientation();
        self.damage_tracker.frame().mark_fully_damaged();
        Some(next)
    }

    /// Toggle performance HUD (WGPU backend only).
    pub fn toggle_perf_hud(&mut self) -> bool {
        let Backend::Wgpu { renderer } = &mut self.backend;
        renderer.toggle_perf_hud();
        true
    }

    /// Adjust subpixel gamma (WGPU backend only).
    pub fn adjust_subpixel_gamma(&mut self, delta: f32) -> bool {
        let Backend::Wgpu { renderer } = &mut self.backend;
        renderer.adjust_subpixel_gamma(delta);
        true
    }

    /// Reset subpixel gamma to default (WGPU backend only).
    pub fn reset_subpixel_gamma(&mut self) -> bool {
        let Backend::Wgpu { renderer } = &mut self.backend;
        renderer.set_subpixel_gamma(2.2);
        true
    }

    /// Draw the screen.
    ///
    /// A reference to Term whose state is being drawn must be provided.
    ///
    /// This call may block if vsync is enabled.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::vec_init_then_push)]
    #[allow(clippy::if_not_else)]
    pub fn draw<T: EventListener>(
        &mut self,
        mut terminal: MutexGuard<'_, Term<T>>,
        scheduler: &mut Scheduler,
        message_buffer: &MessageBuffer,
        config: &UiConfig,
        search_state: &mut SearchState,
        #[cfg(feature = "ai")] ai_state: Option<&crate::ai_runtime::AiUiState>,
        tab_manager: Option<&crate::workspace::TabManager>,
    ) {
        // Compute ai_panel_active flag in a cfg-safe way
        #[cfg(feature = "ai")]
        let ai_active_flag = ai_state.map(|s| s.active).unwrap_or(false);
        #[cfg(not(feature = "ai"))]
        let ai_active_flag = false;

        let _span = tracing::info_span!(
            "render.frame",
            msg_active = message_buffer.message().is_some(),
            search_active = search_state.regex().is_some(),
            ai_panel_active = ai_active_flag,
            overlay_confirm = self.confirm_overlay.active,
        )
        .entered();
        let frame_t0 = Instant::now();
        // Collect renderable content before the terminal is dropped.
        let mut content = RenderableContent::new(config, self, &terminal, search_state);
        let mut grid_cells = Vec::new();
        for cell in &mut content {
            grid_cells.push(cell);
        }
        // Remember if we observed any non-empty content; assign after releasing borrows from
        // `content`.
        let had_nonempty = !grid_cells.is_empty();
        let selection_range = content.selection_range();
        let _foreground_color = content.color(NamedColor::Foreground as usize);
        let background_color = content.color(NamedColor::Background as usize);
        let display_offset = content.display_offset();
        let cursor = content.cursor();
        // At this point `content` has been moved; it's safe to mutate startup flags.
        // If we see any non-empty content, end the clean-startup phase immediately.
        if had_nonempty {
            self.startup_nonempty_seen = true;
        } else {
            // Failsafe: if clean-startup suppression is active but no content is available yet,
            // request a frame and allow UI chrome to draw at least once so the window doesn't
            // appear as a black template. This mirrors Warp's behavior where the chrome renders
            // regardless of terminal content.
            if self.clean_startup_active() {
                // After the very first draw call, drop suppression to avoid getting stuck.
                self.startup_nonempty_seen = true;
            }
        }

        let cursor_point = terminal.grid().cursor.point;
        // Extract prompt prefix up to cursor for completions (before releasing terminal lock)
        #[cfg(feature = "completions")]
        let completions_prefix: String = {
            use openagent_terminal_core::index::Column as Col;
            use openagent_terminal_core::term::cell::Flags as CellFlags;
            let row = &terminal.grid()[cursor_point.line];
            let mut p = String::new();
            for x in 0..cursor_point.column.0 {
                let cell = &row[Col(x)];
                if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                    continue;
                }
                let ch = cell.c;
                if ch != '\u{0}' {
                    p.push(ch);
                }
            }
            p
        };
        #[cfg(feature = "completions")]
        let completions_alt_screen = terminal.mode().contains(TermMode::ALT_SCREEN);

        let total_lines = terminal.grid().total_lines();
        let metrics = self.glyph_cache.font_metrics();
        let size_info = self.size_info;

        let vi_mode = terminal.mode().contains(TermMode::VI);
        let vi_cursor_point = if vi_mode {
            Some(terminal.vi_mode_cursor.point)
        } else {
            None
        };

        // Add damage from the terminal.
        match terminal.damage() {
            TermDamage::Full => self.damage_tracker.frame().mark_fully_damaged(),
            TermDamage::Partial(damaged_lines) => {
                for damage in damaged_lines {
                    self.damage_tracker.frame().damage_line(damage);
                }
            }
        }
        terminal.reset_damage();

        // Drop terminal as early as possible to free lock.
        drop(terminal);

        // Stage HUD text if enabled (WGPU only)
        self.draw_perf_hud_text(config);

        // Invalidate highlighted hints if grid has changed.
        self.validate_hint_highlights(display_offset);

        // Add damage from OpenAgent Terminal's UI elements overlapping terminal.

        let requires_full_damage = self.visual_bell.intensity() != 0.
            || self.hint_state.active()
            || search_state.regex().is_some();
        if requires_full_damage {
            self.damage_tracker.frame().mark_fully_damaged();
            self.damage_tracker.next_frame().mark_fully_damaged();
        }

        let vi_cursor_viewport_point =
            vi_cursor_point.and_then(|cursor| term::point_to_viewport(display_offset, cursor));
        self.damage_tracker
            .damage_vi_cursor(vi_cursor_viewport_point);
        self.damage_tracker
            .damage_selection(selection_range, display_offset);

        // Update selection overlay uniform via renderer (single rect approximation per frame)
        if let Some(sel) = selection_range {
            // Build per-line spans in pixel space (Warp parity)
            let theme = config
                .resolved_theme
                .as_ref()
                .cloned()
                .unwrap_or_else(|| config.theme.resolve());
            let sel_color = if config.debug.theme_selection {
                theme.tokens.selection
            } else {
                config.colors.selection.background.color(self.colors[0], self.colors[0])
            };
            // We'll pack spans sequentially starting at buffer offset 32 (header+color)
            // Header: header0.x = span_count
            // Color written after header
            let mut spans: Vec<[f32; 4]> = Vec::new();
            // Iterate viewport rows intersecting the selection
            let start_line = sel.start.line.0.max(-(display_offset as i32));
            let end_line = sel.end.line.0.min(self.size_info.bottommost_line().0 as i32 - (display_offset as i32));
            if start_line <= end_line {
                for line_i in start_line..=end_line {
                    if line_i < 0 || (line_i as usize) >= self.size_info.screen_lines() { continue; }
                    let vp_line = line_i as usize;
                    // Compute start/end columns for this row within selection
                    let row_start_col = if sel.is_block {
                        sel.start.column.0
                    } else if line_i == sel.start.line.0 { sel.start.column.0 } else { 0 };
                    let row_end_col = if sel.is_block {
                        sel.end.column.0
                    } else if line_i == sel.end.line.0 { sel.end.column.0 } else { self.size_info.columns().saturating_sub(1) };
                    if row_end_col < row_start_col { continue; }
                    let x0 = (row_start_col as f32) * size_info.cell_width() + size_info.padding_x();
                    let y0 = (vp_line as f32) * size_info.cell_height() + size_info.padding_y();
                    let w = ((row_end_col + 1 - row_start_col) as f32) * size_info.cell_width();
                    let h = size_info.cell_height();
                    spans.push([x0, y0, w, h]);
                }
            }
            // Write header (count) and color, then spans to storage buffer
            let count = (spans.len().min(256)) as u32;
            let header: [f32; 4] = [count as f32, 0.0, 0.0, 0.0];
            let color: [f32; 4] = [sel_color.r as f32 / 255.0, sel_color.g as f32 / 255.0, sel_color.b as f32 / 255.0, 0.85];
            let mut bytes: Vec<u8> = Vec::with_capacity(16 + 16 + (count as usize) * 16);
            bytes.extend_from_slice(bytemuck::cast_slice(&header));
            bytes.extend_from_slice(bytemuck::cast_slice(&color));
            if count > 0 {
                bytes.extend_from_slice(bytemuck::cast_slice(&spans[..count as usize]));
            }
            // Zero the rest up to configured capacity to avoid stale data; write only used bytes is acceptable since shader reads by count
            // Write into the selection storage buffer at offset 0
            match &mut self.backend {
                Backend::Wgpu { renderer } => {
                    renderer.write_selection_storage(&bytes);
                }
            }
        } else {
            // No selection: write count=0
            let header: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
            let color: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
            let mut bytes: Vec<u8> = Vec::with_capacity(32);
            bytes.extend_from_slice(bytemuck::cast_slice(&header));
            bytes.extend_from_slice(bytemuck::cast_slice(&color));
            match &mut self.backend {
                Backend::Wgpu { renderer } => {
                    renderer.write_selection_storage(&bytes);
                }
            }
        }

        // Ensure renderer is ready for drawing (WGPU).

        self.renderer_clear(background_color, config.window_opacity());
        let mut lines = RenderLines::new();

        // Optimize loop hint comparator.
        let has_highlighted_hint =
            self.highlighted_hint.is_some() || self.vi_highlighted_hint.is_some();

        // Before drawing, poll DAP events to keep the debug overlay state fresh
        #[cfg(feature = "dap")]
        self.dap_poll_events();

        // Draw grid.
        {
            // Compute clean-startup suppression before taking a meter sampler borrow on `self`.
            let suppress_reserve = self.clean_startup_active()
                || (config.workspace.warp_style && config.workspace.warp_overlay_only);
            let _sampler = self.meter.sampler();

            // Dim inactive panes if enabled (draw before grid text or after depending on desired effect)
            if config.workspace.dim_inactive_panes {
                if let Some(tm) = tab_manager {
                    if let Some(active_tab) = tm.active_tab() {
                        // Compute container rect like elsewhere
                        let si = self.size_info;
                        let x0 = si.padding_x();
                        let mut y0 = si.padding_y();
                        let mut w = si.width() - 2.0 * si.padding_x();
                        let mut h = si.height() - 2.0 * si.padding_y();
                        let tab_cfg = &config.workspace.tab_bar;
                        let is_fs = self.window.is_fullscreen();
                        let eff_vis = match tab_cfg.visibility {
                            crate::config::workspace::TabBarVisibility::Always => crate::config::workspace::TabBarVisibility::Always,
                            crate::config::workspace::TabBarVisibility::Hover => crate::config::workspace::TabBarVisibility::Hover,
                            crate::config::workspace::TabBarVisibility::Auto => {
                                if is_fs { crate::config::workspace::TabBarVisibility::Hover } else { crate::config::workspace::TabBarVisibility::Always }
                            }
                        };
                        if tab_cfg.show && tab_cfg.position != crate::workspace::TabBarPosition::Hidden && matches!(eff_vis, crate::config::workspace::TabBarVisibility::Always) {
                            let ch = si.cell_height();
                            match tab_cfg.position {
                                crate::workspace::TabBarPosition::Top => { y0 += ch; h = (h - ch).max(0.0); }
                                crate::workspace::TabBarPosition::Bottom => { h = (h - ch).max(0.0); }
                                _ => {}
                            }
                        }
                        let container = crate::workspace::split_manager::PaneRect::new(x0, y0, w, h);
                        let _keep_borrow = &self.workspace_animations; // keep borrow local; actual rects computed below
                        // Fetch pane rects from SplitManager via the workspace manager; we do not have direct access here, so replicate logic using active_tab
                        let pane_rects = crate::workspace::SplitManager::new()
                            .calculate_pane_rects(&active_tab.split_layout, container);
                        // Build overlay rects: all panes except active
                        let theme = config
                            .resolved_theme
                            .as_ref()
                            .cloned()
                            .unwrap_or_else(|| config.theme.resolve());
                        let overlay_color = config.workspace.splits.overlay_color.unwrap_or(theme.tokens.overlay);
                        let alpha = config.workspace.dim_inactive_alpha.clamp(0.0, 1.0);
                        let mut rects: Vec<RenderRect> = Vec::new();
                        for (pid, r) in pane_rects.iter().copied() {
                            if pid != active_tab.active_pane {
                                rects.push(RenderRect::new(r.x, r.y, r.width, r.height, overlay_color, alpha));
                            }
                        }
                        if !rects.is_empty() {
                            let metrics = self.glyph_cache.font_metrics();
                            let size_copy = self.size_info;
                            self.renderer_draw_rects(&size_copy, &metrics, rects);
                        }
                    }
                }
            }

            // Ensure macOS hasn't reset our viewport.
            #[cfg(target_os = "macos")]
            self.renderer_set_viewport(&size_info);

            let highlighted_hint = &self.highlighted_hint;
            let vi_highlighted_hint = &self.vi_highlighted_hint;
            let damage_tracker = &mut self.damage_tracker;

            // Determine reserved rows for tab bar (hide grid content beneath)
            // Determine reserved rows for tab bar using effective visibility
            let tab_cfg = &config.workspace.tab_bar;
            let is_fs = self.window.is_fullscreen();
            let effective_visibility = match tab_cfg.visibility {
                crate::config::workspace::TabBarVisibility::Always => {
                    crate::config::workspace::TabBarVisibility::Always
                }
                crate::config::workspace::TabBarVisibility::Hover => {
                    crate::config::workspace::TabBarVisibility::Hover
                }
                crate::config::workspace::TabBarVisibility::Auto => {
                    if is_fs {
                        crate::config::workspace::TabBarVisibility::Hover
                    } else {
                        crate::config::workspace::TabBarVisibility::Always
                    }
                }
            };
            // Suppress reserving rows during clean startup to avoid a template-like top/bottom
            // band.
            let (reserve_top, reserve_bottom) = if suppress_reserve {
                (0usize, 0usize)
            } else if tab_cfg.show
                && tab_cfg.position != crate::config::workspace::TabBarPosition::Hidden
                && matches!(
                    effective_visibility,
                    crate::config::workspace::TabBarVisibility::Always
                )
            {
                match tab_cfg.position {
                    crate::config::workspace::TabBarPosition::Top => (1usize, 0usize),
                    crate::config::workspace::TabBarPosition::Bottom => (0usize, 1usize),
                    crate::config::workspace::TabBarPosition::Hidden => (0, 0),
                }
            } else {
                (0, 0)
            };

            // Filter out cells belonging to folded regions or reserved tab bar rows.
            let elide = self.blocks.enabled;
            let total_lines_vp = self.size_info.screen_lines();
            let cells = grid_cells
                .into_iter()
                .filter(|cell| {
                    // Hide reserved top rows
                    if reserve_top > 0 && cell.point.line < reserve_top {
                        return false;
                    }
                    // Hide reserved bottom rows
                    if reserve_bottom > 0
                        && cell.point.line >= total_lines_vp.saturating_sub(reserve_bottom)
                    {
                        return false;
                    }
                    if elide
                        && self
                            .blocks
                            .is_viewport_line_elided(display_offset, cell.point.line)
                    {
                        // Entire folded region is hidden from rendering (including header content).
                        return false;
                    }
                    true
                })
                .map(|mut cell| {
                    // Underline hints hovered by mouse or vi mode cursor.
                    if has_highlighted_hint {
                        let point = term::viewport_to_point(display_offset, cell.point);
                        let hyperlink = cell
                            .extra
                            .as_ref()
                            .and_then(|extra| extra.hyperlink.as_ref());

                        let should_highlight = |hint: &Option<HintMatch>| {
                            hint.as_ref()
                                .is_some_and(|hint| hint.should_highlight(point, hyperlink))
                        };
                        if should_highlight(highlighted_hint)
                            || should_highlight(vi_highlighted_hint)
                        {
                            damage_tracker.frame().damage_point(cell.point);
                            cell.flags.insert(Flags::UNDERLINE);
                        }
                    }

                    // Update underline/strikeout.
                    lines.update(&cell);

                    cell
                });
            // Drop sampler guard before borrowing `self` mutably again for drawing.
            drop(_sampler);
            let Backend::Wgpu { renderer } = &mut self.backend;
            renderer.draw_cells(&size_info, &mut self.glyph_cache, cells);
        }

        let mut rects = lines.rects(&metrics, &size_info);

        // Draw folding/unfolding animation overlays for command blocks
        if self.blocks.enabled {
            let now = Instant::now();
            let bg_cover = background_color; // cover uses terminal background to mask text
            let theme = config
                .resolved_theme
                .as_ref()
                .cloned()
                .unwrap_or_else(|| config.theme.resolve());
            for b in &mut self.blocks.blocks {
                if let Some(start) = b.anim_start {
                    // Respect reduce_motion: end animation immediately
                    if theme.ui.reduce_motion {
                        b.anim_start = None;
                        continue;
                    }

                    let dur = b.anim_duration_ms.max(1) as u128;
                    let elapsed = now.saturating_duration_since(start).as_millis();
                    let t = (elapsed as f32 / dur as f32).clamp(0.0, 1.0);
                    // Ease-out cubic
                    let eased = 1.0 - (1.0 - t).powi(3);

                    // Determine block viewport range
                    let start_total = b.start_total_line;
                    let end_total = b.end_total_line.unwrap_or(total_lines.saturating_sub(1));
                    if end_total < start_total {
                        continue;
                    }
                    let start_vp = start_total.saturating_sub(display_offset);
                    let end_vp = end_total.saturating_sub(display_offset);
                    if start_vp >= self.size_info.screen_lines() {
                        continue;
                    }
                    let end_vp = end_vp.min(self.size_info.screen_lines().saturating_sub(1));
                    if end_vp < start_vp {
                        continue;
                    }

                    let region_lines = end_vp.saturating_sub(start_vp) + 1;
                    let region_height = region_lines as f32 * self.size_info.cell_height();
                    let y_top = start_vp as f32 * self.size_info.cell_height();

                    // Compute cover height progression
                    let cover_height = if b.anim_opening {
                        // Unfolding: cover shrinks from full to 0
                        (1.0 - eased) * region_height
                    } else {
                        // Folding: cover grows from 0 to full
                        eased * region_height
                    };

                    if cover_height > 0.0 {
                        let cover = RenderRect::new(
                            0.0,
                            y_top,
                            self.size_info.width(),
                            cover_height,
                            bg_cover,
                            1.0,
                        );
                        rects.push(cover);

                        // Damage affected lines current and next frame
                        let first = start_vp;
                        let last = (y_top + cover_height).div_euclid(self.size_info.cell_height())
                            as usize;
                        let last = last.min(self.size_info.screen_lines().saturating_sub(1));
                        for line in first..=last {
                            let damage = LineDamageBounds::new(line, 0, self.size_info.columns());
                            self.damage_tracker.frame().damage_line(damage);
                            self.damage_tracker.next_frame().damage_line(damage);
                        }
                    }

                    // End animation
                    if t >= 1.0 {
                        b.anim_start = None;
                    }
                }
            }
        }

        if let Some(vi_cursor_point) = vi_cursor_point {
            // Indicate vi mode by showing the cursor's position in the top right corner.
            let line = (-vi_cursor_point.line.0 + size_info.bottommost_line().0) as usize;
            let obstructed_column = Some(vi_cursor_point)
                .filter(|point| point.line == -(display_offset as i32))
                .map(|point| point.column);
            // Suppress vi-mode line indicator when the top row is effectively reserved for the tab
            // bar.
            // In Warp-only layout we never reserve a grid row for the tab bar.
            let top_reserved = false;
            if !top_reserved {
                self.draw_line_indicator(config, total_lines, obstructed_column, line);
            }
        } else if search_state.regex().is_some() {
            // Show current display offset in vi-less search to indicate match position.
            self.draw_line_indicator(config, total_lines, None, display_offset);
        }

        // Draw cursor via text shader overlay using cursor uniforms.
        let cursor_elided = self.blocks.enabled
            && self
                .blocks
                .is_viewport_line_elided(display_offset, cursor.point().line);
        // Compute pixel-space parameters for the cursor
        let cw = size_info.cell_width();
        let ch = size_info.cell_height();
        let px = size_info.padding_x();
        let py = size_info.padding_y();
        let x = cursor.point().column.0 as f32 * cw + px;
        let y = cursor.point().line as f32 * ch + py;
        let mut w = cw * (cursor.width().get() as f32);
        let h = ch;
        let shape = cursor.shape();
        let shape_code: u32 = match shape {
            openagent_terminal_core::vte::ansi::CursorShape::Block => 0,
            openagent_terminal_core::vte::ansi::CursorShape::Beam => 1,
            openagent_terminal_core::vte::ansi::CursorShape::Underline => 2,
            openagent_terminal_core::vte::ansi::CursorShape::HollowBlock => 3,
            openagent_terminal_core::vte::ansi::CursorShape::Hidden => 0,
        };
        let thickness_px = (config.cursor.thickness() * cw).max(1.0);
        if shape_code == 1 {
            // Beam: use thickness as width
            w = thickness_px;
        } else if shape_code == 2 {
            // Underline: keep w=cell width, h=thickness handled in shader
            // Leave w,h as cell size
        }
        let color = cursor.color();
        let hidden = matches!(shape, openagent_terminal_core::vte::ansi::CursorShape::Hidden);
        let alpha = if cursor_elided || hidden { 0.0 } else { 1.0 };
        let Backend::Wgpu { renderer } = &mut self.backend;
        renderer.set_cursor_overlay(x, y, w, h, color, alpha, shape_code, thickness_px);

        // Push visual bell after url/underline/strikeout rects.
        let visual_bell_intensity = self.visual_bell.intensity();
        if visual_bell_intensity != 0. {
            let visual_bell_rect = RenderRect::new(
                0.,
                0.,
                size_info.width(),
                size_info.height(),
                config.bell.color,
                visual_bell_intensity as f32,
            );
            rects.push(visual_bell_rect);
        }

        // Handle IME positioning and search bar rendering.
        let ime_position = match search_state.regex() {
            Some(regex) => {
                let search_label = match search_state.direction() {
                    Direction::Right => FORWARD_SEARCH_LABEL,
                    Direction::Left => BACKWARD_SEARCH_LABEL,
                };

                let search_text = Self::format_search(regex, search_label, size_info.columns());

                // Render the search bar.
                self.draw_search(config, &search_text);

                // Draw search bar cursor.
                let line = size_info.screen_lines();
                let column = Column(search_text.chars().count() - 1);

                // Add cursor to search bar if IME is not active.
                if self.ime.preedit().is_none() {
                    let fg = config.colors.footer_bar_foreground();
                    let shape = CursorShape::Underline;
                    let cursor_width = NonZeroU32::new(1).unwrap();
                    let cursor =
                        RenderableCursor::new(Point::new(line, column), shape, fg, cursor_width);
                    rects.extend(cursor.rects(&size_info, config.cursor.thickness()));
                }

                Some(Point::new(line, column))
            }
            None => {
                let num_lines = self.size_info.screen_lines();
                match vi_cursor_viewport_point {
                    None => term::point_to_viewport(display_offset, cursor_point)
                        .filter(|point| point.line < num_lines),
                    point => point,
                }
            }
        };

        // Handle IME.
        if self.ime.is_enabled() {
            if let Some(point) = ime_position {
                let theme = config
                    .resolved_theme
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| config.theme.resolve());
                let (fg, bg) = (theme.tokens.text, theme.tokens.surface_muted);

                self.draw_ime_preview(point, fg, bg, &mut rects, config);
            }
        }

        if let Some(message) = message_buffer.message() {
            let search_offset = usize::from(search_state.regex().is_some());
            let text = message.text(&size_info);

            // Create a new rectangle for the background.
            let start_line = size_info.screen_lines() + search_offset;
            let y = size_info
                .cell_height()
                .mul_add(start_line as f32, size_info.padding_y());

            let theme = config
                .resolved_theme
                .as_ref()
                .cloned()
                .unwrap_or_else(|| config.theme.resolve());
            let bg = match message.ty() {
                MessageType::Error => theme.tokens.error,
                MessageType::Warning => theme.tokens.warning,
            };

            let x = 0;
            let width = size_info.width() as i32;
            let height = (size_info.height() - y) as i32;
            let message_bar_rect =
                RenderRect::new(x as f32, y, width as f32, height as f32, bg, 1.);

            // Push message_bar in the end, so it'll be above all other content.
            rects.push(message_bar_rect);

            // Always damage message bar, since it could have messages of the same size in it.
            self.damage_tracker
                .frame()
                .add_viewport_rect(&size_info, x, y as i32, width, height);

            // Draw rectangles and HUD text (if enabled).
            self.draw_perf_hud_text(config);
            self.renderer_draw_rects(&size_info, &metrics, rects);

            // Relay messages to the user.
            let theme = config
                .resolved_theme
                .as_ref()
                .cloned()
                .unwrap_or_else(|| config.theme.resolve());
            let fg = theme.tokens.surface;
            for (i, message_text) in text.iter().enumerate() {
                let point = Point::new(start_line + i, Column(0));
                let size_info_copy = size_info;
                let Backend::Wgpu { renderer } = &mut self.backend;
                renderer.draw_string(
                    point,
                    fg,
                    bg,
                    message_text.chars(),
                    &size_info_copy,
                    &mut self.glyph_cache,
                );
            }
        } else {
            // Draw rectangles and HUD text (if enabled).
            self.draw_perf_hud_text(config);
            self.renderer_draw_rects(&size_info, &metrics, rects);
        }

        // Draw inline AI suggestion as subtle ghost text at the prompt (when enabled)
        #[cfg(feature = "ai")]
        if let Some(ai) = ai_state {
            if !ai.active {
                if let Some(suffix) = ai.inline_suggestion.as_ref() {
                    if !suffix.is_empty() {
                        if let Some(vp) = term::point_to_viewport(display_offset, cursor_point) {
                            // Compute available width from cursor to end of line
                            let start_col = vp.column.0;
                            let cols = self.size_info.columns();
                            if vp.line < self.size_info.screen_lines() && start_col < cols {
                                let avail = cols - start_col;
                                // Theme colors: use muted text color
                                let theme = config
                                    .resolved_theme
                                    .as_ref()
                                    .cloned()
                                    .unwrap_or_else(|| config.theme.resolve());
                                let fg = theme.tokens.text_muted;
                                // Draw at cursor position (ghost text)
                                let point = Point::new(vp.line, Column(start_col));
                                self.draw_ai_panel_text(point, fg, background_color, suffix, avail);
                            }
                        }
                    }
                }
            }
        }

        // Draw AI panel using unified drawing system.
        #[cfg(feature = "ai")]
        if let Some(ai) = ai_state {
            use crate::display::ai_drawing::AiRenderMode;
            let ai_rects = self.draw_ai_unified(config, ai, AiRenderMode::Panel);
            if !ai_rects.is_empty() {
                let size_info_copy = self.size_info;
                self.renderer_draw_rects(&size_info_copy, &metrics, ai_rects);
            }
        }

        // Draw Blocks Search panel overlay if active.
        #[cfg(feature = "blocks")]
        if self.blocks_search.active {
            let bs_state = self.blocks_search.clone();
            self.draw_blocks_search_overlay(config, &bs_state);
        }

        // Draw always-on completions overlay (experimental) when enabled and applicable.
        #[cfg(feature = "completions")]
        {
            if config.debug.completions {
                let cursor_point_usize =
                    Point::new(cursor_point.line.0 as usize, cursor_point.column);
                self.draw_completions_overlay_with_context(
                    config,
                    &completions_prefix,
                    cursor_point_usize,
                    display_offset,
                    completions_alt_screen,
                );
            }
        }

        // Draw split indicators for Warp-style splits (visual only)
        if config.workspace.warp_style {
            if let Some(active_tab) = tab_manager.and_then(|tm| tm.active_tab()) {
                let indicators = self.warp_split_indicators_from_config(config);
                self.draw_warp_split_indicators(config, &active_tab.split_layout, &indicators);
                // Optional: draw a subtle overlay if zoomed (detected via saved layout)
                if active_tab.zoom_saved_layout.is_some() {
                    // Draw overlay to indicate zoom state
                    self.draw_warp_zoom_overlay(active_tab.active_pane, &indicators);
                }
            }
        }

        // Draw overlay tab bar (legacy tab-row removed)
        if let Some(tm) = tab_manager {
            let tab_cfg = &config.workspace.tab_bar;
            if tab_cfg.show && tab_cfg.position != crate::workspace::TabBarPosition::Hidden {
                let style = crate::display::warp_ui::WarpTabStyle::from_theme(config);
                let is_fs = self.window.is_fullscreen();
                let show_bar = should_show_tab_bar_overlay(
                    self.size_info,
                    self.last_mouse_y,
                    tab_cfg,
                    is_fs,
                    &style,
                );
                if show_bar {
                    let _ = self.draw_warp_tab_bar(config, tm, tab_cfg.position, &style);
                }
            }
        }
        // Draw persistent Quick Actions bar (mouse-first entrypoint)
        // Avoid overlap with reserved bottom tab row and with active search/footer bars
        let has_search = search_state.regex().is_some();
        let has_message = message_buffer.message().is_some();
        // Draw Quick Actions bar when enabled and not obstructed by search/message bar
        if config.workspace.quick_actions.show
            && !has_search
            && !has_message
            && !self.clean_startup_active()
        {
            self.draw_quick_actions_bar(config);
        }

        // Warp-like bottom composer bar (visual only) — always draw for Warp-style UI
        if config.workspace.warp_style {
            self.draw_warp_bottom_composer(config);
        }

        // Transient overlays: draw last, after tab bar, quick actions, and composer.
        if !self.clean_startup_active() {
            // Draw pane drag overlay (preview and drop-zone highlights) if a pane drag is in
            // progress
            if let Some(tm) = tab_manager {
                if let Some(active_tab) = tm.active_tab() {
                    self.draw_pane_drag_overlay(config, active_tab);
                }
            }

            // Confirmation overlay
            if self.confirm_overlay.active {
                let st = self.confirm_overlay.clone();
                self.draw_confirm_overlay(config, &st);
            }

            // DAP debug overlay if active
            #[cfg(feature = "dap")]
            if self.dap_overlay.active {
                let st = self.dap_overlay.clone();
                self.draw_dap_overlay(config, &st);
            }

            // Workflows panel overlay if active
            #[cfg(feature = "workflow")]
            if self.workflows_panel.active {
                let st = self.workflows_panel.clone();
                self.draw_workflows_panel_overlay(config, &st);
            }
            // Notebooks panel overlay if active
            #[cfg(feature = "blocks")]
            if self.notebooks_panel.active {
                let st = self.notebooks_panel.clone();
                self.draw_notebooks_panel_overlay(config, &st);
            }
            // Workflows progress overlay if active
            #[cfg(feature = "workflow")]
            if self.workflows_progress.active {
                let st = self.workflows_progress.clone();
                self.draw_workflows_progress_overlay(config, &st);
            }
            // Workflows params overlay if active
            #[cfg(feature = "workflow")]
            if self.workflows_params.active {
                let st = self.workflows_params.clone();
                self.draw_workflows_params_overlay(config, &st);
            }

            // Settings panel overlay if active
            if self.settings_panel.active {
                let st = self.settings_panel.clone();
                self.draw_settings_panel_overlay(config, &st);
            }

            // Command Palette overlay: draw when active or during animation
            if self.palette.active() || self.palette_anim_start.is_some() {
                self.draw_palette_overlay(config);
            }

            // Debug split overlay preview (temporary until full pane implementation is wired)
            if let Some(vertical) = self.debug_split_overlay {
                let theme = config
                    .resolved_theme
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| config.theme.resolve());
                let tokens = theme.tokens;
                let mut rects = Vec::new();
                let w = self.size_info.width();
                let h = self.size_info.height();
                let gap: f32 = 2.0;
                if vertical {
                    // Top / bottom split
                    let top_h = (h - gap) * 0.5;
                    let bottom_h = h - gap - top_h;
                    rects.push(RenderRect::new(
                        0.0,
                        0.0,
                        w,
                        top_h,
                        tokens.surface_muted,
                        0.96,
                    ));
                    rects.push(RenderRect::new(
                        0.0,
                        top_h + gap,
                        w,
                        bottom_h,
                        tokens.surface,
                        0.96,
                    ));
                } else {
                    // Left / right split
                    let left_w = (w - gap) * 0.5;
                    let right_w = w - gap - left_w;
                    rects.push(RenderRect::new(
                        0.0,
                        0.0,
                        left_w,
                        h,
                        tokens.surface_muted,
                        0.96,
                    ));
                    rects.push(RenderRect::new(
                        left_w + gap,
                        0.0,
                        right_w,
                        h,
                        tokens.surface,
                        0.96,
                    ));
                }
                let metrics = self.glyph_cache.font_metrics();
                let size_info = self.size_info;
                self.renderer_draw_rects(&size_info, &metrics, rects);
            }

            // Draw hyperlink uri preview.
            if has_highlighted_hint {
                let cursor_point = vi_cursor_point.or(Some(cursor_point));
                self.draw_hyperlink_preview(config, cursor_point, display_offset);
            }

            // Draw overlays for command blocks (headers and folded regions).
            if self.blocks.enabled {
                let num_lines = self.size_info.screen_lines();
                let theme = config
                    .resolved_theme
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| config.theme.resolve());
                let fg = theme.tokens.text;
                let bg = theme.tokens.surface_muted;
                for line in 0..num_lines {
                    // Folded overlay.
                    if let Some(label) = self
                        .blocks
                        .folded_label_at_viewport_line(display_offset, line)
                    {
                        let damage = LineDamageBounds::new(line, 0, self.size_info.columns());
                        self.damage_tracker.frame().damage_line(damage);
                        self.damage_tracker.next_frame().damage_line(damage);

                        let point = Point::new(line, Column(0));
                        {
                            let size_info_copy = self.size_info;
                            let Backend::Wgpu { renderer } = &mut self.backend;
                            renderer.draw_string(
                                point,
                                fg,
                                bg,
                                label.chars(),
                                &size_info_copy,
                                &mut self.glyph_cache,
                            );
                        }
                        continue;
                    }

                    // Unfolded block header overlay.
                    if let Some(header) = self.blocks.header_at_viewport_line(display_offset, line)
                    {
                        let damage = LineDamageBounds::new(line, 0, self.size_info.columns());
                        self.damage_tracker.frame().damage_line(damage);
                        self.damage_tracker.next_frame().damage_line(damage);

                        // Draw header text
                        let mut col = 0usize;
                        let point = Point::new(line, Column(col));
                        {
                            let size_info_copy = self.size_info;
                            let Backend::Wgpu { renderer } = &mut self.backend;
                            renderer.draw_string(
                                point,
                                fg,
                                bg,
                                header.chars(),
                                &size_info_copy,
                                &mut self.glyph_cache,
                            );
                        }
                        use unicode_width::UnicodeWidthStr as _;
                        col += header.width() + 2;

                        // Draw action chips: [Copy] [Rerun] [Export]
                        let chips = ["[Copy]", "[Rerun]", "[Export]"];
                        let hover_line = self.blocks_header_hover_line;
                        let hover_chip = self.blocks_header_hover_chip;
                        let press_chip = if self
                            .blocks_press_flash_until
                            .is_some_and(|t| t > Instant::now())
                        {
                            self.blocks_press_flash_chip
                        } else {
                            None
                        };
                        for (i, chip) in chips.iter().enumerate() {
                            if col < self.size_info.columns() {
                                // Optional hover highlight/press flash with pill background
                                if hover_line == Some(line) {
                                    let cw = self.size_info.cell_width();
                                    let ch = self.size_info.cell_height();
                                    let x_px = (col as f32) * cw;
                                    let w_px = (chip.len() as f32) * cw;
                                    let y_px = (line as f32) * ch + (ch * 0.15);
                                    let h_px = (ch * 0.70).max(10.0);
                                    let theme = config
                                        .resolved_theme
                                        .as_ref()
                                        .cloned()
                                        .unwrap_or_else(|| config.theme.resolve());
                                    let hl = theme.tokens.accent;
                                    let is_hover = hover_chip == Some(i);
                                    let is_press = press_chip == Some(i);
                                    let alpha = if is_press {
                                        0.42
                                    } else if is_hover {
                                        0.28
                                    } else {
                                        0.18
                                    };
                                    let mut radius = h_px / 2.0;
                                    if radius > 22.0 {
                                        radius = 22.0;
                                    }
                                    let pill = UiRoundedRect::new(
                                        x_px, y_px, w_px, h_px, radius, hl, alpha,
                                    );
                                    self.stage_ui_rounded_rect(pill);
                                }

                                let point = Point::new(line, Column(col));
                                let size_info_copy = self.size_info;
                                let Backend::Wgpu { renderer } = &mut self.backend;
                                renderer.draw_string(
                                    point,
                                    fg, // keep fg for text readability; could use tokens.accent for emphasis
                                    bg,
                                    chip.chars(),
                                    &size_info_copy,
                                    &mut self.glyph_cache,
                                );
                                col += chip.width() + 1;
                            }
                        }
                        continue;
                    }
                }
            }

            // Draw render timer at the very end (debug only)
            self.draw_render_timer(config);
        } // end of if !self.clean_startup_active()

        // Notify winit that we're about to present.
        self.window.pre_present_notify();

        // Frame end timing
        let elapsed = frame_t0.elapsed();
        tracing::info!(
            elapsed_ms = elapsed.as_millis() as u64,
            "render.frame_complete"
        );

        // Feed perf history for HUD smoothing (keep last 120)
        let Backend::Wgpu { renderer } = &mut self.backend;
        renderer.record_frame_time(elapsed.as_secs_f32() * 1000.0);

        // Highlight damage for debugging.
        if self.damage_tracker.debug {
            let damage = self
                .damage_tracker
                .shape_frame_damage(self.size_info.into());
            let mut rects = Vec::with_capacity(damage.len());
            self.highlight_damage(&mut rects);
            let size_info_copy = self.size_info;
            self.renderer_draw_rects(&size_info_copy, &metrics, rects);
        }

        // Clearing debug highlights from the previous frame requires full redraw.
        if matches!(self.raw_window_handle, RawWindowHandle::Wayland(_)) {
            self.request_frame(scheduler);
        }

        self.damage_tracker.swap_damage();
    }

    /// Update to a new configuration.
    pub fn update_config(&mut self, config: &UiConfig) {
        self.damage_tracker.debug = config.debug.highlight_damage;
        self.visual_bell.update_config(&config.bell);
        self.colors = List::from(&config.colors);

        // Honor blocks flag on reload without disabling if it was already enabled by events.
        if config.debug.blocks {
            self.blocks.enabled = true;
        }
    }

    /// Update the mouse/vi mode cursor hint highlighting.
    ///
    /// This will return whether the highlighted hints changed.
    pub fn update_highlighted_hints<T>(
        &mut self,
        term: &Term<T>,
        config: &UiConfig,
        mouse: &Mouse,
        modifiers: ModifiersState,
    ) -> bool {
        // Update vi mode cursor hint.
        let vi_highlighted_hint = if term.mode().contains(TermMode::VI) {
            let mods = ModifiersState::all();
            let point = term.vi_mode_cursor.point;
            hint::highlighted_at(term, config, point, mods)
        } else {
            None
        };
        let mut dirty = vi_highlighted_hint != self.vi_highlighted_hint;
        self.vi_highlighted_hint = vi_highlighted_hint;
        self.vi_highlighted_hint_age = 0;

        // Force full redraw if the vi mode highlight was cleared.
        if dirty {
            self.damage_tracker.frame().mark_fully_damaged();
        }

        // Abort if mouse highlighting conditions are not met.
        if !mouse.inside_text_area || !term.selection.as_ref().map_or(true, Selection::is_empty) {
            if self.highlighted_hint.take().is_some() {
                self.damage_tracker.frame().mark_fully_damaged();
                dirty = true;
            }
            return dirty;
        }

        // Find highlighted hint at mouse position.
        let point = mouse.point(&self.size_info, term.grid().display_offset());
        let highlighted_hint = hint::highlighted_at(term, config, point, modifiers);

        // Update cursor shape.
        if highlighted_hint.is_some() {
            // If mouse changed the line, we should update the hyperlink preview, since the
            // highlighted hint could be disrupted by the old preview.
            dirty = self.hint_mouse_point.is_some_and(|p| p.line != point.line);
            self.hint_mouse_point = Some(point);
            self.window.set_mouse_cursor(CursorIcon::Pointer);
        } else if self.highlighted_hint.is_some() {
            self.hint_mouse_point = None;
            if term.mode().intersects(TermMode::MOUSE_MODE) && !term.mode().contains(TermMode::VI) {
                self.window.set_mouse_cursor(CursorIcon::Default);
            } else {
                self.window.set_mouse_cursor(CursorIcon::Text);
            }
        }

        let mouse_highlight_dirty = self.highlighted_hint != highlighted_hint;
        dirty |= mouse_highlight_dirty;
        self.highlighted_hint = highlighted_hint;
        self.highlighted_hint_age = 0;

        // Force full redraw if the mouse cursor highlight was changed.
        if mouse_highlight_dirty {
            self.damage_tracker.frame().mark_fully_damaged();
        }

        dirty
    }

    #[inline(never)]
    // Backend helpers for renderer dispatch.
    fn renderer_clear(&self, color: Rgb, alpha: f32) {
        let Backend::Wgpu { renderer } = &self.backend;
        renderer.clear(color, alpha)
    }

    fn renderer_resize(&mut self) {
        let Backend::Wgpu { renderer } = &mut self.backend;
        renderer.resize(&self.size_info)
    }

    #[allow(dead_code)]
    fn stage_ui_rounded_rect(&mut self, rect: UiRoundedRect) {
        let Backend::Wgpu { renderer } = &mut self.backend;
        renderer.stage_ui_rounded_rect(&self.size_info, rect)
    }

    fn renderer_draw_rects(
        &mut self,
        size_info: &SizeInfo,
        metrics: &Metrics,
        rects: Vec<RenderRect>,
    ) {
        let Backend::Wgpu { renderer } = &mut self.backend;
        renderer.draw_rects(self.window.winit_window(), size_info, metrics, rects)
    }

    #[allow(dead_code)]
    fn stage_ui_sprite(&mut self, sprite: UiSprite) {
        let Backend::Wgpu { renderer } = &mut self.backend;
        renderer.stage_ui_sprite(sprite)
    }

    #[allow(dead_code)]
    fn set_ui_sprite_filter(&mut self, nearest: bool) {
        let Backend::Wgpu { renderer } = &mut self.backend;
        renderer.set_sprite_filter_nearest(nearest)
    }

    fn draw_warp_bottom_composer(&mut self, config: &UiConfig) {
        let theme = config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;
        let ui = theme.ui.clone();
        let cw = self.size_info.cell_width();
        let ch = self.size_info.cell_height();
        let cols = self.size_info.columns();
        let lines = self.size_info.screen_lines();
        if cols == 0 || lines == 0 {
            return;
        }
        // Band background on bottom
        let y_band = (lines.saturating_sub(1)) as f32 * ch;
        let rects = vec![RenderRect::new(
            0.0,
            y_band,
            self.size_info.width(),
            ch,
            tokens.surface_muted,
            ui.composer_band_alpha,
        )];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);
        // Rounded composer pill
        let margin_px = ui.composer_margin_px.max(0.0);
        let x = margin_px;
        let y_inset = ui.composer_pill_vertical_inset_px.max(0.0);
        let y = y_band + y_inset;
        let w = self.size_info.width() - margin_px * 2.0;
        let h = ch - y_inset * 2.0;
        // Compute hover and press flash for composer chips
        let px = self.last_mouse_x as f32;
        let py = self.last_mouse_y as f32;
        let inside_pill = px >= x && px < x + w && py >= y && py < y + h;
        let hover_col_for_composer: Option<usize> = if inside_pill {
            Some((((px - self.size_info.padding_x()) / cw).floor() as isize).max(0) as usize)
        } else {
            None
        };
        let now = Instant::now();
        let composer_press_flash = self.composer_press_flash_until.is_some_and(|t| now < t);

        // Focus ring / stronger bg when focused
        if self.composer_focused {
            let ring = UiRoundedRect::new(
                x - 1.0,
                y - 1.0,
                w + 2.0,
                h + 2.0,
                ui.composer_pill_radius_px
                    .unwrap_or(ui.palette_pill_radius_px)
                    + 1.0,
                tokens.accent,
                ui.composer_focus_ring_alpha,
            );
            self.stage_ui_rounded_rect(ring);
        }
        let bg_alpha = if self.composer_focused {
            ui.composer_pill_alpha_focused
        } else {
            ui.composer_pill_alpha_unfocused
        };
        let pill = UiRoundedRect::new(
            x,
            y,
            w,
            h,
            ui.composer_pill_radius_px
                .unwrap_or(ui.palette_pill_radius_px),
            tokens.surface,
            bg_alpha,
        );
        self.stage_ui_rounded_rect(pill);

        // Placeholder text
        let placeholder = ui.composer_placeholder_text.as_deref().unwrap_or(
            "Warp anything e.g. Help me optimize my SQL queries that are running slowly",
        );
        let mut start_col = 2usize;
        // Sparkle/star glyph to hint AI
        let star = ui.composer_star_glyph.as_deref().unwrap_or("✦ ");
        let star_color = tokens.accent;
        self.draw_ai_text(
            Point::new(lines.saturating_sub(1), Column(start_col)),
            star_color,
            tokens.surface_muted,
            star,
            cols.saturating_sub(start_col),
        );
        start_col += star.len();

        // Inline provider chip and compact model badge on the left inside the composer pill
        // Compute labels
        let provider_label = {
            let pid = self.ai_current_provider.as_str();
            match pid {
                "openai" => "OpenAI",
                "openrouter" => "OpenRouter",
                "anthropic" => "Anthropic",
                "ollama" => "Ollama",
                _ => {
                    // Title-case fallback
                    if pid.is_empty() {
                        "Provider"
                    } else {
                        pid
                    }
                }
            }
        };
        let provider_chip = format!("[{} ▾]", provider_label);
        // Draw provider chip capsule
        {
            use unicode_width::UnicodeWidthStr as _;
            let wcols = provider_chip.width();
            // Capsule background
            let pad = ui
                .composer_chip_pad_px
                .unwrap_or(ui.palette_chip_pad_px)
                .max(1.0);
            let x_px = (start_col as f32) * cw - pad;
            let y_px = (lines.saturating_sub(1) as f32) * ch + (ch - (ch * 0.8)) * 0.5;
            let h_px = ch * 0.8;
            let w_px = (wcols as f32) * cw + pad * 2.0;
            let radius = ui.palette_pill_radius_px.min(h_px * 0.5);
            let mut alpha = if self.composer_focused {
                ui.composer_chip_alpha_focused
            } else {
                ui.composer_chip_alpha_unfocused
            };
            // Hover detection for provider chip
            let is_hovered =
                hover_col_for_composer.is_some_and(|c| c >= start_col && c < start_col + wcols);
            if is_hovered {
                alpha = (alpha + ui.composer_chip_alpha_hover_delta).min(1.0);
            }
            if composer_press_flash && is_hovered {
                alpha = (alpha + ui.composer_chip_alpha_press_delta).min(1.0);
            }
            let pill =
                UiRoundedRect::new(x_px, y_px, w_px, h_px, radius, tokens.surface_muted, alpha);
            self.stage_ui_rounded_rect(pill);
            // Chip text
            let text_color = if is_hovered {
                tokens.accent
            } else {
                tokens.text
            };
            self.draw_ai_text(
                Point::new(lines.saturating_sub(1), Column(start_col)),
                text_color,
                tokens.surface_muted,
                &provider_chip,
                wcols,
            );
            start_col += wcols + 1; // space after chip
        }
        // Compact model badge, if available
        if !self.ai_current_model.is_empty() {
            use unicode_width::UnicodeWidthStr as _;
            let model_text = {
                // Truncate very long model names to a reasonable length for the chip
                let max_len = 24usize;
                if self.ai_current_model.len() > max_len {
                    format!("{}…", &self.ai_current_model[..max_len])
                } else {
                    self.ai_current_model.clone()
                }
            };
            let model_chip = format!("[{}]", model_text);
            let wcols = model_chip.width();
            let pad = ui
                .composer_chip_pad_px
                .unwrap_or(ui.palette_chip_pad_px)
                .max(1.0);
            let x_px = (start_col as f32) * cw - pad;
            let y_px = (lines.saturating_sub(1) as f32) * ch + (ch - (ch * 0.8)) * 0.5;
            let h_px = ch * 0.8;
            let w_px = (wcols as f32) * cw + pad * 2.0;
            let radius = ui.palette_pill_radius_px.min(h_px * 0.5);
            let mut alpha = if self.composer_focused {
                ui.composer_chip_alpha_focused
            } else {
                ui.composer_chip_alpha_unfocused
            };
            let is_hovered =
                hover_col_for_composer.is_some_and(|c| c >= start_col && c < start_col + wcols);
            if is_hovered {
                alpha = (alpha + ui.composer_chip_alpha_hover_delta).min(1.0);
            }
            if composer_press_flash && is_hovered {
                alpha = (alpha + ui.composer_chip_alpha_press_delta).min(1.0);
            }
            let pill =
                UiRoundedRect::new(x_px, y_px, w_px, h_px, radius, tokens.surface_muted, alpha);
            self.stage_ui_rounded_rect(pill);
            let text_color = if is_hovered {
                tokens.accent
            } else {
                tokens.text
            };
            self.draw_ai_text(
                Point::new(lines.saturating_sub(1), Column(start_col)),
                text_color,
                tokens.surface_muted,
                &model_chip,
                wcols,
            );
            start_col += wcols + 2; // extra space after model chip
        }

        let available = cols.saturating_sub(start_col + 2);

        use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

        // Update caret blink state
        if self.composer_focused {
            let rate = ui.composer_blink_rate_ms;
            if rate == 0 || ui.reduce_motion {
                self.composer_caret_visible = true;
                self.composer_caret_last_toggle = Some(std::time::Instant::now());
            } else if let Some(t0) = self.composer_caret_last_toggle {
                if t0.elapsed().as_millis() as u64 >= rate as u64 {
                    self.composer_caret_visible = !self.composer_caret_visible;
                    self.composer_caret_last_toggle = Some(std::time::Instant::now());
                }
            } else {
                self.composer_caret_visible = true;
                self.composer_caret_last_toggle = Some(std::time::Instant::now());
            }
        } else {
            self.composer_caret_visible = false;
            self.composer_caret_last_toggle = None;
        }

        if available > 0 {
            let text = self.composer_text.clone();
            if text.is_empty() && !self.composer_focused {
                let ph_color = tokens.text_muted;
                self.draw_ai_text(
                    Point::new(lines.saturating_sub(1), Column(start_col)),
                    ph_color,
                    tokens.surface_muted,
                    placeholder,
                    available,
                );
                // Also draw action chips even when placeholder is shown
                {
                    use unicode_width::UnicodeWidthStr as _;
                    let chips = ["[Palette]", "[Run]"];
                    let mut col_end = cols.saturating_sub(2);
                    let gap = ui.composer_chip_gap_cols as usize;
                    for label in chips.iter() {
                        let wcols = label.width();
                        if wcols + 1 >= col_end {
                            break;
                        }
                        let start = col_end.saturating_sub(wcols);
                        // Capsule background
                        let pad = ui
                            .composer_chip_pad_px
                            .unwrap_or(ui.palette_chip_pad_px)
                            .max(1.0);
                        let x_px = (start as f32) * cw - pad;
                        let y_px = (lines.saturating_sub(1) as f32) * ch + (ch - (ch * 0.8)) * 0.5;
                        let h_px = ch * 0.8; // slightly inset vertically
                        let w_px = (wcols as f32) * cw + pad * 2.0;
                        let radius = ui.palette_pill_radius_px.min(h_px * 0.5);
                        let mut alpha = if self.composer_focused {
                            ui.composer_chip_alpha_focused
                        } else {
                            ui.composer_chip_alpha_unfocused
                        };
                        let is_hovered =
                            hover_col_for_composer.is_some_and(|c| c >= start && c < start + wcols);
                        if is_hovered {
                            alpha = (alpha + ui.composer_chip_alpha_hover_delta).min(1.0);
                        }
                        if composer_press_flash && is_hovered {
                            alpha = (alpha + ui.composer_chip_alpha_press_delta).min(1.0);
                        }
                        let pill = UiRoundedRect::new(
                            x_px,
                            y_px,
                            w_px,
                            h_px,
                            radius,
                            tokens.surface_muted,
                            alpha,
                        );
                        self.stage_ui_rounded_rect(pill);
                        // Label text
                        let text_color = if is_hovered {
                            tokens.accent
                        } else {
                            tokens.text
                        };
                        self.draw_ai_text(
                            Point::new(lines.saturating_sub(1), Column(start)),
                            text_color,
                            tokens.surface_muted,
                            label,
                            wcols,
                        );
                        if start <= 2 {
                            break;
                        }
                        col_end = start.saturating_sub(gap);
                    }
                }
                return;
            }

            // Compute total width cols and cursor col
            let total_cols = text.width();
            let cursor_cols = text[..self.composer_cursor.min(text.len())].width();

            // Ensure cursor stays visible by adjusting view offset
            let mut offset = self.composer_view_col_offset.min(total_cols);
            if cursor_cols < offset {
                offset = cursor_cols;
            } else if cursor_cols.saturating_sub(offset) >= available {
                let target = cursor_cols.saturating_sub(available.saturating_sub(1));
                offset = target.min(total_cols);
            } else if total_cols.saturating_sub(offset) < available {
                // Shift back when there's slack at the end
                let slack = available.saturating_sub(total_cols.saturating_sub(offset));
                offset = offset.saturating_sub(slack);
            }
            self.composer_view_col_offset = offset;

            // Map from col offset -> byte index into text
            let mut col_acc = 0usize;
            let mut start_byte = 0usize;
            for (i, ch) in text.char_indices() {
                let wch = ch.width().unwrap_or(1);
                if col_acc + wch > offset {
                    start_byte = i;
                    break;
                }
                col_acc += wch;
                start_byte = i + ch.len_utf8();
            }
            // Collect visible slice within available cols
            let mut used_cols = 0usize;
            let mut end_byte = start_byte;
            for (i, ch) in text[start_byte..].char_indices() {
                let wch = ch.width().unwrap_or(1);
                if used_cols + wch > available {
                    break;
                }
                used_cols += wch;
                end_byte = start_byte + i + ch.len_utf8();
            }
            let visible = &text[start_byte..end_byte];

            // Draw selection background if any intersects visible
            if let Some(anchor) = self.composer_sel_anchor {
                if anchor != self.composer_cursor {
                    let sel_lo = anchor.min(self.composer_cursor);
                    let sel_hi = anchor.max(self.composer_cursor);
                    let sel_left_cols = text[..sel_lo].width();
                    let sel_width_cols = text[sel_lo..sel_hi].width();
                    // Visible intersection in columns
                    let vis_lo = sel_left_cols.saturating_sub(offset);
                    let vis_hi = sel_left_cols + sel_width_cols;
                    let vis_hi = vis_hi.saturating_sub(offset);
                    let start_in_vis = vis_lo.min(available);
                    if vis_hi > 0 && start_in_vis < available {
                        let end_in_vis = vis_hi.min(available);
                        let width_cols = end_in_vis.saturating_sub(start_in_vis);
                        if width_cols > 0 {
                            let x_px = ((start_col + start_in_vis) as f32) * cw;
                            let y_px = (lines.saturating_sub(1) as f32) * ch;
                            let w_px = (width_cols as f32) * cw;
                            let rect = UiRoundedRect::new(
                                x_px,
                                y_px,
                                w_px,
                                ch,
                                ui.palette_pill_radius_px * 0.35,
                                tokens.selection,
                                0.9,
                            );
                            self.stage_ui_rounded_rect(rect);
                        }
                    }
                }
            }

            // Draw text segments (before selection / selection / after selection)
            if let Some(anchor) = self.composer_sel_anchor {
                if anchor == self.composer_cursor {
                    // No selection
                    self.draw_ai_text(
                        Point::new(lines.saturating_sub(1), Column(start_col)),
                        tokens.text,
                        tokens.surface_muted,
                        visible,
                        available,
                    );
                } else {
                    let sel_lo = anchor.min(self.composer_cursor);
                    let sel_hi = anchor.max(self.composer_cursor);
                    let vis_sel_start_cols =
                        text[..sel_lo].width().saturating_sub(offset).min(available);
                    let vis_sel_end_cols =
                        text[..sel_hi].width().saturating_sub(offset).min(available);
                    // Compute byte indices for segment boundaries within visible range
                    let vis_sel_start_byte = if vis_sel_start_cols == 0 {
                        start_byte
                    } else {
                        let mut cacc = 0usize;
                        let mut b = start_byte;
                        for (i, ch) in text[start_byte..].char_indices() {
                            let wch = ch.width().unwrap_or(1);
                            if cacc + wch > vis_sel_start_cols {
                                break;
                            }
                            cacc += wch;
                            b = start_byte + i + ch.len_utf8();
                        }
                        b
                    };
                    let vis_sel_end_byte = if vis_sel_end_cols == 0 {
                        start_byte
                    } else {
                        let mut cacc = 0usize;
                        let mut b = start_byte;
                        for (i, ch) in text[start_byte..].char_indices() {
                            let wch = ch.width().unwrap_or(1);
                            if cacc + wch > vis_sel_end_cols {
                                break;
                            }
                            cacc += wch;
                            b = start_byte + i + ch.len_utf8();
                        }
                        b
                    };
                    // Draw 'before'
                    if vis_sel_start_byte > start_byte {
                        let before = &text[start_byte..vis_sel_start_byte];
                        self.draw_ai_text(
                            Point::new(lines.saturating_sub(1), Column(start_col)),
                            tokens.text,
                            tokens.surface_muted,
                            before,
                            available,
                        );
                    }
                    // Draw selection text
                    if vis_sel_end_byte > vis_sel_start_byte {
                        let sel_vis = &text[vis_sel_start_byte..vis_sel_end_byte];
                        let sel_offset_cols = vis_sel_start_cols;
                        self.draw_ai_text(
                            Point::new(
                                lines.saturating_sub(1),
                                Column(start_col + sel_offset_cols),
                            ),
                            tokens.text,
                            tokens.surface_muted,
                            sel_vis,
                            available.saturating_sub(sel_offset_cols),
                        );
                    }
                    // Draw 'after'
                    if end_byte > vis_sel_end_byte {
                        let after = &text[vis_sel_end_byte..end_byte];
                        let after_offset_cols = vis_sel_end_cols;
                        self.draw_ai_text(
                            Point::new(
                                lines.saturating_sub(1),
                                Column(start_col + after_offset_cols),
                            ),
                            tokens.text,
                            tokens.surface_muted,
                            after,
                            available.saturating_sub(after_offset_cols),
                        );
                    }
                }
            } else {
                // No selection
                self.draw_ai_text(
                    Point::new(lines.saturating_sub(1), Column(start_col)),
                    tokens.text,
                    tokens.surface_muted,
                    visible,
                    available,
                );
            }

            // Draw caret when focused and visible
            if self.composer_focused && self.composer_caret_visible {
                let caret_cols_in_vis = cursor_cols.saturating_sub(offset).min(available);
                let caret_col = start_col + caret_cols_in_vis.min(available.saturating_sub(1));
                self.draw_ai_text(
                    Point::new(lines.saturating_sub(1), Column(caret_col)),
                    tokens.surface_muted,
                    tokens.text,
                    " ",
                    1,
                );
            }

            // Draw action chips aligned to the right inside the pill: [Palette] [AI] [Run]
            {
                use unicode_width::UnicodeWidthStr as _;
                let chips = ["[Palette]", "[Run]"];
                let mut col_end = cols.saturating_sub(2);
                let gap = ui.composer_chip_gap_cols as usize;
                for label in chips.iter() {
                    let wcols = label.width();
                    if wcols + 1 >= col_end {
                        break;
                    }
                    let start = col_end.saturating_sub(wcols);
                    // Capsule background
                    let pad = ui
                        .composer_chip_pad_px
                        .unwrap_or(ui.palette_chip_pad_px)
                        .max(1.0);
                    let x_px = (start as f32) * cw - pad;
                    let y_px = (lines.saturating_sub(1) as f32) * ch + (ch - (ch * 0.8)) * 0.5;
                    let h_px = ch * 0.8; // slightly inset vertically
                    let w_px = (wcols as f32) * cw + pad * 2.0;
                    let radius = ui.palette_pill_radius_px.min(h_px * 0.5);
                    let mut alpha = if self.composer_focused {
                        ui.composer_chip_alpha_focused
                    } else {
                        ui.composer_chip_alpha_unfocused
                    };
                    let is_hovered =
                        hover_col_for_composer.is_some_and(|c| c >= start && c < start + wcols);
                    if is_hovered {
                        alpha = (alpha + ui.composer_chip_alpha_hover_delta).min(1.0);
                    }
                    if composer_press_flash && is_hovered {
                        alpha = (alpha + ui.composer_chip_alpha_press_delta).min(1.0);
                    }
                    let pill = UiRoundedRect::new(
                        x_px,
                        y_px,
                        w_px,
                        h_px,
                        radius,
                        tokens.surface_muted,
                        alpha,
                    );
                    self.stage_ui_rounded_rect(pill);
                    // Label text
                    let text_color = if is_hovered {
                        tokens.accent
                    } else {
                        tokens.text
                    };
                    self.draw_ai_text(
                        Point::new(lines.saturating_sub(1), Column(start)),
                        text_color,
                        tokens.surface_muted,
                        label,
                        wcols,
                    );
                    if start <= 2 {
                        break;
                    }
                    col_end = start.saturating_sub(gap);
                }
            }
        }

        // Provider dropdown overlay (inline), anchored above the composer pill
        if self.ai_provider_dropdown_open {
            use unicode_width::UnicodeWidthStr as _;
            let providers: &[(&str, &str)] = &[
                ("openrouter", "OpenRouter"),
                ("openai", "OpenAI"),
                ("anthropic", "Anthropic"),
                ("ollama", "Ollama"),
            ];
            // Place the overlay slightly above the pill with same width as its contents
            let mut col = 2 + star.len();
            let y_px = (lines.saturating_sub(2) as f32) * ch + (ch - (ch * 0.8)) * 0.5; // line above
            for (id, label) in providers.iter() {
                let chip = format!("[{}]", label);
                let wcols = chip.width();
                if col + wcols + 2 >= cols {
                    break;
                }
                let pad = ui
                    .composer_chip_pad_px
                    .unwrap_or(ui.palette_chip_pad_px)
                    .max(1.0);
                let x_px = (col as f32) * cw - pad;
                let h_px = ch * 0.8;
                let w_px = (wcols as f32) * cw + pad * 2.0;
                let radius = ui.palette_pill_radius_px.min(h_px * 0.5);
                let mut alpha = ui.composer_chip_alpha_unfocused;
                let is_selected = self.ai_current_provider.eq_ignore_ascii_case(id);
                if is_selected {
                    alpha = (alpha + 0.08).min(1.0);
                }
                // Hover state by mouse col on that line
                let hover_on_row = ((self.last_mouse_y as f32)
                    >= ((lines.saturating_sub(2) as f32) * ch))
                    && ((self.last_mouse_y as f32) < ((lines.saturating_sub(2) as f32) * ch + ch));
                let hover = hover_on_row
                    && hover_col_for_composer.is_some_and(|c| c >= col && c < col + wcols);
                if hover {
                    alpha = (alpha + ui.composer_chip_alpha_hover_delta).min(1.0);
                }
                if composer_press_flash && hover {
                    alpha = (alpha + ui.composer_chip_alpha_press_delta).min(1.0);
                }
                let pill =
                    UiRoundedRect::new(x_px, y_px, w_px, h_px, radius, tokens.surface, alpha);
                self.stage_ui_rounded_rect(pill);
                let text_color = if is_selected {
                    tokens.accent
                } else {
                    tokens.text
                };
                self.draw_ai_text(
                    Point::new(lines.saturating_sub(2), Column(col)),
                    text_color,
                    tokens.surface_muted,
                    &chip,
                    wcols,
                );
                col += wcols + ui.composer_chip_gap_cols as usize;
            }
        }
    }

    #[allow(dead_code)]
    fn renderer_draw_cells<I: Iterator<Item = RenderableCell>>(
        &mut self,
        size_info: &SizeInfo,
        glyph_cache: &mut GlyphCache,
        cells: I,
    ) {
        let Backend::Wgpu { renderer } = &mut self.backend;
        renderer.draw_cells(size_info, glyph_cache, cells)
    }

    #[allow(dead_code)]
    fn renderer_draw_string(
        &mut self,
        point: Point<usize>,
        fg: Rgb,
        bg: Rgb,
        string_chars: impl Iterator<Item = char>,
        size_info: &SizeInfo,
        glyph_cache: &mut GlyphCache,
    ) {
        let Backend::Wgpu { renderer } = &mut self.backend;
        renderer.draw_string(point, fg, bg, string_chars, size_info, glyph_cache)
    }

    /// Prepare the renderer to capture a screenshot of the next frame.
    /// WGPU: currently a no-op.
    #[allow(dead_code)]
    pub fn begin_screenshot(&mut self) {
        // no-op
    }

    /// Read the current frame's RGBA pixels. Returns (bytes, width, height) on success.
    /// WGPU: currently unsupported (returns None). Use external screen capture tools if needed.
    #[allow(dead_code)]
    pub fn read_frame_rgba(&mut self) -> Option<(Vec<u8>, u32, u32)> {
        None
    }

    #[allow(dead_code)]
    fn renderer_set_viewport(&self, _size: &SizeInfo) {
        // WGPU manages viewport via surface configuration
    }

    #[allow(dead_code)]
    fn renderer_with_loader<F: FnOnce(LoaderApi<'_>) -> T, T>(&mut self, func: F) -> T {
        // Provide a dummy loader API; WGPU path uploads glyphs differently.
        func(LoaderApi::new())
    }

    /// Make the graphics context not current.
    /// WGPU-only: no-op.
    #[allow(dead_code)]
    pub fn make_not_current(&mut self) {}

    fn draw_ime_preview(
        &mut self,
        point: Point<usize>,
        fg: Rgb,
        bg: Rgb,
        rects: &mut Vec<RenderRect>,
        config: &UiConfig,
    ) {
        let preedit = match self.ime.preedit() {
            Some(preedit) => preedit,
            None => {
                // In case we don't have preedit, just set the popup point.
                self.window.update_ime_position(point, &self.size_info);
                return;
            }
        };

        let num_cols = self.size_info.columns();

        // Get the visible preedit.
        let visible_text: String = match (preedit.cursor_byte_offset, preedit.cursor_end_offset) {
            (Some(byte_offset), Some(end_offset)) if end_offset.0 > num_cols => StrShortener::new(
                &preedit.text[byte_offset.0..],
                num_cols,
                ShortenDirection::Right,
                Some(SHORTENER),
            ),
            _ => StrShortener::new(
                &preedit.text,
                num_cols,
                ShortenDirection::Left,
                Some(SHORTENER),
            ),
        }
        .collect();

        let visible_len = visible_text.chars().count();

        let end = cmp::min(point.column.0 + visible_len, num_cols);
        let start = end.saturating_sub(visible_len);

        let start = Point::new(point.line, Column(start));
        let end = Point::new(point.line, Column(end - 1));

        let metrics = self.glyph_cache.font_metrics();

        // Draw preedit text using the active backend without borrowing Display twice.
        {
            let size_info_copy = self.size_info;
            let Backend::Wgpu { renderer } = &mut self.backend;
            renderer.draw_string(
                start,
                fg,
                bg,
                visible_text.chars(),
                &size_info_copy,
                &mut self.glyph_cache,
            );
        }

        // Damage preedit inside the terminal viewport.
        if point.line < self.size_info.screen_lines() {
            let damage = LineDamageBounds::new(start.line, 0, num_cols);
            self.damage_tracker.frame().damage_line(damage);
            self.damage_tracker.next_frame().damage_line(damage);
        }

        // Add underline for preedit text.
        let underline = RenderLine {
            start,
            end,
            color: fg,
        };
        rects.extend(underline.rects(Flags::UNDERLINE, &metrics, &self.size_info));

        let ime_popup_point = match preedit.cursor_end_offset {
            Some(cursor_end_offset) => {
                // Use hollow block when multiple characters are changed at once.
                let (shape, width) = if let Some(width) =
                    NonZeroU32::new((cursor_end_offset.0 - cursor_end_offset.1) as u32)
                {
                    (CursorShape::HollowBlock, width)
                } else {
                    (CursorShape::Beam, NonZeroU32::new(1).unwrap())
                };

                let cursor_column = Column(
                    (end.column.0 as isize - cursor_end_offset.0 as isize + 1).max(0) as usize,
                );
                let cursor_point = Point::new(point.line, cursor_column);
                let cursor = RenderableCursor::new(cursor_point, shape, fg, width);
                rects.extend(cursor.rects(&self.size_info, config.cursor.thickness()));
                cursor_point
            }
            _ => end,
        };

        self.window
            .update_ime_position(ime_popup_point, &self.size_info);
    }

    /// Format search regex to account for the cursor and fullwidth characters.
    fn format_search(search_regex: &str, search_label: &str, max_width: usize) -> String {
        let label_len = search_label.len();

        // Skip `search_regex` formatting if only label is visible.
        if label_len > max_width {
            return search_label[..max_width].to_owned();
        }

        // The search string consists of `search_label` + `search_regex` + `cursor`.
        let mut bar_text = String::from(search_label);
        bar_text.extend(StrShortener::new(
            search_regex,
            max_width.wrapping_sub(label_len + 1),
            ShortenDirection::Left,
            Some(SHORTENER),
        ));

        // Add place for cursor.
        bar_text.push(' ');

        bar_text
    }

    /// Draw preview for the currently highlighted `Hyperlink`.
    #[inline(never)]
    fn draw_hyperlink_preview(
        &mut self,
        config: &UiConfig,
        cursor_point: Option<Point>,
        display_offset: usize,
    ) {
        let num_cols = self.size_info.columns();
        let uris: Vec<_> = self
            .highlighted_hint
            .iter()
            .chain(&self.vi_highlighted_hint)
            .filter_map(|hint| hint.hyperlink().map(|hyperlink| hyperlink.uri()))
            .map(|uri| StrShortener::new(uri, num_cols, ShortenDirection::Right, Some(SHORTENER)))
            .collect();

        if uris.is_empty() {
            return;
        }

        // The maximum amount of protected lines including the ones we'll show preview on.
        let max_protected_lines = uris.len() * 2;

        // Lines we shouldn't show preview on, because it'll obscure the highlighted hint.
        let mut protected_lines = Vec::with_capacity(max_protected_lines);
        if self.size_info.screen_lines() > max_protected_lines {
            // Prefer to show preview even when it'll likely obscure the highlighted hint, when
            // there's no place left for it.
            protected_lines.push(self.hint_mouse_point.map(|point| point.line));
            protected_lines.push(cursor_point.map(|point| point.line));
        }

        // Find the line in viewport we can draw preview on without obscuring protected lines.
        let viewport_bottom = self.size_info.bottommost_line() - Line(display_offset as i32);
        let viewport_top = viewport_bottom - (self.size_info.screen_lines() - 1);
        let uri_lines = (viewport_top.0..=viewport_bottom.0)
            .rev()
            .map(|line| Some(Line(line)))
            .filter_map(|line| {
                if protected_lines.contains(&line) {
                    None
                } else {
                    protected_lines.push(line);
                    line
                }
            })
            .take(uris.len())
            .flat_map(|line| term::point_to_viewport(display_offset, Point::new(line, Column(0))));

        let theme = config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| config.theme.resolve());
        let fg = theme.tokens.text;
        let bg = theme.tokens.surface_muted;
        for (uri, point) in uris.into_iter().zip(uri_lines) {
            // Damage the uri preview.
            let damage = LineDamageBounds::new(point.line, point.column.0, num_cols);
            self.damage_tracker.frame().damage_line(damage);

            // Damage the uri preview for the next frame as well.
            self.damage_tracker.next_frame().damage_line(damage);

            {
                let size_info_copy = self.size_info;
                let uri_string: String = uri.collect();
                let Backend::Wgpu { renderer } = &mut self.backend;
                renderer.draw_string(
                    point,
                    fg,
                    bg,
                    uri_string.chars(),
                    &size_info_copy,
                    &mut self.glyph_cache,
                );
            }
        }
    }

    /// Draw current search regex.
    #[inline(never)]
    fn draw_search(&mut self, config: &UiConfig, text: &str) {
        // Assure text length is at least num_cols.
        let num_cols = self.size_info.columns();
        let text = format!("{text:<num_cols$}");

        let point = Point::new(self.size_info.screen_lines(), Column(0));

        let theme = config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| config.theme.resolve());
        let (fg, bg) = match config.debug.render_timer_style {
            crate::config::debug::RenderTimerStyle::LowContrast => {
                (theme.tokens.text, theme.tokens.surface_muted)
            }
            crate::config::debug::RenderTimerStyle::Warning => {
                (theme.tokens.surface, theme.tokens.warning)
            }
        };

        {
            let size_info_copy = self.size_info;
            let Backend::Wgpu { renderer } = &mut self.backend;
            renderer.draw_string(
                point,
                fg,
                bg,
                text.chars(),
                &size_info_copy,
                &mut self.glyph_cache,
            );
        }
    }

    /// Draw render timer.
    #[inline(never)]
    fn draw_render_timer(&mut self, config: &UiConfig) {
        if !config.debug.render_timer {
            return;
        }

        let timing = format!("{:.3} usec", self.meter.average());
        let point = Point::new(self.size_info.screen_lines().saturating_sub(2), Column(0));

        let theme = config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| config.theme.resolve());
        let fg = theme.tokens.text;
        let bg = theme.tokens.surface_muted;

        // Damage render timer for current and next frame.
        let damage = LineDamageBounds::new(point.line, point.column.0, timing.len());
        self.damage_tracker.frame().damage_line(damage);
        self.damage_tracker.next_frame().damage_line(damage);

        {
            let size_info_copy = self.size_info;
            let Backend::Wgpu { renderer } = &mut self.backend;
            renderer.draw_string(
                point,
                fg,
                bg,
                timing.chars(),
                &size_info_copy,
                &mut self.glyph_cache,
            );
        }
    }

    /// Draw an indicator for the position of a line in history.
    #[inline(never)]
    fn draw_line_indicator(
        &mut self,
        config: &UiConfig,
        total_lines: usize,
        obstructed_column: Option<Column>,
        line: usize,
    ) {
        let columns = self.size_info.columns();
        let text = format!("[{}/{}]", line, total_lines - 1);
        let column = Column(self.size_info.columns().saturating_sub(text.len()));
        let point = Point::new(0, column);

        // Damage the line indicator for current and next frame.
        let damage = LineDamageBounds::new(point.line, point.column.0, columns - 1);
        self.damage_tracker.frame().damage_line(damage);
        self.damage_tracker.next_frame().damage_line(damage);

        let theme = config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| config.theme.resolve());
        let fg = theme.tokens.text;
        let bg = theme.tokens.surface_muted;

        // Do not render anything if it would obscure the vi mode cursor.
        if obstructed_column.map_or(true, |obstructed_column| obstructed_column < column) {
            {
                let size_info_copy = self.size_info;
                let Backend::Wgpu { renderer } = &mut self.backend;
                renderer.draw_string(
                    point,
                    fg,
                    bg,
                    text.chars(),
                    &size_info_copy,
                    &mut self.glyph_cache,
                );
            }
        }
    }

    /// Highlight damaged rects.
    ///
    /// This function is for debug purposes only.
    fn highlight_damage(&self, render_rects: &mut Vec<RenderRect>) {
        for damage_rect in &self
            .damage_tracker
            .shape_frame_damage(self.size_info.into())
        {
            let x = damage_rect.x as f32;
            let height = damage_rect.height as f32;
            let width = damage_rect.width as f32;
            let y = damage_y_to_viewport_y(&self.size_info, damage_rect) as f32;
            let render_rect = RenderRect::new(x, y, width, height, DAMAGE_RECT_COLOR, 0.5);

            render_rects.push(render_rect);
        }
    }

    /// Check whether a hint highlight needs to be cleared.
    fn validate_hint_highlights(&mut self, display_offset: usize) {
        let frame = self.damage_tracker.frame();
        let hints = [
            (
                &mut self.highlighted_hint,
                &mut self.highlighted_hint_age,
                true,
            ),
            (
                &mut self.vi_highlighted_hint,
                &mut self.vi_highlighted_hint_age,
                false,
            ),
        ];

        let num_lines = self.size_info.screen_lines();
        for (hint, hint_age, reset_mouse) in hints {
            let (start, end) = match hint {
                Some(hint) => (*hint.bounds().start(), *hint.bounds().end()),
                None => continue,
            };

            // Ignore hints that were created this frame.
            *hint_age += 1;
            if *hint_age == 1 {
                continue;
            }

            // Convert hint bounds to viewport coordinates.
            let start = term::point_to_viewport(display_offset, start)
                .filter(|point| point.line < num_lines)
                .unwrap_or_default();
            let end = term::point_to_viewport(display_offset, end)
                .filter(|point| point.line < num_lines)
                .unwrap_or_else(|| Point::new(num_lines - 1, self.size_info.last_column()));

            // Clear invalidated hints.
            if frame.intersects(start, end) {
                if reset_mouse {
                    self.window.set_mouse_cursor(CursorIcon::Default);
                }
                frame.mark_fully_damaged();
                *hint = None;
            }
        }
    }

    /// Request a new frame for a window on Wayland.
    fn request_frame(&mut self, scheduler: &mut Scheduler) {
        // Mark that we've used a frame.
        self.window.has_frame = false;

        // Get the display vblank interval.
        let monitor_vblank_interval = 1_000_000.
            / self
                .window
                .current_monitor()
                .and_then(|monitor| monitor.refresh_rate_millihertz())
                .unwrap_or(60_000) as f64;

        // Now convert it to micro seconds.
        let monitor_vblank_interval =
            Duration::from_micros((1000. * monitor_vblank_interval) as u64);

        let swap_timeout = self.frame_timer.compute_timeout(monitor_vblank_interval);

        let window_id = self.window.id();
        let timer_id = TimerId::new(Topic::Frame, window_id);
        let event = Event::new(EventType::Frame, window_id);

        // Coalesce any previously scheduled frame for this window before scheduling a new one.
        let coalesced = scheduler.unschedule(timer_id).is_some();
        log::debug!(
            "request_frame: coalesced={} delay_ms={}",
            coalesced,
            swap_timeout.as_millis()
        );
        scheduler.schedule(event, swap_timeout, false, timer_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_bar_overlay_visibility_always() {
        let si = SizeInfo::new(800.0, 600.0, 8.0, 16.0, 0.0, 0.0, false);
        let mut cfg = crate::config::workspace::TabBarConfig::default();
        cfg.position = crate::workspace::TabBarPosition::Top;
        cfg.visibility = crate::config::workspace::TabBarVisibility::Always;
        let style = crate::display::warp_ui::WarpTabStyle::default();
        // Far from the top; should still show because Always
        assert!(should_show_tab_bar_overlay(si, 400, &cfg, false, &style));
    }

    #[test]
    fn tab_bar_overlay_visibility_hover_top_and_bottom() {
        let si = SizeInfo::new(1000.0, 700.0, 10.0, 20.0, 0.0, 0.0, false);
        let mut cfg = crate::config::workspace::TabBarConfig::default();
        let style = crate::display::warp_ui::WarpTabStyle::default();

        // Top position
        cfg.position = crate::workspace::TabBarPosition::Top;
        cfg.visibility = crate::config::workspace::TabBarVisibility::Hover;
        // Near top should show
        assert!(should_show_tab_bar_overlay(si, 5, &cfg, false, &style));
        // Far from top should hide
        assert!(!should_show_tab_bar_overlay(si, 300, &cfg, false, &style));

        // Bottom position
        cfg.position = crate::workspace::TabBarPosition::Bottom;
        // Near bottom should show
        assert!(should_show_tab_bar_overlay(si, (si.height() as usize) - 2, &cfg, false, &style));
        // Far from bottom should hide
        assert!(!should_show_tab_bar_overlay(si, 100, &cfg, false, &style));
    }

    #[test]
    fn tab_bar_overlay_visibility_auto_respects_fullscreen() {
        let si = SizeInfo::new(900.0, 600.0, 9.0, 18.0, 0.0, 0.0, false);
        let mut cfg = crate::config::workspace::TabBarConfig::default();
        cfg.position = crate::workspace::TabBarPosition::Top;
        cfg.visibility = crate::config::workspace::TabBarVisibility::Auto;
        let style = crate::display::warp_ui::WarpTabStyle::default();

        // Not fullscreen -> treated as Always
        assert!(should_show_tab_bar_overlay(si, 400, &cfg, false, &style));
        // Fullscreen -> treated as Hover (far from top should hide, near top should show)
        assert!(!should_show_tab_bar_overlay(si, 400, &cfg, true, &style));
        assert!(should_show_tab_bar_overlay(si, 3, &cfg, true, &style));
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        match &mut self.backend {
            Backend::Wgpu { .. } => {
                // WGPU resources drop automatically.
            }
        }
    }
}

/// Input method state.
#[derive(Debug, Default)]
pub struct Ime {
    /// Whether the IME is enabled.
    enabled: bool,

    /// Current IME preedit.
    preedit: Option<Preedit>,
}

impl Ime {
    #[inline]
    pub fn set_enabled(&mut self, is_enabled: bool) {
        if is_enabled {
            self.enabled = is_enabled
        } else {
            // Clear state when disabling IME.
            *self = Default::default();
        }
    }

    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[inline]
    pub fn set_preedit(&mut self, preedit: Option<Preedit>) {
        self.preedit = preedit;
    }

    #[inline]
    pub fn preedit(&self) -> Option<&Preedit> {
        self.preedit.as_ref()
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Preedit {
    /// The preedit text.
    text: String,

    /// Byte offset for cursor start into the preedit text.
    ///
    /// `None` means that the cursor is invisible.
    cursor_byte_offset: Option<(usize, usize)>,

    /// The cursor offset from the end of the start of the preedit in char width.
    cursor_end_offset: Option<(usize, usize)>,
}

impl Preedit {
    pub fn new(text: String, cursor_byte_offset: Option<(usize, usize)>) -> Self {
        let cursor_end_offset = if let Some(byte_offset) = cursor_byte_offset {
            // Convert byte offset into char offset.
            let start_to_end_offset = text[byte_offset.0..]
                .chars()
                .fold(0, |acc, ch| acc + ch.width().unwrap_or(1));
            let end_to_end_offset = text[byte_offset.1..]
                .chars()
                .fold(0, |acc, ch| acc + ch.width().unwrap_or(1));

            Some((start_to_end_offset, end_to_end_offset))
        } else {
            None
        };

        Self {
            text,
            cursor_byte_offset,
            cursor_end_offset,
        }
    }
}

/// Pending renderer updates.
///
/// All renderer updates are cached to be applied just before rendering, to avoid platform-specific
/// rendering issues.
#[derive(Debug, Default, Copy, Clone)]
pub struct RendererUpdate {
    /// Should resize the window.
    resize: bool,

    /// Clear font caches.
    clear_font_cache: bool,
}

/// The frame timer state.
pub struct FrameTimer {
    /// Base timestamp used to compute sync points.
    base: Instant,

    /// The last timestamp we synced to.
    last_synced_timestamp: Instant,

    /// The refresh rate we've used to compute sync timestamps.
    refresh_interval: Duration,
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameTimer {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            base: now,
            last_synced_timestamp: now,
            refresh_interval: Duration::ZERO,
        }
    }

    /// Compute the delay that we should use to achieve the target frame
    /// rate.
    pub fn compute_timeout(&mut self, refresh_interval: Duration) -> Duration {
        let now = Instant::now();

        // Handle refresh rate change.
        if self.refresh_interval != refresh_interval {
            self.base = now;
            self.last_synced_timestamp = now;
            self.refresh_interval = refresh_interval;
            return refresh_interval;
        }

        let next_frame = self.last_synced_timestamp + self.refresh_interval;

        if next_frame < now {
            // Redraw immediately if we haven't drawn in over `refresh_interval` microseconds.
            let elapsed_micros = (now - self.base).as_micros() as u64;
            let refresh_micros = self.refresh_interval.as_micros() as u64;
            self.last_synced_timestamp =
                now - Duration::from_micros(elapsed_micros % refresh_micros);
            Duration::ZERO
        } else {
            // Redraw on the next `refresh_interval` clock tick.
            self.last_synced_timestamp = next_frame;
            next_frame - now
        }
    }
}

/// Calculate the cell dimensions based on font metrics.
///
/// This will return a tuple of the cell width and height.
#[inline]
fn compute_cell_size(config: &UiConfig, metrics: &crossfont::Metrics) -> (f32, f32) {
    let offset_x = f64::from(config.font.offset.x);
    let offset_y = f64::from(config.font.offset.y);
    (
        (metrics.average_advance + offset_x).floor().max(1.) as f32,
        (metrics.line_height + offset_y).floor().max(1.) as f32,
    )
}

/// Calculate the size of the window given padding, terminal dimensions and cell size.
fn window_size(
    config: &UiConfig,
    dimensions: Dimensions,
    cell_width: f32,
    cell_height: f32,
    scale_factor: f32,
) -> PhysicalSize<u32> {
    let padding = config.window.padding(scale_factor);

    let grid_width = cell_width * dimensions.columns.max(MIN_COLUMNS) as f32;
    let grid_height = cell_height * dimensions.lines.max(MIN_SCREEN_LINES) as f32;

    let width = (padding.0).mul_add(2., grid_width).floor();
    let height = (padding.1).mul_add(2., grid_height).floor();

    PhysicalSize::new(width as u32, height as u32)
}
