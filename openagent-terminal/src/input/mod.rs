//! Handle input from winit.
//!
//! Certain key combinations should send some escape sequence back to the PTY.
//! In order to figure that out, state about which modifier keys are pressed
//! needs to be tracked. Additionally, we need a bit of a state machine to
//! determine what to do when a non-modifier key is pressed.

use std::borrow::Cow;
use std::cmp::{max, min, Ordering};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::mem;
use std::time::{Duration, Instant};

use log::debug;
use winit::dpi::PhysicalPosition;
use winit::event::{
    ElementState, Modifiers, MouseButton, MouseScrollDelta, Touch as TouchEvent, TouchPhase,
};
#[cfg(target_os = "macos")]
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::ModifiersState;
#[cfg(target_os = "macos")]
use winit::platform::macos::ActiveEventLoopExtMacOS;
use winit::window::CursorIcon;

use openagent_terminal_core::event::EventListener;
use openagent_terminal_core::grid::{Dimensions, Scroll};
use openagent_terminal_core::index::{Boundary, Column, Direction, Point, Side};
use openagent_terminal_core::selection::SelectionType;
use openagent_terminal_core::term::search::Match;
use openagent_terminal_core::term::{ClipboardType, Term, TermMode};
use openagent_terminal_core::vi_mode::ViMotion;
use openagent_terminal_core::vte::ansi::{ClearMode, Handler};

use crate::clipboard::Clipboard;
#[cfg(target_os = "macos")]
use crate::config::window::Decorations;
use crate::config::{Action, BindingMode, MouseAction, SearchAction, UiConfig, ViAction};
use crate::display::hint::HintMatch;
use crate::display::window::Window;
use crate::display::{Display, SizeInfo};
use crate::event::{
    ClickState, Event, EventType, InlineSearchState, Mouse, TouchPurpose, TouchZoom,
};
use crate::message_bar::{self, Message};
use crate::scheduler::{Scheduler, TimerId, Topic};
use crate::security::{RiskLevel, SecurityLens};

// Preview sanitization helpers for paste/confirm dialogs
fn strip_ansi(input: &str) -> String {
    // Remove a subset of ANSI escape sequences (CSI and OSC common forms)
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1B {
            // ESC sequence: try to skip CSI "\x1b[...<ender>"
            if i + 1 < bytes.len() {
                let next = bytes[i + 1];
                if next == b'[' {
                    // Skip until a byte in 0x40..=0x7E
                    i += 2;
                    while i < bytes.len() {
                        let b = bytes[i];
                        if (0x40..=0x7E).contains(&b) {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                    continue;
                } else if next == b']' {
                    // OSC: ESC ] ... BEL or ESC \
                    i += 2;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        // ESC \
                        if bytes[i] == 0x1B && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                    continue;
                }
            }
            // Fallback: skip the ESC only
            i += 1;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn redact_line(mut line: String) -> String {
    // Optional extended redaction via env flag: OPENAGENT_PRIVACY_EXTENDED=1
    let extended_env = std::env::var("OPENAGENT_PRIVACY_EXTENDED")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    // Prefer config flag when available
    let extended_cfg = crate::event::Processor::privacy_extended_flag();
    let extended = extended_cfg.unwrap_or(extended_env);
    let lower = line.to_lowercase();
    let keywords = ["api_key", "apikey", "token", "secret", "password", "passwd", "authorization", "auth"];    
    let mut redacted = false;

    // Authorization: Bearer ...
    if let Some(pos) = lower.find("bearer ") {
        // Replace everything after 'Bearer '
        let cut = pos + "bearer ".len();
        if cut <= line.len() {
            // Find end of token (to end of line)
            line.replace_range(cut.., "{{REDACTED}}");
            redacted = true;
        }
    }

    // If already redacted by Bearer rule, skip generic key-value pass to avoid double redaction
    if redacted {
        return line;
    }

    // Extended patterns (AWS/JWT) when enabled
    if extended {
        // Very rough AWS Access Key ID pattern: AKIA or ASIA followed by 16 alnum
        if let Some(idx) = lower.find("akia") { // case-insensitive via lower
            let start = idx;
            let end = (start + 20).min(line.len());
            line.replace_range(start..end, "{{REDACTED_AWS_KEY}}");
            return line;
        }
        if let Some(idx) = lower.find("asia") {
            let start = idx;
            let end = (start + 20).min(line.len());
            line.replace_range(start..end, "{{REDACTED_AWS_KEY}}");
            return line;
        }
        // JWT-like: three base64url-ish segments separated by dots; replace the middle+signature
        // We only look for two dots to keep it simple
        if let Some(first_dot) = line.find('.') {
            if let Some(second_dot) = line[first_dot + 1..].find('.') {
                let mid_start = first_dot + 1;
                let mid_end = mid_start + second_dot + 1; // include the second dot
                // Replace middle and signature segment with marker
                line.replace_range(mid_start..line.len(), "{{REDACTED_JWT}}");
                return line;
            }
        }
    }

    // Key-value secrets (keyword followed by ':' or '=')
    for kw in keywords.iter() {
        if let Some(kw_pos) = lower.find(kw) {
            // Find separator after keyword
            let after_kw = kw_pos + kw.len();
            // Search for ':' or '=' after keyword
            let mut sep_idx: Option<usize> = None;
            for (i, ch) in line.char_indices().skip(after_kw) {
                if ch == ':' || ch == '=' {
                    sep_idx = Some(i);
                    break;
                }
            }
            if let Some(sep) = sep_idx {
                // Skip whitespace after separator
                let mut val_start = sep + 1;
                while val_start < line.len() {
                    if let Some(c) = line[val_start..].chars().next() {
                        if c.is_whitespace() { val_start += c.len_utf8(); } else { break; }
                    } else {
                        break;
                    }
                }
                if val_start < line.len() {
                    line.replace_range(val_start.., "{{REDACTED}}");
                    redacted = true;
                    break;
                }
            }
        }
    }

    if redacted { line } else { line }
}

fn sanitize_preview(text: &str, max_lines: usize, max_chars: usize) -> String {
    let mut clean = strip_ansi(text);
    // Normalize CRLF -> LF to avoid double counting
    clean = clean.replace("\r\n", "\n");
    let mut out = String::new();
    let mut chars_budget = max_chars;
    for (i, raw) in clean.lines().enumerate() {
        if i >= max_lines || chars_budget == 0 { break; }
        let red = redact_line(raw.to_string());
        let take = red.chars().take(chars_budget).collect::<String>();
        out.push_str(&take);
        out.push('\n');
        let used = take.chars().count() + 1; // +1 for newline
        if chars_budget >= used { chars_budget -= used; } else { chars_budget = 0; }
    }
    if out.ends_with('\n') { let _ = out.pop(); }
    if chars_budget == 0 || clean.lines().count() > max_lines {
        if !out.ends_with('…') { out.push_str("…"); }
    }
    out
}

pub mod keyboard;

/// Font size change interval in px.
pub const FONT_SIZE_STEP: f32 = 1.;

/// Interval for mouse scrolling during selection outside of the boundaries.
const SELECTION_SCROLLING_INTERVAL: Duration = Duration::from_millis(15);

/// Minimum number of pixels at the bottom/top where selection scrolling is performed.
const MIN_SELECTION_SCROLLING_HEIGHT: f64 = 5.;

/// Number of pixels for increasing the selection scrolling speed factor by one.
const SELECTION_SCROLLING_STEP: f64 = 20.;

/// Distance before a touch input is considered a drag.
const MAX_TAP_DISTANCE: f64 = 20.;

/// Threshold used for double_click/triple_click.
const CLICK_THRESHOLD: Duration = Duration::from_millis(400);

/// Processes input from winit.
///
/// An escape sequence may be emitted in case specific keys or key combinations
/// are activated.
pub struct Processor<T: EventListener, A: ActionContext<T>> {
    pub ctx: A,
    _phantom: PhantomData<T>,
}

#[allow(dead_code)]
pub trait ActionContext<T: EventListener> {
    fn write_to_pty<B: Into<Cow<'static, [u8]>>>(&self, _data: B) {}
    fn mark_dirty(&mut self) {}
    fn size_info(&self) -> SizeInfo;
    fn copy_selection(&mut self, _ty: ClipboardType) {}
    fn start_selection(&mut self, _ty: SelectionType, _point: Point, _side: Side) {}
    fn toggle_selection(&mut self, _ty: SelectionType, _point: Point, _side: Side) {}
    fn update_selection(&mut self, _point: Point, _side: Side) {}
    fn clear_selection(&mut self) {}
    fn selection_is_empty(&self) -> bool;
    fn mouse_mut(&mut self) -> &mut Mouse;
    fn mouse(&self) -> &Mouse;
    fn touch_purpose(&mut self) -> &mut TouchPurpose;
    fn modifiers(&mut self) -> &mut Modifiers;
    fn scroll(&mut self, _scroll: Scroll) {}
    fn window(&mut self) -> &mut Window;
    fn display(&mut self) -> &mut Display;
    fn terminal(&self) -> &Term<T>;
    fn terminal_mut(&mut self) -> &mut Term<T>;
    fn spawn_new_instance(&mut self) {}
    #[cfg(target_os = "macos")]
    fn create_new_window(&mut self, _tabbing_id: Option<String>) {}
    #[cfg(not(target_os = "macos"))]
    fn create_new_window(&mut self) {}
    fn change_font_size(&mut self, _delta: f32) {}
    fn reset_font_size(&mut self) {}
    fn pop_message(&mut self) {}
    fn message(&self) -> Option<&Message>;
    fn config(&self) -> &UiConfig;
    #[cfg(target_os = "macos")]
    fn event_loop(&self) -> &ActiveEventLoop;
    fn mouse_mode(&self) -> bool;
    fn clipboard_mut(&mut self) -> &mut Clipboard;
    fn scheduler_mut(&mut self) -> &mut Scheduler;
    fn start_search(&mut self, _direction: Direction) {}
    fn start_seeded_search(&mut self, _direction: Direction, _text: String) {}
    fn confirm_search(&mut self) {}
    fn cancel_search(&mut self) {}
    fn search_input(&mut self, _c: char) {}
    fn search_pop_word(&mut self) {}
    fn search_history_previous(&mut self) {}
    fn search_history_next(&mut self) {}
    fn search_next(&mut self, origin: Point, direction: Direction, side: Side) -> Option<Match>;
    fn advance_search_origin(&mut self, _direction: Direction) {}
    fn search_direction(&self) -> Direction;
    fn search_active(&self) -> bool;
    fn on_typing_start(&mut self) {}
    fn toggle_vi_mode(&mut self) {}
    fn inline_search_state(&mut self) -> &mut InlineSearchState;
    fn start_inline_search(&mut self, _direction: Direction, _stop_short: bool) {}
    fn inline_search_next(&mut self) {}
    fn inline_search_input(&mut self, _text: &str) {}
    fn inline_search_previous(&mut self) {}
    fn hint_input(&mut self, _character: char) {}
    fn trigger_hint(&mut self, _hint: &HintMatch) {}
    fn expand_selection(&mut self) {}
    fn semantic_word(&self, point: Point) -> String;
    fn on_terminal_input_start(&mut self) {}
    fn paste(&mut self, _text: &str, _bracketed: bool) {}
    fn spawn_daemon<I, S>(&self, _program: &str, _args: I)
    where
        I: IntoIterator<Item = S> + Debug + Copy,
        S: AsRef<OsStr>,
    {
    }
    fn copy_to_clipboard(&mut self, _text: String) {}
    fn spawn_shell_command_in_cwd(&mut self, _cmd: String, _cwd: String) {}
    fn prompt_and_export_block_output(&mut self, _text: String) {}

    // Inline AI suggestions (feature = "ai")
    #[cfg(feature = "ai")]
    fn inline_suggestion_visible(&self) -> bool {
        false
    }
    #[cfg(feature = "ai")]
    fn accept_inline_suggestion(&mut self) {}
    #[cfg(feature = "ai")]
    fn accept_inline_suggestion_word(&mut self) {}
    #[cfg(feature = "ai")]
    fn accept_inline_suggestion_char(&mut self) {}
    #[cfg(feature = "ai")]
    fn dismiss_inline_suggestion(&mut self) {}
    #[cfg(feature = "ai")]
    fn schedule_inline_suggest(&mut self) {}
    #[cfg(feature = "ai")]
    fn clear_inline_suggestion(&mut self) {}

    // Command palette API
    fn open_command_palette(&mut self) {}
    fn palette_active(&self) -> bool {
        false
    }
    fn palette_input(&mut self, _c: char) {}
    fn palette_backspace(&mut self) {}
    fn palette_move_selection(&mut self, _delta: isize) {}
    fn palette_confirm(&mut self) {}
    fn palette_cancel(&mut self) {}
    fn palette_confirm_cd(&mut self) {}
    fn run_workflow_by_name(&mut self, _name: &str) {}

    // Blocks Search panel - basic functionality
    fn open_blocks_search_panel(&mut self) {}
    fn close_blocks_search_panel(&mut self) {}
    fn blocks_search_active(&self) -> bool {
        false
    }

    // File Tree overlay controls
    fn open_file_tree_panel(&mut self) {}
    fn close_file_tree_panel(&mut self) {}
    fn file_tree_active(&self) -> bool {
        false
    }
    fn file_tree_move_selection(&mut self, _delta: isize) {}
    fn file_tree_confirm(&mut self) {}
    fn blocks_search_input(&mut self, _c: char) {}
    fn blocks_search_backspace(&mut self) {}
    fn blocks_search_move_selection(&mut self, _delta: isize) {}
    fn blocks_search_confirm(&mut self) {}
    fn blocks_search_cancel(&mut self) {}

    // Blocks Search panel - enhanced functionality
    fn blocks_search_cycle_mode(&mut self) {}
    fn blocks_search_cycle_sort_field(&mut self) {}
    fn blocks_search_toggle_sort_order(&mut self) {}
    fn blocks_search_toggle_starred(&mut self) {}
    fn blocks_search_clear_filters(&mut self) {}
    fn blocks_search_next_page(&mut self) {}
    fn blocks_search_prev_page(&mut self) {}
    fn blocks_search_toggle_star_selected(&mut self) {}
    fn blocks_search_show_actions(&mut self) {}
    fn blocks_search_delete_selected(&mut self) {}
    fn blocks_search_copy_command(&mut self) {}
    fn blocks_search_copy_output(&mut self) {}
    fn blocks_search_rerun_selected(&mut self) {}
    fn blocks_search_insert_heredoc(&mut self) {}
    fn blocks_search_insert_heredoc_custom(&mut self) {}
    fn blocks_search_insert_json_heredoc(&mut self) {}
    fn blocks_search_insert_shell_heredoc(&mut self) {}
    fn blocks_search_show_help(&mut self) {}
    fn blocks_search_export_selected(&mut self) {}
    fn blocks_search_toggle_tag(&mut self) {}
    fn blocks_search_copy_both(&mut self) {}
    fn blocks_search_insert_command(&mut self) {}
    fn blocks_search_view_output(&mut self) {}
    fn blocks_search_share_block(&mut self) {}
    fn blocks_search_create_snippet(&mut self) {}

    // Blocks Search panel - actions menu support
    fn blocks_search_actions_menu_active(&self) -> bool {
        false
    }
    fn blocks_search_execute_action(&mut self) {}
    fn blocks_search_close_actions_menu(&mut self) {}
    fn blocks_search_move_actions_selection(&mut self, _delta: isize) {}

    // Blocks Search panel - help overlay support
    fn blocks_search_help_active(&self) -> bool {
        false
    }
    fn blocks_search_close_help(&mut self) {}
    fn blocks_search_navigate_help(&mut self, _forward: bool) {}

    // Workflows panel (feature="workflow"). Default to no-op/false when disabled.
    fn open_workflows_panel(&mut self) {}

    // Notebooks panel controls (feature = "blocks")
    #[cfg(feature = "blocks")]
    fn open_notebooks_panel(&mut self) {}
    #[cfg(feature = "blocks")]
    fn notebooks_panel_active(&self) -> bool {
        false
    }
    #[cfg(feature = "blocks")]
    fn notebooks_panel_close(&mut self) {}
    #[cfg(feature = "blocks")]
    fn notebooks_panel_move_selection(&mut self, _delta: isize) {}
    #[cfg(feature = "blocks")]
    fn notebooks_panel_confirm(&mut self) {}
    #[cfg(feature = "blocks")]
    fn notebooks_panel_run_all(&mut self) {}
    #[cfg(feature = "blocks")]
    fn notebooks_panel_focus_next(&mut self) {}
    #[cfg(feature = "blocks")]
    fn notebooks_panel_focus_prev(&mut self) {}
    #[cfg(feature = "blocks")]
    fn notebooks_panel_rerun_selected(&mut self) {}
    #[cfg(feature = "blocks")]
    fn notebooks_panel_add_command_cell(&mut self) {}
    #[cfg(feature = "blocks")]
    fn notebooks_panel_add_markdown_cell(&mut self) {}

    // Settings panel controls
    fn open_settings_panel(&mut self) {}
    fn close_settings_panel(&mut self) {}
    fn settings_panel_active(&self) -> bool {
        false
    }
    fn settings_panel_input(&mut self, _c: char) {}
    fn settings_panel_backspace(&mut self) {}
    fn settings_panel_next_field(&mut self) {}
    fn settings_panel_prev_field(&mut self) {}
    fn settings_panel_cycle_provider(&mut self, _forward: bool) {}
    fn settings_panel_save(&mut self) {}
    fn settings_panel_switch_category(&mut self, _forward: bool) {}
    fn settings_panel_test_connection(&mut self) {}
    fn settings_panel_move_selection(&mut self, _delta: isize) {}
    fn settings_panel_begin_capture(&mut self) {}
    fn settings_panel_cancel_capture(&mut self) {}
    fn settings_panel_is_capturing(&self) -> bool {
        false
    }
    fn settings_panel_capture(
        &mut self,
        _key: winit::keyboard::Key<String>,
        _mods: ModifiersState,
    ) {
    }
    fn workflows_panel_cancel(&mut self) {}
    fn workflows_panel_confirm(&mut self) {}
    fn workflows_panel_input(&mut self, _c: char) {}
    fn workflows_panel_backspace(&mut self) {}
    fn workflows_panel_move_selection(&mut self, _delta: isize) {}
    fn workflows_panel_active(&self) -> bool {
        false
    }

    // Workflows progress overlay controls
    fn workflows_progress_active(&self) -> bool {
        false
    }
    fn workflows_progress_dismiss(&mut self) {}
    fn workflows_progress_terminal(&self) -> bool {
        false
    }

    // Confirm overlay
    fn confirm_overlay_active(&self) -> bool {
        false
    }
    fn confirm_overlay_confirm(&mut self) {}
    fn confirm_overlay_cancel(&mut self) {}

    // AI panel (feature=ai)
    fn open_ai_panel(&mut self) {}
    fn close_ai_panel(&mut self) {}
    fn ai_active(&self) -> bool {
        false
    }
    fn ai_input(&mut self, _c: char) {}
    fn ai_backspace(&mut self) {}
    fn ai_propose(&mut self) {}

    // Return true if a header control click was handled (AI panel)
    fn ai_try_handle_header_click(&mut self) -> bool {
        false
    }
    // Update AI header hover state; return true if hovering a control (for pointer cursor)
    fn ai_update_hover_header(&mut self) -> bool {
        false
    }

    // UI event dispatch (e.g., to event loop proxy); default no-op in tests
    fn send_user_event(&self, _event: crate::event::EventType) {}

    // AI runtime accessors; default to None
    #[cfg(feature = "ai")]
    fn ai_runtime_mut(&mut self) -> Option<&mut crate::ai_runtime::AiRuntime> {
        None
    }
    #[cfg(feature = "ai")]
    fn ai_runtime_ref(&self) -> Option<&crate::ai_runtime::AiRuntime> {
        None
    }

    // Workspace / panes (placeholders; real implementation lives in event::ActionContext)
    fn workspace_split_horizontal(&mut self) {}
    fn workspace_split_vertical(&mut self) {}
    fn workspace_focus_next_pane(&mut self) {}
    fn workspace_focus_previous_pane(&mut self) {}
    fn workspace_close_pane(&mut self) {}
    fn workspace_resize_left(&mut self) {}
    fn workspace_resize_right(&mut self) {}
    fn workspace_resize_up(&mut self) {}
    fn workspace_resize_down(&mut self) {}

    // Tab management (placeholders; real implementation lives in event::ActionContext)
    fn workspace_create_tab(&mut self) {}
    fn workspace_close_tab(&mut self) {}
    fn workspace_next_tab(&mut self) {}
    fn workspace_previous_tab(&mut self) {}
    fn workspace_switch_to_tab(&mut self, _tab_id: crate::workspace::TabId) {}

    // Toggle zoom of active pane in active tab
    fn workspace_toggle_zoom(&mut self) {}

    // Workspace tab helpers
    fn workspace_mark_active_tab_error(&mut self, _non_zero: bool) {}

    // Toggle sync for active tab
    fn workspace_toggle_sync(&mut self) {}

    // Workspace tab bar hit testing (handled in event::ActionContext)
    fn workspace_tab_bar_hit(
        &mut self,
        _mouse_x: usize,
        _mouse_y: usize,
    ) -> Option<crate::display::warp_ui::TabBarAction> {
        None
    }

    /// Hit-test split divider (input coords in pixels), returns info if hovering a divider.
    fn workspace_split_hit(
        &mut self,
        _mouse_x_px: f32,
        _mouse_y_px: f32,
        _tolerance_px: f32,
    ) -> Option<crate::workspace::split_manager::SplitDividerHit> {
        None
    }

    // Tab bar drag-and-drop helpers (handled in event::ActionContext)
    fn workspace_tab_bar_drag_press(
        &mut self,
        _mouse_x: usize,
        _mouse_y: usize,
        _button: MouseButton,
    ) -> bool {
        false
    }
    fn workspace_tab_bar_drag_move(&mut self, _mouse_x: usize, _mouse_y: usize) -> bool {
        false
    }
    fn workspace_tab_bar_drag_release(&mut self, _button: MouseButton) -> bool {
        false
    }

    // Pane drag-and-drop helpers (Alt+LeftDrag)
    fn workspace_pane_drag_press(
        &mut self,
        _mouse_x_px: f32,
        _mouse_y_px: f32,
        _button: MouseButton,
    ) -> bool {
        false
    }
    fn workspace_pane_drag_move(&mut self, _mouse_x_px: f32, _mouse_y_px: f32) -> bool {
        false
    }
    fn workspace_pane_drag_release(&mut self, _button: MouseButton) -> bool {
        false
    }

    /// Apply a new split ratio at a divider path
    fn workspace_set_split_ratio_at_path(
        &mut self,
        _path: Vec<crate::workspace::split_manager::SplitChild>,
        _axis: crate::workspace::split_manager::SplitAxis,
        _new_ratio: f32,
    ) {
    }
}

impl Action {
    fn toggle_selection<T, A>(ctx: &mut A, ty: SelectionType)
    where
        A: ActionContext<T>,
        T: EventListener,
    {
        ctx.toggle_selection(ty, ctx.terminal().vi_mode_cursor.point, Side::Left);

        // Make sure initial selection is not empty.
        if let Some(selection) = &mut ctx.terminal_mut().selection {
            selection.include_all();
        }
    }
}

trait Execute<T: EventListener> {
    fn execute<A: ActionContext<T>>(&self, ctx: &mut A);
}

impl<T: EventListener> Execute<T> for Action {
    #[inline]
    fn execute<A: ActionContext<T>>(&self, ctx: &mut A) {
        match self {
            Action::Esc(s) => ctx.paste(s, false),
            Action::Command(program) => ctx.spawn_daemon(program.program(), program.args()),
            Action::Hint(hint) => {
                ctx.display().hint_state.start(hint.clone());
                ctx.mark_dirty();
            }
            Action::ToggleViMode => {
                ctx.on_typing_start();
                ctx.toggle_vi_mode()
            }
            action @ (Action::ViMotion(_) | Action::Vi(_))
                if !ctx.terminal().mode().contains(TermMode::VI) =>
            {
                debug!("Ignoring {action:?}: Vi mode inactive");
            }
            Action::ViMotion(motion) => {
                ctx.on_typing_start();
                ctx.terminal_mut().vi_motion(*motion);
                ctx.mark_dirty();

                // Auto-unfold if vi cursor entered a folded region.
                if ctx.display().blocks.enabled {
                    let display_offset = ctx.terminal().grid().display_offset();
                    if let Some(view) = openagent_terminal_core::term::point_to_viewport(
                        display_offset,
                        ctx.terminal().vi_mode_cursor.point,
                    ) {
                        let total_line = display_offset + view.line;
                        let changed =
                            { ctx.display().blocks.ensure_unfold_at_total_line(total_line) };
                        if changed {
                            ctx.display().damage_tracker.frame().mark_fully_damaged();
                            ctx.mark_dirty();
                        }
                    }
                }
            }
            Action::Vi(ViAction::ToggleNormalSelection) => {
                Self::toggle_selection(ctx, SelectionType::Simple);
            }
            Action::Vi(ViAction::ToggleLineSelection) => {
                Self::toggle_selection(ctx, SelectionType::Lines);
            }
            Action::Vi(ViAction::ToggleBlockSelection) => {
                Self::toggle_selection(ctx, SelectionType::Block);
            }
            Action::Vi(ViAction::ToggleSemanticSelection) => {
                Self::toggle_selection(ctx, SelectionType::Semantic);
            }
            Action::Vi(ViAction::Open) => {
                let hint = ctx.display().vi_highlighted_hint.take();
                if let Some(hint) = &hint {
                    ctx.mouse_mut().block_hint_launcher = false;
                    ctx.trigger_hint(hint);
                }
                ctx.display().vi_highlighted_hint = hint;
            }
            Action::Vi(ViAction::SearchNext) => {
                ctx.on_typing_start();

                let terminal = ctx.terminal();
                let direction = ctx.search_direction();
                let vi_point = terminal.vi_mode_cursor.point;
                let origin = match direction {
                    Direction::Right => vi_point.add(terminal, Boundary::None, 1),
                    Direction::Left => vi_point.sub(terminal, Boundary::None, 1),
                };

                if let Some(regex_match) = ctx.search_next(origin, direction, Side::Left) {
                    ctx.terminal_mut().vi_goto_point(*regex_match.start());
                    ctx.mark_dirty();

                    // Auto-unfold if vi cursor entered a folded region.
                    if ctx.display().blocks.enabled {
                        let display_offset = ctx.terminal().grid().display_offset();
                        if let Some(view) = openagent_terminal_core::term::point_to_viewport(
                            display_offset,
                            ctx.terminal().vi_mode_cursor.point,
                        ) {
                            let total_line = display_offset + view.line;
                            let changed =
                                { ctx.display().blocks.ensure_unfold_at_total_line(total_line) };
                            if changed {
                                ctx.display().damage_tracker.frame().mark_fully_damaged();
                                ctx.mark_dirty();
                            }
                        }
                    }
                }
            }
            Action::Vi(ViAction::SearchPrevious) => {
                ctx.on_typing_start();

                let terminal = ctx.terminal();
                let direction = ctx.search_direction().opposite();
                let vi_point = terminal.vi_mode_cursor.point;
                let origin = match direction {
                    Direction::Right => vi_point.add(terminal, Boundary::None, 1),
                    Direction::Left => vi_point.sub(terminal, Boundary::None, 1),
                };

                if let Some(regex_match) = ctx.search_next(origin, direction, Side::Left) {
                    ctx.terminal_mut().vi_goto_point(*regex_match.start());
                    ctx.mark_dirty();

                    // Auto-unfold if vi cursor entered a folded region.
                    if ctx.display().blocks.enabled {
                        let display_offset = ctx.terminal().grid().display_offset();
                        if let Some(view) = openagent_terminal_core::term::point_to_viewport(
                            display_offset,
                            ctx.terminal().vi_mode_cursor.point,
                        ) {
                            let total_line = display_offset + view.line;
                            let changed =
                                { ctx.display().blocks.ensure_unfold_at_total_line(total_line) };
                            if changed {
                                ctx.display().damage_tracker.frame().mark_fully_damaged();
                                ctx.mark_dirty();
                            }
                        }
                    }
                }
            }
            Action::Vi(ViAction::SearchStart) => {
                let terminal = ctx.terminal();
                let origin = terminal
                    .vi_mode_cursor
                    .point
                    .sub(terminal, Boundary::None, 1);

                if let Some(regex_match) = ctx.search_next(origin, Direction::Left, Side::Left) {
                    ctx.terminal_mut().vi_goto_point(*regex_match.start());
                    ctx.mark_dirty();

                    // Auto-unfold if vi cursor entered a folded region.
                    if ctx.display().blocks.enabled {
                        let display_offset = ctx.terminal().grid().display_offset();
                        if let Some(view) = openagent_terminal_core::term::point_to_viewport(
                            display_offset,
                            ctx.terminal().vi_mode_cursor.point,
                        ) {
                            let total_line = display_offset + view.line;
                            let changed =
                                { ctx.display().blocks.ensure_unfold_at_total_line(total_line) };
                            if changed {
                                ctx.display().damage_tracker.frame().mark_fully_damaged();
                                ctx.mark_dirty();
                            }
                        }
                    }
                }
            }
            Action::Vi(ViAction::SearchEnd) => {
                let terminal = ctx.terminal();
                let origin = terminal
                    .vi_mode_cursor
                    .point
                    .add(terminal, Boundary::None, 1);

                if let Some(regex_match) = ctx.search_next(origin, Direction::Right, Side::Right) {
                    ctx.terminal_mut().vi_goto_point(*regex_match.end());
                    ctx.mark_dirty();

                    // Auto-unfold if vi cursor entered a folded region.
                    if ctx.display().blocks.enabled {
                        let display_offset = ctx.terminal().grid().display_offset();
                        if let Some(view) = openagent_terminal_core::term::point_to_viewport(
                            display_offset,
                            ctx.terminal().vi_mode_cursor.point,
                        ) {
                            let total_line = display_offset + view.line;
                            let changed =
                                { ctx.display().blocks.ensure_unfold_at_total_line(total_line) };
                            if changed {
                                ctx.display().damage_tracker.frame().mark_fully_damaged();
                                ctx.mark_dirty();
                            }
                        }
                    }
                }
            }
            Action::Vi(ViAction::CenterAroundViCursor) => {
                let term = ctx.terminal();
                let display_offset = term.grid().display_offset() as i32;
                let target = -display_offset + term.screen_lines() as i32 / 2 - 1;
                let line = term.vi_mode_cursor.point.line;
                let scroll_lines = target - line.0;

                ctx.scroll(Scroll::Delta(scroll_lines));
            }
            Action::Vi(ViAction::InlineSearchForward) => {
                ctx.start_inline_search(Direction::Right, false)
            }
            Action::Vi(ViAction::InlineSearchBackward) => {
                ctx.start_inline_search(Direction::Left, false)
            }
            Action::Vi(ViAction::InlineSearchForwardShort) => {
                ctx.start_inline_search(Direction::Right, true)
            }
            Action::Vi(ViAction::InlineSearchBackwardShort) => {
                ctx.start_inline_search(Direction::Left, true)
            }
            Action::Vi(ViAction::InlineSearchNext) => ctx.inline_search_next(),
            Action::Vi(ViAction::InlineSearchPrevious) => ctx.inline_search_previous(),
            Action::Vi(ViAction::SemanticSearchForward | ViAction::SemanticSearchBackward) => {
                let seed_text = match ctx.terminal().selection_to_string() {
                    Some(selection) if !selection.is_empty() => selection,
                    // Get semantic word at the vi cursor position.
                    _ => ctx.semantic_word(ctx.terminal().vi_mode_cursor.point),
                };

                if !seed_text.is_empty() {
                    let direction = match self {
                        Action::Vi(ViAction::SemanticSearchForward) => Direction::Right,
                        _ => Direction::Left,
                    };
                    ctx.start_seeded_search(direction, seed_text);
                }
            }
            action @ Action::Search(_) if !ctx.search_active() => {
                debug!("Ignoring {action:?}: Search mode inactive");
            }
            Action::Search(SearchAction::SearchFocusNext) => {
                ctx.advance_search_origin(ctx.search_direction());
            }
            Action::Search(SearchAction::SearchFocusPrevious) => {
                let direction = ctx.search_direction().opposite();
                ctx.advance_search_origin(direction);
            }
            Action::Search(SearchAction::SearchConfirm) => ctx.confirm_search(),
            Action::Search(SearchAction::SearchCancel) => ctx.cancel_search(),
            Action::Search(SearchAction::SearchClear) => {
                let direction = ctx.search_direction();
                ctx.cancel_search();
                ctx.start_search(direction);
            }
            Action::Search(SearchAction::SearchDeleteWord) => ctx.search_pop_word(),
            Action::Search(SearchAction::SearchHistoryPrevious) => ctx.search_history_previous(),
            Action::Search(SearchAction::SearchHistoryNext) => ctx.search_history_next(),
            Action::Mouse(MouseAction::ExpandSelection) => ctx.expand_selection(),
            Action::OpenCommandPalette => ctx.open_command_palette(),
            Action::RunWorkflow(name) => ctx.run_workflow_by_name(name),
            Action::OpenBlocksSearchPanel => {
                if ctx.blocks_search_active() {
                    ctx.blocks_search_cancel();
                } else {
                    ctx.open_blocks_search_panel();
                }
            }
            Action::OpenWorkflowsPanel => {
                if ctx.workflows_panel_active() {
                    ctx.workflows_panel_cancel();
                } else {
                    ctx.open_workflows_panel();
                }
            }
            Action::OpenFileTree => {
                if ctx.file_tree_active() {
                    ctx.close_file_tree_panel();
                } else {
                    ctx.open_file_tree_panel();
                }
            }
            Action::OpenSettingsPanel => {
                if ctx.settings_panel_active() {
                    ctx.close_settings_panel();
                } else {
                    ctx.open_settings_panel();
                }
            }
            Action::ToggleAiPanel => ctx.open_ai_panel(),
            Action::OpenDebugPanel => {
                // Toggle DAP overlay
                #[cfg(feature = "dap")]
                {
                    // If already open, close
                    if cfg!(feature = "dap") {
                        if ctx.display().dap_overlay.active {
                            ctx.display().dap_close();
                        } else {
                            ctx.display().dap_open(None);
                        }
                        ctx.mark_dirty();
                    }
                }
            }
            #[cfg(feature = "ai")]
            Action::AiExplain => {
                // Get selected text or last command output for explanation
                let text_to_explain = ctx
                    .terminal()
                    .selection_to_string()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| {
                        // Fallback: get the last few lines of terminal output
                        // For now, use a simple placeholder
                        "Please explain the last command output".to_string()
                    });

                // Open AI panel with explain prompt
                ctx.open_ai_panel();
                if let Some(runtime) = ctx.ai_runtime_mut() {
                    runtime.ui.scratch = format!("Explain this: {}", text_to_explain);
                    runtime.ui.cursor_position = runtime.ui.scratch.len();
                }
                ctx.mark_dirty();
            }
            #[cfg(feature = "ai")]
            Action::AiFix => {
                // Get selected text or error output for fixing suggestions
                let text_to_fix = ctx
                    .terminal()
                    .selection_to_string()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| {
                        // Fallback: analyze recent terminal output for errors
                        "Please suggest a fix for the recent error".to_string()
                    });

                // Open AI panel with fix prompt
                ctx.open_ai_panel();
                if let Some(runtime) = ctx.ai_runtime_mut() {
                    runtime.ui.scratch = format!("How do I fix this error: {}", text_to_fix);
                    runtime.ui.cursor_position = runtime.ui.scratch.len();
                }
                ctx.mark_dirty();
            }
            Action::SearchForward => ctx.start_search(Direction::Right),
            Action::SearchBackward => ctx.start_search(Direction::Left),
            Action::Copy => ctx.copy_selection(ClipboardType::Clipboard),
            #[cfg(not(any(target_os = "macos", windows)))]
            Action::CopySelection => ctx.copy_selection(ClipboardType::Selection),
            Action::ClearSelection => ctx.clear_selection(),
            Action::Paste => {
                let text = ctx.clipboard_mut().load(ClipboardType::Clipboard);
                if !text.is_empty() && ctx.config().security.gate_paste_events {
                    // Analyze paste content with Security Lens
                    let policy = ctx.config().security.clone();
                    let mut lens = SecurityLens::new(policy.clone());
                    if let Some(risk) = lens.analyze_paste_content(&text) {
                        // Block if policy requires blocking critical
                        if lens.should_block(&risk) {
                            let message = message_bar::Message::new(
                                format!(
                                    "Blocked risky paste ({}). {}",
                                    match risk.level {
                                        RiskLevel::Critical => "CRITICAL",
                                        RiskLevel::Warning => "WARNING",
                                        RiskLevel::Caution => "CAUTION",
                                        RiskLevel::Safe => "SAFE",
                                    },
                                    risk.explanation
                                ),
                                message_bar::MessageType::Error,
                            );
                            ctx.send_user_event(crate::event::EventType::Message(message));
                            return;
                        }

                        // Require confirmation if policy says so
                        let requires_confirmation = policy
                            .require_confirmation
                            .get(&risk.level)
                            .copied()
                            .unwrap_or(false);

                        if requires_confirmation {
                            // Prepare confirmation body
                            let mut body = String::new();
                            body.push_str(&format!("{}\n\n", risk.explanation));
                            if !risk.mitigations.is_empty() {
                                body.push_str("Suggested mitigations:\n");
                                for m in &risk.mitigations {
                                    body.push_str(&format!("  • {}\n", m));
                                }
                                body.push('\n');
                            }
                            // Show sanitized preview of pasted content for safety
                               let preview = sanitize_preview(&text, 10, 1200);
                            body.push_str(&format!("Pasted content (preview):\n{}", preview));

                            let title = match risk.level {
                                RiskLevel::Critical => "CRITICAL: Confirm paste".into(),
                                RiskLevel::Warning => "Warning: Confirm paste".into(),
                                RiskLevel::Caution => "Caution: Confirm paste".into(),
                                RiskLevel::Safe => "Confirm paste".into(),
                            };

                            match crate::ui_confirm::request_confirm(
                                title,
                                body,
                                Some("Paste".into()),
                                Some("Cancel".into()),
                                Some(30_000),
                            ) {
                                Ok(true) => {
                                    ctx.paste(&text, true);
                                }
                                Ok(false) => {
                                    // User canceled: do nothing
                                }
                                Err(e) => {
                                    // On error, surface a message and do not paste
                                    let message = message_bar::Message::new(
                                        format!("Paste confirmation failed: {}", e),
                                        message_bar::MessageType::Warning,
                                    );
                                    ctx.send_user_event(crate::event::EventType::Message(message));
                                }
                            }
                            return;
                        }
                    }
                }
                // Default path: if multi-line, show quick Run/Cancel; else paste directly
                let is_multiline = text.contains('\n');
                if is_multiline {
                    // Build sanitized preview of the multi-line content (first 10 lines)
                    let preview = sanitize_preview(&text, 10, 1200);
                    let title = "Multi-line paste".to_string();
                    let body = format!("About to paste multiple lines. Run after paste?\n\nPreview:\n{}", preview);
                    match crate::ui_confirm::request_confirm(
                        title,
                        body,
                        Some("Run".into()),
                        Some("Just Paste".into()),
                        Some(20_000),
                    ) {
                        Ok(true) => {
                            // Paste and run
                            ctx.paste(&text, true);
                            ctx.write_to_pty("\n".as_bytes());
                        }
                        Ok(false) => {
                            // Paste only
                            ctx.paste(&text, true);
                        }
                        Err(_e) => {
                            // On error or dismiss, default to insert-only
                            ctx.paste(&text, true);
                        }
                    }
                } else {
                    ctx.paste(&text, true);
                }
            }
            Action::PasteAndRun => {
                let text = ctx.clipboard_mut().load(ClipboardType::Clipboard);
                if text.is_empty() {
                    return;
                }
                // Always present a preview before executing even if safe
                let mut require_preview = true;
                if ctx.config().security.gate_paste_events {
                    let policy = ctx.config().security.clone();
                    let mut lens = SecurityLens::new(policy.clone());
                    if let Some(risk) = lens.analyze_paste_content(&text) {
                        if lens.should_block(&risk) {
                            let message = message_bar::Message::new(
                                format!(
                                    "Blocked risky paste ({}). {}",
                                    match risk.level {
                                        RiskLevel::Critical => "CRITICAL",
                                        RiskLevel::Warning => "WARNING",
                                        RiskLevel::Caution => "CAUTION",
                                        RiskLevel::Safe => "SAFE",
                                    },
                                    risk.explanation
                                ),
                                message_bar::MessageType::Error,
                            );
                            ctx.send_user_event(crate::event::EventType::Message(message));
                            return;
                        }
                        let requires_confirmation = policy
                            .require_confirmation
                            .get(&risk.level)
                            .copied()
                            .unwrap_or(false);
                        if requires_confirmation {
                            let mut body = String::new();
                            body.push_str(&format!("{}\n\n", risk.explanation));
                            if !risk.mitigations.is_empty() {
                                body.push_str("Suggested mitigations:\n");
                                for m in &risk.mitigations {
                                    body.push_str(&format!("  • {}\n", m));
                                }
                                body.push('\n');
                            }
                            let preview = sanitize_preview(&text, 10, 1200);
                            body.push_str(&format!("Pasted content (preview):\n{}", preview));
                            let title = match risk.level {
                                RiskLevel::Critical => "CRITICAL: Confirm paste & run".into(),
                                RiskLevel::Warning => "Warning: Confirm paste & run".into(),
                                RiskLevel::Caution => "Caution: Confirm paste & run".into(),
                                RiskLevel::Safe => "Confirm paste & run".into(),
                            };
                            match crate::ui_confirm::request_confirm(
                                title,
                                body,
                                Some("Paste & Run".into()),
                                Some("Cancel".into()),
                                Some(30_000),
                            ) {
                                Ok(true) => {
                                    ctx.paste(&text, true);
                                    ctx.write_to_pty("\n".as_bytes());
                                }
                                Ok(false) => {}
                                Err(e) => {
                                    let message = message_bar::Message::new(
                                        format!("Paste & run confirmation failed: {}", e),
                                        message_bar::MessageType::Warning,
                                    );
                                    ctx.send_user_event(crate::event::EventType::Message(message));
                                }
                            }
                            return;
                        }
                        // If a risk-level confirmation was already shown, we won't show the preview again
                        require_preview = !requires_confirmation;
                    }
                }
                // Show preview confirmation when required (safe or not prompted yet)
                if require_preview {
                    let preview = sanitize_preview(&text, 10, 1200);
                    let title = "Paste & Run".to_string();
                    let body = format!("About to paste and run:\n\n{}", preview);
                    match crate::ui_confirm::request_confirm(
                        title,
                        body,
                        Some("Paste & Run".into()),
                        Some("Cancel".into()),
                        Some(20_000),
                    ) {
                        Ok(true) => {
                            ctx.paste(&text, true);
                            ctx.write_to_pty("\n".as_bytes());
                        }
                        _ => { /* Cancelled: do nothing */ }
                    }
                } else {
                    // Already confirmed by security gating; proceed
                    ctx.paste(&text, true);
                    ctx.write_to_pty("\n".as_bytes());
                }
            }
            Action::PasteSelection => {
                let text = ctx.clipboard_mut().load(ClipboardType::Selection);
                if !text.is_empty() && ctx.config().security.gate_paste_events {
                    // Analyze paste content with Security Lens
                    let policy = ctx.config().security.clone();
                    let mut lens = SecurityLens::new(policy.clone());
                    if let Some(risk) = lens.analyze_paste_content(&text) {
                        if lens.should_block(&risk) {
                            let message = message_bar::Message::new(
                                format!(
                                    "Blocked risky paste ({}). {}",
                                    match risk.level {
                                        RiskLevel::Critical => "CRITICAL",
                                        RiskLevel::Warning => "WARNING",
                                        RiskLevel::Caution => "CAUTION",
                                        RiskLevel::Safe => "SAFE",
                                    },
                                    risk.explanation
                                ),
                                message_bar::MessageType::Error,
                            );
                            ctx.send_user_event(crate::event::EventType::Message(message));
                            return;
                        }
                        let requires_confirmation = policy
                            .require_confirmation
                            .get(&risk.level)
                            .copied()
                            .unwrap_or(false);
                        if requires_confirmation {
                            let mut body = String::new();
                            body.push_str(&format!("{}\n\n", risk.explanation));
                            if !risk.mitigations.is_empty() {
                                body.push_str("Suggested mitigations:\n");
                                for m in &risk.mitigations {
                                    body.push_str(&format!("  • {}\n", m));
                                }
                                body.push('\n');
                            }
                            let preview: String =
                                text.lines().take(10).collect::<Vec<_>>().join("\n");
                            body.push_str(&format!("Pasted content (preview):\n{}", preview));

                            let title = match risk.level {
                                RiskLevel::Critical => "CRITICAL: Confirm paste".into(),
                                RiskLevel::Warning => "Warning: Confirm paste".into(),
                                RiskLevel::Caution => "Caution: Confirm paste".into(),
                                RiskLevel::Safe => "Confirm paste".into(),
                            };

                            match crate::ui_confirm::request_confirm(
                                title,
                                body,
                                Some("Paste".into()),
                                Some("Cancel".into()),
                                Some(30_000),
                            ) {
                                Ok(true) => {
                                    ctx.paste(&text, true);
                                }
                                Ok(false) => {}
                                Err(e) => {
                                    let message = message_bar::Message::new(
                                        format!("Paste confirmation failed: {}", e),
                                        message_bar::MessageType::Warning,
                                    );
                                    ctx.send_user_event(crate::event::EventType::Message(message));
                                }
                            }
                            return;
                        }
                    }
                }
                ctx.paste(&text, true);
            }
            Action::ToggleFullscreen => ctx.window().toggle_fullscreen(),
            Action::ToggleMaximized => ctx.window().toggle_maximized(),
            #[cfg(target_os = "macos")]
            Action::ToggleSimpleFullscreen => ctx.window().toggle_simple_fullscreen(),
            #[cfg(target_os = "macos")]
            Action::Hide => ctx.event_loop().hide_application(),
            #[cfg(target_os = "macos")]
            Action::HideOtherApplications => ctx.event_loop().hide_other_applications(),
            #[cfg(not(target_os = "macos"))]
            Action::Hide => ctx.window().set_visible(false),
            Action::Minimize => ctx.window().set_minimized(true),
            Action::Quit => {
                ctx.window().hold = false;
                ctx.terminal_mut().exit();
            }
            Action::IncreaseFontSize => ctx.change_font_size(FONT_SIZE_STEP),
            Action::DecreaseFontSize => ctx.change_font_size(-FONT_SIZE_STEP),
            Action::ResetFontSize => ctx.reset_font_size(),
            Action::ScrollPageUp
            | Action::ScrollPageDown
            | Action::ScrollHalfPageUp
            | Action::ScrollHalfPageDown => {
                // Move vi mode cursor.
                let term = ctx.terminal_mut();
                let (scroll, amount) = match self {
                    Action::ScrollPageUp => (Scroll::PageUp, term.screen_lines() as i32),
                    Action::ScrollPageDown => (Scroll::PageDown, -(term.screen_lines() as i32)),
                    Action::ScrollHalfPageUp => {
                        let amount = term.screen_lines() as i32 / 2;
                        (Scroll::Delta(amount), amount)
                    }
                    Action::ScrollHalfPageDown => {
                        let amount = -(term.screen_lines() as i32 / 2);
                        (Scroll::Delta(amount), amount)
                    }
                    _ => unreachable!(),
                };

                let old_vi_cursor = term.vi_mode_cursor;
                term.vi_mode_cursor = term.vi_mode_cursor.scroll(term, amount);
                if old_vi_cursor != term.vi_mode_cursor {
                    ctx.mark_dirty();
                }

                ctx.scroll(scroll);
            }
            Action::ScrollLineUp => ctx.scroll(Scroll::Delta(1)),
            Action::ScrollLineDown => ctx.scroll(Scroll::Delta(-1)),
            Action::ScrollToTop => {
                ctx.scroll(Scroll::Top);

                // Move vi mode cursor.
                let topmost_line = ctx.terminal().topmost_line();
                ctx.terminal_mut().vi_mode_cursor.point.line = topmost_line;
                ctx.terminal_mut().vi_motion(ViMotion::FirstOccupied);
                ctx.mark_dirty();
            }
            Action::ScrollToBottom => {
                ctx.scroll(Scroll::Bottom);

                // Move vi mode cursor.
                let term = ctx.terminal_mut();
                term.vi_mode_cursor.point.line = term.bottommost_line();

                // Move to beginning twice, to always jump across linewraps.
                term.vi_motion(ViMotion::FirstOccupied);
                term.vi_motion(ViMotion::FirstOccupied);
                ctx.mark_dirty();
            }
            Action::ClearHistory => ctx.terminal_mut().clear_screen(ClearMode::Saved),
            Action::ClearLogNotice => (),
            Action::DumpAtlasStats => {
                ctx.display().dump_atlas_stats();
            }
            Action::TogglePerfHud => {
                if ctx.display().toggle_perf_hud() {
                    let msg = message_bar::Message::new(
                        "Perf HUD toggled".into(),
                        crate::message_bar::MessageType::Warning,
                    );
                    ctx.send_user_event(crate::event::EventType::Message(msg));
                }
            }
            Action::IncreaseSubpixelGamma => {
                if ctx.display().adjust_subpixel_gamma(0.1) {
                    let msg = message_bar::Message::new(
                        "Gamma +0.1".into(),
                        crate::message_bar::MessageType::Warning,
                    );
                    ctx.send_user_event(crate::event::EventType::Message(msg));
                }
            }
            Action::DecreaseSubpixelGamma => {
                if ctx.display().adjust_subpixel_gamma(-0.1) {
                    let msg = message_bar::Message::new(
                        "Gamma -0.1".into(),
                        crate::message_bar::MessageType::Warning,
                    );
                    ctx.send_user_event(crate::event::EventType::Message(msg));
                }
            }
            Action::ResetSubpixelGamma => {
                if ctx.display().reset_subpixel_gamma() {
                    let msg = message_bar::Message::new(
                        "Gamma reset".into(),
                        crate::message_bar::MessageType::Warning,
                    );
                    ctx.send_user_event(crate::event::EventType::Message(msg));
                }
            }
            Action::ToggleSubpixelText => {
                let applied = ctx.display().toggle_subpixel_text();
                if applied {
                    let msg = message_bar::Message::new(
                        "Toggled subpixel text".into(),
                        crate::message_bar::MessageType::Warning,
                    );
                    ctx.send_user_event(crate::event::EventType::Message(msg));
                    ctx.mark_dirty();
                }
            }
            Action::CycleSubpixelOrientation => {
                if let Some(next) = ctx.display().cycle_subpixel_orientation() {
                    let label = match next {
                        crate::config::debug::SubpixelOrientation::RGB => "RGB",
                        crate::config::debug::SubpixelOrientation::BGR => "BGR",
                    };
                    let msg = message_bar::Message::new(
                        format!("Subpixel orientation: {}", label),
                        crate::message_bar::MessageType::Warning,
                    );
                    ctx.send_user_event(crate::event::EventType::Message(msg));
                    ctx.mark_dirty();
                }
            }
            #[cfg(not(target_os = "macos"))]
            Action::CreateNewWindow => ctx.create_new_window(),
            Action::SpawnNewInstance => ctx.spawn_new_instance(),
            #[cfg(target_os = "macos")]
            Action::CreateNewWindow => ctx.create_new_window(None),
            #[cfg(target_os = "macos")]
            Action::CreateNewTab => {
                // Tabs on macOS are not possible without decorations.
                if ctx.config().window.decorations != Decorations::None {
                    let tabbing_id = Some(ctx.window().tabbing_id());
                    ctx.create_new_window(tabbing_id);
                }
            }
            #[cfg(target_os = "macos")]
            Action::SelectNextTab => ctx.window().select_next_tab(),
            #[cfg(target_os = "macos")]
            Action::SelectPreviousTab => ctx.window().select_previous_tab(),
            #[cfg(target_os = "macos")]
            Action::SelectTab1 => ctx.window().select_tab_at_index(0),
            #[cfg(target_os = "macos")]
            Action::SelectTab2 => ctx.window().select_tab_at_index(1),
            #[cfg(target_os = "macos")]
            Action::SelectTab3 => ctx.window().select_tab_at_index(2),
            #[cfg(target_os = "macos")]
            Action::SelectTab4 => ctx.window().select_tab_at_index(3),
            #[cfg(target_os = "macos")]
            Action::SelectTab5 => ctx.window().select_tab_at_index(4),
            #[cfg(target_os = "macos")]
            Action::SelectTab6 => ctx.window().select_tab_at_index(5),
            #[cfg(target_os = "macos")]
            Action::SelectTab7 => ctx.window().select_tab_at_index(6),
            #[cfg(target_os = "macos")]
            Action::SelectTab8 => ctx.window().select_tab_at_index(7),
            #[cfg(target_os = "macos")]
            Action::SelectTab9 => ctx.window().select_tab_at_index(8),
            #[cfg(target_os = "macos")]
            Action::SelectLastTab => ctx.window().select_last_tab(),
            Action::ToggleFoldBlock => {
                // Only toggle when blocks manager is active.
                if ctx.display().blocks.enabled {
                    // Use the bottom-most visible line as the target viewport point.
                    let screen_lines = ctx.size_info().screen_lines();
                    let viewport_point = Point::new(screen_lines.saturating_sub(1), Column(0));

                    // Borrow display to toggle and then mark damage after releasing the borrow.
                    let toggled = {
                        // Compute display offset without holding the display borrow.
                        let display_offset = { ctx.terminal().grid().display_offset() };
                        let display = ctx.display();
                        display
                            .blocks
                            .toggle_fold_at_viewport_point(display_offset, viewport_point)
                    };

                    if toggled {
                        // Fully damage the frame to ensure overlays and content update correctly.
                        ctx.display().damage_tracker.frame().mark_fully_damaged();
                        ctx.mark_dirty();
                    }
                }
            }
            Action::NextBlock => {
                if ctx.display().blocks.enabled {
                    let display_offset = ctx.terminal().grid().display_offset();
                    let target = { ctx.display().blocks.next_block_after(display_offset) };
                    if let Some(new_offset) = target {
                        let delta = new_offset as i32 - display_offset as i32;
                        ctx.scroll(Scroll::Delta(delta));
                    }
                }
            }
            Action::PreviousBlock => {
                if ctx.display().blocks.enabled {
                    let display_offset = ctx.terminal().grid().display_offset();
                    let target = { ctx.display().blocks.prev_block_before(display_offset) };
                    if let Some(new_offset) = target {
                        let delta = new_offset as i32 - display_offset as i32;
                        ctx.scroll(Scroll::Delta(delta));
                    }
                }
            }
            Action::CopyBlockOutput => {
                if ctx.display().blocks.enabled {
                    if let Some(text) = ctx.terminal().extract_current_block_output() {
                        ctx.copy_to_clipboard(text);
                    }
                }
            }
            Action::CopyBlockCommand => {
                if ctx.display().blocks.enabled {
                    if let Some(cmd) = ctx.terminal().current_block_command() {
                        ctx.copy_to_clipboard(cmd);
                    }
                }
            }
            Action::RerunBlockCommand => {
                if ctx.display().blocks.enabled {
                    if let Some((cmd, cwd)) = ctx.terminal().current_block_cmd_and_cwd() {
                        ctx.spawn_shell_command_in_cwd(cmd, cwd);
                    }
                }
            }
            Action::ExportBlockOutput => {
                if ctx.display().blocks.enabled {
                    if let Some(text) = ctx.terminal().extract_current_block_output() {
                        ctx.prompt_and_export_block_output(text);
                    }
                }
            }
            // Workspace / pane management actions
            Action::SplitHorizontal => ctx.workspace_split_horizontal(),
            Action::SplitVertical => ctx.workspace_split_vertical(),
            Action::FocusNextPane => ctx.workspace_focus_next_pane(),
            Action::FocusPreviousPane => ctx.workspace_focus_previous_pane(),
            Action::ClosePane => ctx.workspace_close_pane(),
            Action::ResizePaneLeft => ctx.workspace_resize_left(),
            Action::ResizePaneRight => ctx.workspace_resize_right(),
            Action::ResizePaneUp => ctx.workspace_resize_up(),
            Action::ResizePaneDown => ctx.workspace_resize_down(),
            // Tab management actions
            Action::CreateTab => ctx.workspace_create_tab(),
            Action::CloseTab => ctx.workspace_close_tab(),
            Action::NextTab => ctx.workspace_next_tab(),
            Action::PreviousTab => ctx.workspace_previous_tab(),
            // Pane zoom
            Action::ToggleZoom => ctx.workspace_toggle_zoom(),
            Action::TogglePaneSync => ctx.workspace_toggle_sync(),
            _ => (),
        }
    }
}

impl<T: EventListener, A: ActionContext<T>> Processor<T, A> {
    pub fn new(ctx: A) -> Self {
        Self {
            ctx,
            _phantom: Default::default(),
        }
    }

    /// Handle clicks on the tab bar (top or bottom)
    #[allow(dead_code)]
    fn process_tab_bar_click(&mut self) -> bool {
        if !self.ctx.config().workspace.tab_bar.show {
            return false;
        }
        // Compute mouse point in grid coordinates
        let size_info = self.ctx.size_info();
        let display_offset = self.ctx.terminal().grid().display_offset();
        let point = self.ctx.mouse().point(&size_info, display_offset);
        if point.line < 0 {
            return false;
        }
        let mouse_x = point.column.0;
        let mouse_y = point.line.0 as usize;

        // Ask the context to compute hit-testing, then perform the action
        if let Some(action) = self.ctx.workspace_tab_bar_hit(mouse_x, mouse_y) {
            use crate::display::warp_ui::TabBarAction;
            match action {
                TabBarAction::SelectTab(tab_id) => {
                    self.ctx.workspace_switch_to_tab(tab_id);
                }
                TabBarAction::CloseTab(tab_id) => {
                    // Ensure we close the correct tab: switch to it, then close
                    self.ctx.workspace_switch_to_tab(tab_id);
                    self.ctx.workspace_close_tab();
                }
                TabBarAction::CreateTab => {
                    self.ctx.workspace_create_tab();
                }
                // Drag-related actions are handled by dedicated handlers; treat as handled here
                TabBarAction::BeginDrag(_)
                | TabBarAction::DragMove(..)
                | TabBarAction::EndDrag(_)
                | TabBarAction::CancelDrag(_) => {
                    // no-op: considered handled
                }
                TabBarAction::OpenSettings => {
                    self.ctx.open_settings_panel();
                }
            }
            return true;
        }
        false
    }

    /// Handle clicks on the Warp-like bottom composer pill (visual-only -> opens AI panel)
    #[cfg_attr(test, allow(dead_code))]
    fn process_bottom_composer_click(&mut self) -> bool {
        use unicode_width::UnicodeWidthStr as _;
        let size_info = self.ctx.size_info();
        let ch = size_info.cell_height();
        let _cw = size_info.cell_width();
        let lines = size_info.screen_lines();
        if lines == 0 {
            return false;
        }
        // Compute composer pill rectangle in pixels (must mirror draw_warp_bottom_composer)
        let y_band = (lines.saturating_sub(1)) as f32 * ch;
        let margin_px = 6.0_f32;
        let x = margin_px;
        let y = y_band + 2.0_f32;
        let w = size_info.width() - margin_px * 2.0;
        let h = ch - 4.0_f32;
        let mx = self.ctx.display().last_mouse_x as f32;
        let my = self.ctx.display().last_mouse_y as f32;
        let inside = mx >= x && mx <= x + w && my >= y && my <= y + h;

        // Convert mouse x to column on the bottom line
        let point = {
            let display_offset = self.ctx.terminal().grid().display_offset();
            self.ctx.mouse().point(&size_info, display_offset)
        };
        let mouse_col = point.column.0;
        let line_idx = point.line.0 as usize;
        let bottom_line = lines.saturating_sub(1);

        // If provider dropdown is open, handle selection on the overlay row (line above)
        if self.ctx.display().ai_provider_dropdown_open {
            if line_idx + 1 == bottom_line {
                // Same provider list as draw routine
                let providers: &[(&str, &str)] = &[
                    ("openrouter", "OpenRouter"),
                    ("openai", "OpenAI"),
                    ("anthropic", "Anthropic"),
                    ("ollama", "Ollama"),
                ];
                // Start column accounts for star glyph width on overlay row
                let theme = self
                    .ctx
                    .config()
                    .resolved_theme
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| self.ctx.config().theme.resolve());
                let ui = theme.ui;
                let star = ui.composer_star_glyph.as_deref().unwrap_or("✦ ");
                let mut col = 2 + star.width();
                for (_pid, label) in providers.iter() {
                    let chip = format!("[{}]", label);
                    let wcols = chip.width();
                    let start = col;
                    let end = col + wcols;
                    if mouse_col >= start && mouse_col < end {
                        // Switch provider
                        #[cfg(feature = "ai")]
                        {
                            self.ctx
                                .send_user_event(crate::event::EventType::AiSwitchProvider(
                                    (*_pid).to_string(),
                                ));
                        }
                        self.ctx.display().ai_provider_dropdown_open = false;
                        self.ctx.mark_dirty();
                        return true;
                    }
                    col = end + ui.composer_chip_gap_cols as usize;
                }
                // Click elsewhere on overlay just closes it
                self.ctx.display().ai_provider_dropdown_open = false;
                self.ctx.mark_dirty();
                return true;
            } else {
                // Click outside overlay closes it
                self.ctx.display().ai_provider_dropdown_open = false;
                self.ctx.mark_dirty();
                // do not return; continue normal handling of click
            }
        }

        // Chip hit-testing: right-aligned inside the pill
        if inside && line_idx == bottom_line {
            let cols = size_info.columns();
            let mut col_end = cols.saturating_sub(2);
            // order: [Palette] [Run] drawn from right to left
            let chips = ["[Palette]", "[Run]"];
            for label in chips.iter() {
                let wcols = label.width();
                if wcols + 1 >= col_end {
                    break;
                }
                let start = col_end.saturating_sub(wcols);
                let end = col_end;
                if mouse_col >= start && mouse_col < end {
                    match *label {
                        "[Palette]" => {
                            self.ctx.open_command_palette();
                            return true;
                        }
                        "[Run]" => {
                            #[cfg(feature = "ai")]
                            {
                                // Seed AI panel with composer text if any, then propose
                                self.ctx.open_ai_panel();
                                let text = self.ctx.display().composer_text.clone();
                                if let Some(rt) = self.ctx.ai_runtime_mut() {
                                    if !text.is_empty() {
                                        rt.ui.scratch = text;
                                        rt.ui.cursor_position = rt.ui.scratch.len();
                                    }
                                }
                                self.ctx.ai_propose();
                                // reset composer focus
                                self.ctx.display().composer_text.clear();
                                self.ctx.display().composer_cursor = 0;
                                self.ctx.display().composer_sel_anchor = None;
                                self.ctx.display().composer_view_col_offset = 0;
                                self.ctx.display().composer_focused = false;
                                self.ctx.mark_dirty();
                                return true;
                            }
                        }
                        _ => {}
                    }
                }
                if start <= 2 {
                    break;
                }
                col_end = start.saturating_sub(2);
            }

            // Provider chip on the left: toggle dropdown when clicked
            let theme = self
                .ctx
                .config()
                .resolved_theme
                .as_ref()
                .cloned()
                .unwrap_or_else(|| self.ctx.config().theme.resolve());
            let star = theme.ui.composer_star_glyph.as_deref().unwrap_or("✦ ");
            let col = 2 + star.width();
            let provider_label = {
                let pid = self.ctx.display().ai_current_provider.as_str();
                match pid {
                    "openai" => "OpenAI",
                    "openrouter" => "OpenRouter",
                    "anthropic" => "Anthropic",
                    "ollama" => "Ollama",
                    _ => {
                        if pid.is_empty() {
                            "Provider"
                        } else {
                            pid
                        }
                    }
                }
            };
            let provider_chip = format!("[{} ▾]", provider_label);
            let wcols = provider_chip.width();
            let start = col;
            let end = col + wcols;
            if mouse_col >= start && mouse_col < end {
                // Toggle overlay dropdown
                let open = self.ctx.display().ai_provider_dropdown_open;
                self.ctx.display().ai_provider_dropdown_open = !open;
                self.ctx.mark_dirty();
                return true;
            }
        }

        // If clicked inside pill but not on a chip: focus or open AI (instant behavior)
        self.ctx.display().composer_focused = inside;
        if inside {
            #[cfg(feature = "ai")]
            {
                // In instant mode we open AI panel on click; commit mode will just focus
                let theme = self
                    .ctx
                    .config()
                    .resolved_theme
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| self.ctx.config().theme.resolve());
                if matches!(
                    theme.ui.composer_open_mode,
                    crate::config::theme::ComposerOpenMode::Instant
                ) {
                    self.ctx.open_ai_panel();
                    return true;
                }
            }
        }
        false
    }

    /// Handle clicks on the persistent Quick Actions bar at the bottom
    #[cfg_attr(test, allow(dead_code))]
    fn process_quick_actions_click(&mut self) -> bool {
        // Gather inputs without holding long immutable borrows
        let is_fs = self.ctx.window().is_fullscreen();
        let cfg = self.ctx.config();
        if !cfg.workspace.quick_actions.show {
            return false;
        }

        let size_info = self.ctx.size_info();
        let display_offset = self.ctx.terminal().grid().display_offset();
        let point = self.ctx.mouse().point(&size_info, display_offset);

        // Mirror line selection logic from draw_quick_actions_bar
        let lines = size_info.screen_lines();
        let tab_cfg = &cfg.workspace.tab_bar;
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
        let reserve_top = tab_cfg.show
            && !cfg.workspace.warp_overlay_only
            && matches!(
                effective_visibility,
                crate::config::workspace::TabBarVisibility::Always
            )
            && tab_cfg.position == crate::workspace::TabBarPosition::Top;
        let reserve_bottom = tab_cfg.show
            && !cfg.workspace.warp_overlay_only
            && matches!(
                effective_visibility,
                crate::config::workspace::TabBarVisibility::Always
            )
            && tab_cfg.position == crate::workspace::TabBarPosition::Bottom;

        let mut line = match cfg.workspace.quick_actions.position {
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
        if point.line != line {
            return false;
        }

        // Build label hitboxes; match drawing logic and AI enablement
        let mut labels: Vec<&str> = vec!["[Workflows]", "[Blocks]"];
        if cfg.workspace.quick_actions.show_palette {
            labels.push("[Palette]");
        }
        #[cfg(feature = "ai")]
        if cfg.ai.enabled {
            labels.push("[AI]");
        }

        let mut start = 1usize;
        let col = point.column.0;

        for label in labels {
            let end = start + label.chars().count();
            if col >= start && col < end {
                match label {
                    "[Workflows]" => {
                        #[cfg(feature = "workflow")]
                        {
                            self.ctx.open_workflows_panel();
                        }
                        self.ctx.display().quick_actions_press_flash_until =
                            Some(std::time::Instant::now() + std::time::Duration::from_millis(140));
                        return true;
                    }
                    "[Blocks]" => {
                        #[cfg(feature = "blocks")]
                        {
                            self.ctx.open_blocks_search_panel();
                        }
                        self.ctx.display().quick_actions_press_flash_until =
                            Some(std::time::Instant::now() + std::time::Duration::from_millis(140));
                        return true;
                    }
                    "[Palette]" => {
                        self.ctx.open_command_palette();
                        self.ctx.display().quick_actions_press_flash_until =
                            Some(std::time::Instant::now() + std::time::Duration::from_millis(140));
                        return true;
                    }
                    "[AI]" => {
                        #[cfg(feature = "ai")]
                        {
                            self.ctx.open_ai_panel();
                        }
                        self.ctx.display().quick_actions_press_flash_until =
                            Some(std::time::Instant::now() + std::time::Duration::from_millis(140));
                        return true;
                    }
                    _ => {}
                }
            }
            start = end + 2;
        }

        // Check right-aligned settings button: [⚙]
        let settings = "[⚙]";
        let cols = size_info.columns();
        if settings.chars().count() + 2 < cols {
            let start = cols.saturating_sub(settings.chars().count() + 2);
            let end = start + settings.chars().count();
            let col = point.column.0;
            if col >= start && col < end {
                self.ctx.open_settings_panel();
                self.ctx.display().quick_actions_press_flash_until =
                    Some(std::time::Instant::now() + std::time::Duration::from_millis(140));
                return true;
            }
        }

        false
    }

    #[inline]
    pub fn mouse_moved(&mut self, position: PhysicalPosition<f64>) {
        let size_info = self.ctx.size_info();

        let (x, y) = position.into();

        let lmb_pressed = self.ctx.mouse().left_button_state == ElementState::Pressed;
        let rmb_pressed = self.ctx.mouse().right_button_state == ElementState::Pressed;
        if !self.ctx.selection_is_empty() && (lmb_pressed || rmb_pressed) {
            self.update_selection_scrolling(y);
        }

        let display_offset = self.ctx.terminal().grid().display_offset();
        let old_point = self.ctx.mouse().point(&size_info, display_offset);

        let x = x.clamp(0, size_info.width() as i32 - 1) as usize;
        let y = y.clamp(0, size_info.height() as i32 - 1) as usize;
        self.ctx.mouse_mut().x = x;
        self.ctx.mouse_mut().y = y;
        // Track raw mouse position on the display for near-edge hover detection
        self.ctx.display().last_mouse_x = x;
        self.ctx.display().last_mouse_y = y;

        let inside_text_area = size_info.contains_point(x, y);
        let cell_side = self.cell_side(x);

        let point = self.ctx.mouse().point(&size_info, display_offset);
        let cell_changed = old_point != point;

        // If the mouse hasn't changed cells, do nothing.
        if !cell_changed
            && self.ctx.mouse().cell_side == cell_side
            && self.ctx.mouse().inside_text_area == inside_text_area
        {
            return;
        }

        self.ctx.mouse_mut().inside_text_area = inside_text_area;
        self.ctx.mouse_mut().cell_side = cell_side;

        // Update AI header hover state first; override cursor to pointer if hovering controls.
        let ai_hover = {
            #[cfg(feature = "ai")]
            {
                self.ctx.ai_update_hover_header()
            }
            #[cfg(not(feature = "ai"))]
            {
                false
            }
        };

        // Split divider drag handling: if dragging, update ratio and set resize cursor
        if self.ctx.display().split_drag.is_some() {
            if let Some(hit) = self.ctx.display().split_drag.clone() {
                let (mx, my) = (x as f32, y as f32);
                let rect = hit.rect;
                let mut ratio = match hit.axis {
                    crate::workspace::split_manager::SplitAxis::Horizontal => {
                        (mx - rect.x) / rect.width
                    }
                    crate::workspace::split_manager::SplitAxis::Vertical => {
                        (my - rect.y) / rect.height
                    }
                };
                ratio = ratio.clamp(0.1, 0.9);
                self.ctx
                    .workspace_set_split_ratio_at_path(hit.path.clone(), hit.axis, ratio);
                // Set appropriate cursor while dragging
                match hit.axis {
                    crate::workspace::split_manager::SplitAxis::Horizontal => {
                        self.ctx.window().set_mouse_cursor(CursorIcon::ColResize)
                    }
                    crate::workspace::split_manager::SplitAxis::Vertical => {
                        self.ctx.window().set_mouse_cursor(CursorIcon::RowResize)
                    }
                }
                return; // Do not process other hover logic while dragging
            }
        }

        // If a pane drag is active, update drag and short-circuit other hover logic
        if self
            .ctx
            .display()
            .pane_drag_manager
            .current_drag()
            .is_some()
        {
            let handled = self.ctx.workspace_pane_drag_move(x as f32, y as f32);
            if handled {
                return;
            }
        }

        // Tab bar hover: set pointer when hovering on a clickable tab area
        let tab_hover = {
            if self.ctx.config().workspace.tab_bar.show
                && self.ctx.config().workspace.tab_bar.position
                    != crate::workspace::TabBarPosition::Hidden
            {
                let mx = point.column.0;
                let my = point.line.0 as usize;
                self.ctx.workspace_tab_bar_hit(mx, my).is_some()
            } else {
                false
            }
        };

        // Quick Actions hover: set pointer when hovering labels or gear
        let quick_actions_hover = {
            let is_fs = self.ctx.window().is_fullscreen();
            let cfg = self.ctx.config();
            if cfg.workspace.quick_actions.show {
                let mut qa_hover = false;
                // Mirror line selection logic from draw_quick_actions_bar
                let size_info = self.ctx.size_info();
                let display_offset = self.ctx.terminal().grid().display_offset();
                let point = self.ctx.mouse().point(&size_info, display_offset);

                let lines = size_info.screen_lines();
                let tab_cfg = &cfg.workspace.tab_bar;
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
                let reserve_top = tab_cfg.show
                    && !cfg.workspace.warp_overlay_only
                    && matches!(
                        effective_visibility,
                        crate::config::workspace::TabBarVisibility::Always
                    )
                    && tab_cfg.position == crate::workspace::TabBarPosition::Top;
                let reserve_bottom = tab_cfg.show
                    && !cfg.workspace.warp_overlay_only
                    && matches!(
                        effective_visibility,
                        crate::config::workspace::TabBarVisibility::Always
                    )
                    && tab_cfg.position == crate::workspace::TabBarPosition::Bottom;
                let mut line = match cfg.workspace.quick_actions.position {
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
                if point.line.0 as usize == line {
                    // Labels and gear regions
                    use unicode_width::UnicodeWidthStr as _;
                    let cols = size_info.columns();
                    let mut col = 1usize;
                    let mut labels: Vec<&str> = vec!["[Workflows]", "[Blocks]"];
                    if cfg.workspace.quick_actions.show_palette {
                        labels.push("[Palette]");
                    }
                    #[cfg(feature = "ai")]
                    if cfg.ai.enabled {
                        labels.push("[AI]");
                    }
                    let pcol = point.column.0;
                    for label in labels {
                        let end = col + label.width();
                        if pcol >= col && pcol < end {
                            qa_hover = true;
                            break;
                        }
                        col = end + 2;
                    }
                    // Gear area: approx 3 cols from right with 2 padding
                    if !qa_hover {
                        let gear_cols = 3usize;
                        if gear_cols + 2 < cols {
                            let start = cols.saturating_sub(gear_cols + 2);
                            let end = start + gear_cols;
                            if pcol >= start && pcol < end {
                                qa_hover = true;
                            }
                        }
                    }
                    qa_hover
                } else {
                    false
                }
            } else {
                false
            }
        };

        // Update mouse state with split hover taking precedence for resize cursor
        if let Some(hit) = self.ctx.display().split_hover.clone() {
            match hit.axis {
                crate::workspace::split_manager::SplitAxis::Horizontal => {
                    self.ctx.window().set_mouse_cursor(CursorIcon::ColResize)
                }
                crate::workspace::split_manager::SplitAxis::Vertical => {
                    self.ctx.window().set_mouse_cursor(CursorIcon::RowResize)
                }
            }
        } else {
            // Update mouse state and check for URL change.
            let mouse_state = self.cursor_state();
            if ai_hover || tab_hover || quick_actions_hover {
                self.ctx.window().set_mouse_cursor(CursorIcon::Pointer);
            } else {
                self.ctx.window().set_mouse_cursor(mouse_state);
            }
        }

        // Update split hover state
        // Derive hover tolerance from configuration so users can tune indicator/handle sizes.
        let splits = &self.ctx.config().workspace.splits;
        let base_line = splits.indicator_line_width.max(1.0);
        let base_handle = if splits.show_resize_handles {
            splits.handle_size.max(1.0)
        } else {
            0.0
        };
        // Use half of the larger visual element as tolerance, with sane bounds.
        let mut tol = (base_line.max(base_handle)) * 0.5;
        // Slightly increase tolerance to ease hover acquisition, but clamp to reasonable range.
        tol = (tol + 2.0).clamp(3.0, 12.0);

        let split_hover_new = self.ctx.workspace_split_hit(x as f32, y as f32, tol);
        if self
            .ctx
            .display()
            .split_hover
            .as_ref()
            .map(|h| (h.axis, h.rect.x, h.rect.y))
            != split_hover_new
                .as_ref()
                .map(|h| (h.axis, h.rect.x, h.rect.y))
        {
            self.ctx.display().split_hover = split_hover_new;
            // Start hover animation timestamp for split indicators
            self.ctx.display().split_hover_anim_start = Some(std::time::Instant::now());
            self.ctx.mark_dirty();
        }

        // If a tab drag is active, process drag move to update potential reorder target
        if self.ctx.display().tab_drag_active.is_some() {
            let size_info = self.ctx.size_info();
            let display_offset = self.ctx.terminal().grid().display_offset();
            let p = self.ctx.mouse().point(&size_info, display_offset);
            let _ = self
                .ctx
                .workspace_tab_bar_drag_move(p.column.0, p.line.0 as usize);
        }

        // Update tab hover state for visuals and damage tab bar line when it changes
        let new_hover = if self.ctx.config().workspace.tab_bar.show
            && self.ctx.config().workspace.tab_bar.position
                != crate::workspace::TabBarPosition::Hidden
        {
            let mx = point.column.0;
            let my = point.line.0 as usize;
            if let Some(action) = self.ctx.workspace_tab_bar_hit(mx, my) {
                use crate::display::warp_ui::TabBarAction;
                match action {
                    TabBarAction::SelectTab(id) => Some(crate::display::TabHoverTarget::Tab(id)),
                    TabBarAction::CloseTab(id) => Some(crate::display::TabHoverTarget::Close(id)),
                    TabBarAction::CreateTab => Some(crate::display::TabHoverTarget::Create),
                    // Drag actions don't map to a static hover target
                    TabBarAction::BeginDrag(_)
                    | TabBarAction::DragMove(..)
                    | TabBarAction::EndDrag(_)
                    | TabBarAction::CancelDrag(_) => None,
                    TabBarAction::OpenSettings => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        if self.ctx.display().tab_hover != new_hover {
            self.ctx.display().tab_hover = new_hover;
            // Start hover animation timestamp for tabs
            self.ctx.display().tab_hover_anim_start = Some(std::time::Instant::now());
            // Damage the tab bar line
            let line = match self.ctx.config().workspace.tab_bar.position {
                crate::workspace::TabBarPosition::Top => 0,
                crate::workspace::TabBarPosition::Bottom => {
                    self.ctx.size_info().screen_lines().saturating_sub(1)
                }
                crate::workspace::TabBarPosition::Hidden => 0,
            };
            let cols = self.ctx.size_info().columns();
            self.ctx.display().damage_tracker.frame().damage_line(
                openagent_terminal_core::term::LineDamageBounds::new(line, 0, cols),
            );
            self.ctx.mark_dirty();
        }

        // Prompt hint highlight update.
        self.ctx.mouse_mut().hint_highlight_dirty = true;

        // Don't launch URLs if mouse has moved.
        self.ctx.mouse_mut().block_hint_launcher = true;

        if (lmb_pressed || rmb_pressed)
            && (self.ctx.modifiers().state().shift_key() || !self.ctx.mouse_mode())
        {
            self.ctx.update_selection(point, cell_side);
        } else if cell_changed
            && self
                .ctx
                .terminal()
                .mode()
                .intersects(TermMode::MOUSE_MOTION | TermMode::MOUSE_DRAG)
        {
            if lmb_pressed {
                self.mouse_report(32, ElementState::Pressed);
            } else if self.ctx.mouse().middle_button_state == ElementState::Pressed {
                self.mouse_report(33, ElementState::Pressed);
            } else if self.ctx.mouse().right_button_state == ElementState::Pressed {
                self.mouse_report(34, ElementState::Pressed);
            } else if self.ctx.terminal().mode().contains(TermMode::MOUSE_MOTION) {
                self.mouse_report(35, ElementState::Pressed);
            }
        }
    }

    /// Check which side of a cell an X coordinate lies on.
    fn cell_side(&self, x: usize) -> Side {
        let size_info = self.ctx.size_info();

        let cell_x =
            x.saturating_sub(size_info.padding_x() as usize) % size_info.cell_width() as usize;
        let half_cell_width = (size_info.cell_width() / 2.0) as usize;

        let additional_padding =
            (size_info.width() - size_info.padding_x() * 2.) % size_info.cell_width();
        let end_of_grid = size_info.width() - size_info.padding_x() - additional_padding;

        if cell_x > half_cell_width
            // Edge case when mouse leaves the window.
            || x as f32 >= end_of_grid
        {
            Side::Right
        } else {
            Side::Left
        }
    }

    fn mouse_report(&mut self, button: u8, state: ElementState) {
        let display_offset = self.ctx.terminal().grid().display_offset();
        let point = self
            .ctx
            .mouse()
            .point(&self.ctx.size_info(), display_offset);

        // Assure the mouse point is not in the scrollback.
        if point.line < 0 {
            return;
        }

        // Calculate modifiers value.
        let mut mods = 0;
        let modifiers = self.ctx.modifiers().state();
        if modifiers.shift_key() {
            mods += 4;
        }
        if modifiers.alt_key() {
            mods += 8;
        }
        if modifiers.control_key() {
            mods += 16;
        }

        // Report mouse events.
        if self.ctx.terminal().mode().contains(TermMode::SGR_MOUSE) {
            self.sgr_mouse_report(point, button + mods, state);
        } else if let ElementState::Released = state {
            self.normal_mouse_report(point, 3 + mods);
        } else {
            self.normal_mouse_report(point, button + mods);
        }
    }

    fn normal_mouse_report(&mut self, point: Point, button: u8) {
        let Point { line, column } = point;
        let utf8 = self.ctx.terminal().mode().contains(TermMode::UTF8_MOUSE);

        let max_point = if utf8 { 2015 } else { 223 };

        if line >= max_point || column >= max_point {
            return;
        }

        let mut msg = vec![b'\x1b', b'[', b'M', 32 + button];

        let mouse_pos_encode = |pos: usize| -> Vec<u8> {
            let pos = 32 + 1 + pos;
            let first = 0xC0 + pos / 64;
            let second = 0x80 + (pos & 63);
            vec![first as u8, second as u8]
        };

        if utf8 && column >= Column(95) {
            msg.append(&mut mouse_pos_encode(column.0));
        } else {
            msg.push(32 + 1 + column.0 as u8);
        }

        if utf8 && line >= 95 {
            msg.append(&mut mouse_pos_encode(line.0 as usize));
        } else {
            msg.push(32 + 1 + line.0 as u8);
        }

        self.ctx.write_to_pty(msg);
    }

    fn sgr_mouse_report(&mut self, point: Point, button: u8, state: ElementState) {
        let c = match state {
            ElementState::Pressed => 'M',
            ElementState::Released => 'm',
        };

        let msg = format!(
            "\x1b[<{};{};{}{}",
            button,
            point.column + 1,
            point.line + 1,
            c
        );
        self.ctx.write_to_pty(msg.into_bytes());
    }

    fn on_mouse_press(&mut self, button: MouseButton) {
        // Handle mouse mode.
        if !self.ctx.modifiers().state().shift_key() && self.ctx.mouse_mode() {
            self.ctx.mouse_mut().click_state = ClickState::None;

            let code = match button {
                MouseButton::Left => 0,
                MouseButton::Middle => 1,
                MouseButton::Right => 2,
                // Can't properly report more than three buttons..
                MouseButton::Back | MouseButton::Forward | MouseButton::Other(_) => return,
            };

            self.mouse_report(code, ElementState::Pressed);
        } else {
            // Calculate time since the last click to handle double/triple clicks.
            let now = Instant::now();
            let elapsed = now - self.ctx.mouse().last_click_timestamp;
            self.ctx.mouse_mut().last_click_timestamp = now;

            // Update multi-click state.
            self.ctx.mouse_mut().click_state = match self.ctx.mouse().click_state {
                // Reset click state if button has changed.
                _ if button != self.ctx.mouse().last_click_button => {
                    self.ctx.mouse_mut().last_click_button = button;
                    ClickState::Click
                }
                ClickState::Click if elapsed < CLICK_THRESHOLD => ClickState::DoubleClick,
                ClickState::DoubleClick if elapsed < CLICK_THRESHOLD => ClickState::TripleClick,
                _ => ClickState::Click,
            };

            // Load mouse point, treating message bar and padding as the closest cell.
            let display_offset = self.ctx.terminal().grid().display_offset();
            let point = self
                .ctx
                .mouse()
                .point(&self.ctx.size_info(), display_offset);

            // Handle AI panel header controls first (stop, regenerate, close).
            #[cfg(feature = "ai")]
            {
                if button == MouseButton::Left && self.ctx.ai_try_handle_header_click() {
                    return;
                }
            }

            // Handle clickable fold headers (skip in tests to avoid requiring full Display mock).
            #[cfg(not(test))]
            {
                if button == MouseButton::Left && self.ctx.display().blocks.enabled {
                    if let Some(view) =
                        openagent_terminal_core::term::point_to_viewport(display_offset, point)
                    {
                        let toggled = {
                            let display = self.ctx.display();
                            display
                                .blocks
                                .toggle_fold_header_at_viewport_line(display_offset, view.line)
                        };
                        if toggled {
                            // Fully damage and mark dirty; skip normal selection behavior.
                            self.ctx
                                .display()
                                .damage_tracker
                                .frame()
                                .mark_fully_damaged();
                            self.ctx.mark_dirty();
                            return;
                        }
                    }
                }
            }

            if let MouseButton::Left = button {
                self.on_left_click(point)
            }
        }
    }

    /// Handle left click selection and vi mode cursor movement.
    fn on_left_click(&mut self, point: Point) {
        let side = self.ctx.mouse().cell_side;
        let control = self.ctx.modifiers().state().control_key();

        match self.ctx.mouse().click_state {
            ClickState::Click => {
                // Don't launch URLs if this click cleared the selection.
                self.ctx.mouse_mut().block_hint_launcher = !self.ctx.selection_is_empty();

                self.ctx.clear_selection();

                // Start new empty selection.
                if control {
                    self.ctx.start_selection(SelectionType::Block, point, side);
                } else {
                    self.ctx.start_selection(SelectionType::Simple, point, side);
                }
            }
            ClickState::DoubleClick if !control => {
                self.ctx.mouse_mut().block_hint_launcher = true;
                self.ctx
                    .start_selection(SelectionType::Semantic, point, side);
            }
            ClickState::TripleClick if !control => {
                self.ctx.mouse_mut().block_hint_launcher = true;
                self.ctx.start_selection(SelectionType::Lines, point, side);
            }
            _ => (),
        };

        // Move vi mode cursor to mouse click position.
        if self.ctx.terminal().mode().contains(TermMode::VI) && !self.ctx.search_active() {
            self.ctx.terminal_mut().vi_mode_cursor.point = point;
            self.ctx.mark_dirty();
        }
    }

    fn on_mouse_release(&mut self, button: MouseButton) {
        if !self.ctx.modifiers().state().shift_key() && self.ctx.mouse_mode() {
            let code = match button {
                MouseButton::Left => 0,
                MouseButton::Middle => 1,
                MouseButton::Right => 2,
                // Can't properly report more than three buttons.
                MouseButton::Back | MouseButton::Forward | MouseButton::Other(_) => return,
            };
            self.mouse_report(code, ElementState::Released);
            return;
        }

        #[cfg(not(test))]
        {
            // Trigger hints highlighted by the mouse.
            let hint = self.ctx.display().highlighted_hint.take();
            if let Some(hint) = hint.as_ref().filter(|_| button == MouseButton::Left) {
                self.ctx.trigger_hint(hint);
            }
            self.ctx.display().highlighted_hint = hint;

            let timer_id = TimerId::new(Topic::SelectionScrolling, self.ctx.window().id());
            self.ctx.scheduler_mut().unschedule(timer_id);

            // Stop split drag on mouse release
            if let MouseButton::Left = button {
                // First, try to finalize pane drag if active
                if self.ctx.workspace_pane_drag_release(button) {
                    return;
                }
                if self.ctx.display().split_drag.take().is_some() {
                    return;
                }
            }
        }

        if let MouseButton::Left | MouseButton::Right = button {
            // Copy selection on release, to prevent flooding the display server.
            self.ctx.copy_selection(ClipboardType::Selection);
        }
    }

    pub fn mouse_wheel_input(&mut self, delta: MouseScrollDelta, phase: TouchPhase) {
        let multiplier = self.ctx.config().scrolling.multiplier;
        match delta {
            MouseScrollDelta::LineDelta(columns, lines) => {
                let new_scroll_px_x = columns * self.ctx.size_info().cell_width();
                let new_scroll_px_y = lines * self.ctx.size_info().cell_height();
                self.scroll_terminal(
                    new_scroll_px_x as f64,
                    new_scroll_px_y as f64,
                    multiplier as f64,
                );
            }
            MouseScrollDelta::PixelDelta(mut lpos) => {
                match phase {
                    TouchPhase::Started => {
                        // Reset offset to zero.
                        self.ctx.mouse_mut().accumulated_scroll = Default::default();
                    }
                    TouchPhase::Moved => {
                        // When the angle between (x, 0) and (x, y) is lower than ~25 degrees
                        // (cosine is larger that 0.9) we consider this scrolling as horizontal.
                        if lpos.x.abs() / lpos.x.hypot(lpos.y) > 0.9 {
                            lpos.y = 0.;
                        } else {
                            lpos.x = 0.;
                        }

                        self.scroll_terminal(lpos.x, lpos.y, multiplier as f64);
                    }
                    _ => (),
                }
            }
        }
    }

    fn scroll_terminal(&mut self, new_scroll_x_px: f64, new_scroll_y_px: f64, multiplier: f64) {
        const MOUSE_WHEEL_UP: u8 = 64;
        const MOUSE_WHEEL_DOWN: u8 = 65;
        const MOUSE_WHEEL_LEFT: u8 = 66;
        const MOUSE_WHEEL_RIGHT: u8 = 67;

        let width = f64::from(self.ctx.size_info().cell_width());
        let height = f64::from(self.ctx.size_info().cell_height());

        if self.ctx.mouse_mode() {
            self.ctx.mouse_mut().accumulated_scroll.x += new_scroll_x_px;
            self.ctx.mouse_mut().accumulated_scroll.y += new_scroll_y_px;

            let code = if new_scroll_y_px > 0. {
                MOUSE_WHEEL_UP
            } else {
                MOUSE_WHEEL_DOWN
            };
            let lines = (self.ctx.mouse().accumulated_scroll.y / height).abs() as i32;

            for _ in 0..lines {
                self.mouse_report(code, ElementState::Pressed);
            }

            let code = if new_scroll_x_px > 0. {
                MOUSE_WHEEL_LEFT
            } else {
                MOUSE_WHEEL_RIGHT
            };
            let columns = (self.ctx.mouse().accumulated_scroll.x / width).abs() as i32;

            for _ in 0..columns {
                self.mouse_report(code, ElementState::Pressed);
            }
        } else if self
            .ctx
            .terminal()
            .mode()
            .contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL)
            && !self.ctx.modifiers().state().shift_key()
        {
            self.ctx.mouse_mut().accumulated_scroll.x += new_scroll_x_px * multiplier;
            self.ctx.mouse_mut().accumulated_scroll.y += new_scroll_y_px * multiplier;

            // The chars here are the same as for the respective arrow keys.
            let line_cmd = if new_scroll_y_px > 0. { b'A' } else { b'B' };
            let column_cmd = if new_scroll_x_px > 0. { b'D' } else { b'C' };

            let lines = (self.ctx.mouse().accumulated_scroll.y / height).abs() as usize;
            let columns = (self.ctx.mouse().accumulated_scroll.x / width).abs() as usize;

            let mut content = Vec::with_capacity(3 * (lines + columns));

            for _ in 0..lines {
                content.push(0x1b);
                content.push(b'O');
                content.push(line_cmd);
            }

            for _ in 0..columns {
                content.push(0x1b);
                content.push(b'O');
                content.push(column_cmd);
            }

            self.ctx.write_to_pty(content);
        } else {
            self.ctx.mouse_mut().accumulated_scroll.y += new_scroll_y_px * multiplier;

            let lines = (self.ctx.mouse().accumulated_scroll.y / height) as i32;

            if lines != 0 {
                self.ctx.scroll(Scroll::Delta(lines));
            }
        }

        self.ctx.mouse_mut().accumulated_scroll.x %= width;
        self.ctx.mouse_mut().accumulated_scroll.y %= height;
    }

    pub fn on_focus_change(&mut self, is_focused: bool) {
        if self.ctx.terminal().mode().contains(TermMode::FOCUS_IN_OUT) {
            let chr = if is_focused { "I" } else { "O" };

            let msg = format!("\x1b[{chr}");
            self.ctx.write_to_pty(msg.into_bytes());
        }
    }

    /// Handle touch input.
    pub fn touch(&mut self, touch: TouchEvent) {
        match touch.phase {
            TouchPhase::Started => self.on_touch_start(touch),
            TouchPhase::Moved => self.on_touch_motion(touch),
            TouchPhase::Ended | TouchPhase::Cancelled => self.on_touch_end(touch),
        }
    }

    /// Handle beginning of touch input.
    pub fn on_touch_start(&mut self, touch: TouchEvent) {
        let touch_purpose = self.ctx.touch_purpose();
        *touch_purpose = match mem::take(touch_purpose) {
            TouchPurpose::None => TouchPurpose::Tap(touch),
            TouchPurpose::Tap(start) => TouchPurpose::Zoom(TouchZoom::new((start, touch))),
            TouchPurpose::ZoomPendingSlot(slot) => {
                TouchPurpose::Zoom(TouchZoom::new((slot, touch)))
            }
            TouchPurpose::Zoom(zoom) => {
                let slots = zoom.slots();
                let mut set = HashSet::default();
                set.insert(slots.0.id);
                set.insert(slots.1.id);
                TouchPurpose::Invalid(set)
            }
            TouchPurpose::Scroll(event) | TouchPurpose::Select(event) => {
                let mut set = HashSet::default();
                set.insert(event.id);
                TouchPurpose::Invalid(set)
            }
            TouchPurpose::Invalid(mut slots) => {
                slots.insert(touch.id);
                TouchPurpose::Invalid(slots)
            }
        };
    }

    /// Handle touch input movement.
    pub fn on_touch_motion(&mut self, touch: TouchEvent) {
        let touch_purpose = self.ctx.touch_purpose();
        match touch_purpose {
            TouchPurpose::None => (),
            // Handle transition from tap to scroll/select.
            TouchPurpose::Tap(start) => {
                let delta_x = touch.location.x - start.location.x;
                let delta_y = touch.location.y - start.location.y;
                if delta_x.abs() > MAX_TAP_DISTANCE {
                    // Update gesture state.
                    let start_location = start.location;
                    *touch_purpose = TouchPurpose::Select(*start);

                    // Start simulated mouse input.
                    self.mouse_moved(start_location);
                    self.mouse_input(ElementState::Pressed, MouseButton::Left);

                    // Apply motion since touch start.
                    self.on_touch_motion(touch);
                } else if delta_y.abs() > MAX_TAP_DISTANCE {
                    // Update gesture state.
                    *touch_purpose = TouchPurpose::Scroll(*start);

                    // Apply motion since touch start.
                    self.on_touch_motion(touch);
                }
            }
            TouchPurpose::Zoom(zoom) => {
                let font_delta = zoom.font_delta(touch);
                self.ctx.change_font_size(font_delta);
            }
            TouchPurpose::Scroll(last_touch) => {
                // Calculate delta and update last touch position.
                let delta_y = touch.location.y - last_touch.location.y;
                *touch_purpose = TouchPurpose::Scroll(touch);

                // Use a fixed scroll factor for touchscreens, to accurately track finger motion.
                self.scroll_terminal(0., delta_y, 1.0);
            }
            TouchPurpose::Select(_) => self.mouse_moved(touch.location),
            TouchPurpose::ZoomPendingSlot(_) | TouchPurpose::Invalid(_) => (),
        }
    }

    /// Handle end of touch input.
    pub fn on_touch_end(&mut self, touch: TouchEvent) {
        // Finalize the touch motion up to the release point.
        self.on_touch_motion(touch);

        let touch_purpose = self.ctx.touch_purpose();
        match touch_purpose {
            // Simulate LMB clicks.
            TouchPurpose::Tap(start) => {
                let start_location = start.location;
                *touch_purpose = Default::default();

                self.mouse_moved(start_location);
                self.mouse_input(ElementState::Pressed, MouseButton::Left);
                self.mouse_input(ElementState::Released, MouseButton::Left);
            }
            // Transition zoom to pending state once a finger was released.
            TouchPurpose::Zoom(zoom) => {
                let slots = zoom.slots();
                let remaining = if slots.0.id == touch.id {
                    slots.1
                } else {
                    slots.0
                };
                *touch_purpose = TouchPurpose::ZoomPendingSlot(remaining);
            }
            TouchPurpose::ZoomPendingSlot(_) => *touch_purpose = Default::default(),
            // Reset touch state once all slots were released.
            TouchPurpose::Invalid(slots) => {
                slots.remove(&touch.id);
                if slots.is_empty() {
                    *touch_purpose = Default::default();
                }
            }
            // Release simulated LMB.
            TouchPurpose::Select(_) => {
                *touch_purpose = Default::default();
                self.mouse_input(ElementState::Released, MouseButton::Left);
            }
            // Reset touch state on scroll finish.
            TouchPurpose::Scroll(_) => *touch_purpose = Default::default(),
            TouchPurpose::None => (),
        }
    }

    /// Reset mouse cursor based on modifier and terminal state.
    #[inline]
    pub fn reset_mouse_cursor(&mut self) {
        let mouse_state = self.cursor_state();
        self.ctx.window().set_mouse_cursor(mouse_state);
    }

    /// Modifier state change.
    pub fn modifiers_input(&mut self, modifiers: Modifiers) {
        *self.ctx.modifiers() = modifiers;

        // Prompt hint highlight update.
        self.ctx.mouse_mut().hint_highlight_dirty = true;

        // Update mouse state and check for URL change.
        let mouse_state = self.cursor_state();
        self.ctx.window().set_mouse_cursor(mouse_state);
    }

    pub fn mouse_input(&mut self, state: ElementState, button: MouseButton) {
        match button {
            MouseButton::Left => self.ctx.mouse_mut().left_button_state = state,
            MouseButton::Middle => self.ctx.mouse_mut().middle_button_state = state,
            MouseButton::Right => self.ctx.mouse_mut().right_button_state = state,
            _ => (),
        }

        // Skip normal mouse events if the message bar has been clicked.
        if self.message_bar_cursor_state() == Some(CursorIcon::Pointer)
            && state == ElementState::Pressed
        {
            let size = self.ctx.size_info();

            let current_lines = self.ctx.message().map_or(0, |m| m.text(&size).len());

            self.ctx.clear_selection();
            self.ctx.pop_message();

            // Reset cursor when message bar height changed or all messages are gone.
            let new_lines = self.ctx.message().map_or(0, |m| m.text(&size).len());

            let new_icon = match current_lines.cmp(&new_lines) {
                Ordering::Less => CursorIcon::Default,
                Ordering::Equal => CursorIcon::Pointer,
                Ordering::Greater => {
                    if self.ctx.mouse_mode() {
                        CursorIcon::Default
                    } else {
                        CursorIcon::Text
                    }
                }
            };

            self.ctx.window().set_mouse_cursor(new_icon);
        } else {
            match state {
                ElementState::Pressed => {
                    #[cfg(not(test))]
                    {
                        // Start split drag if hovering a divider
                        if let Some(hit) = self.ctx.display().split_hover.clone() {
                            self.ctx.display().split_drag = Some(hit);
                            return;
                        }
                        // Pane drag gesture starts a pane drag operation (configurable)
                        {
                            let dcfg = {
                                // Copy drag config to avoid holding immutable borrow across mutable
                                // ctx use
                                self.ctx.config().workspace.drag.clone()
                            };
                            if dcfg.enable_pane_drag {
                                let mods = self.ctx.modifiers().state();
                                let modifier_ok = match dcfg.pane_drag_modifier {
                                    crate::config::workspace::DragModifier::None => {
                                        !mods.alt_key() && !mods.control_key() && !mods.shift_key()
                                    }
                                    crate::config::workspace::DragModifier::Alt => mods.alt_key(),
                                    crate::config::workspace::DragModifier::Ctrl => {
                                        mods.control_key()
                                    }
                                    crate::config::workspace::DragModifier::Shift => {
                                        mods.shift_key()
                                    }
                                };
                                let button_ok = match dcfg.pane_drag_button {
                                    crate::config::workspace::DragButton::Left => {
                                        button == MouseButton::Left
                                    }
                                    crate::config::workspace::DragButton::Middle => {
                                        button == MouseButton::Middle
                                    }
                                    crate::config::workspace::DragButton::Right => {
                                        button == MouseButton::Right
                                    }
                                };
                                if modifier_ok && button_ok {
                                    let mx = self.ctx.display().last_mouse_x as f32;
                                    let my = self.ctx.display().last_mouse_y as f32;
                                    if self.ctx.workspace_pane_drag_press(mx, my, button) {
                                        return;
                                    }
                                }
                            }
                        }
                        // Tab bar drag/click handling (top/bottom)
                        // Compute current grid coordinates
                        let size_info = self.ctx.size_info();
                        let display_offset = self.ctx.terminal().grid().display_offset();
                        let point = self.ctx.mouse().point(&size_info, display_offset);
                        let mouse_x = point.column.0;
                        let mouse_y = point.line.0 as usize;
                        if self
                            .ctx
                            .workspace_tab_bar_drag_press(mouse_x, mouse_y, button)
                        {
                            return;
                        }
                        // Bottom composer click handling (opens AI panel)
                        if self.process_bottom_composer_click() {
                            return;
                        }
                        // Quick Actions bar click handling (bottom line)
                        if self.process_quick_actions_click() {
                            return;
                        }
                    }

                    // Process mouse press before bindings to update the `click_state`.
                    self.on_mouse_press(button);
                    self.process_mouse_bindings(button);
                }
                ElementState::Released => {
                    // Finish any active tab drag operation if present
                    if self.ctx.workspace_tab_bar_drag_release(button) {
                        return;
                    }
                    self.on_mouse_release(button)
                }
            }
        }
    }

    /// Attempt to find a binding and execute its action.
    ///
    /// The provided mode, mods, and key must match what is allowed by a binding
    /// for its action to be executed.
    fn process_mouse_bindings(&mut self, button: MouseButton) {
        let mode = BindingMode::new(self.ctx.terminal().mode(), self.ctx.search_active());
        let mouse_mode = self.ctx.mouse_mode();
        let mods = self.ctx.modifiers().state();
        let mouse_bindings = self.ctx.config().mouse_bindings().to_owned();

        // If mouse mode is active, also look for bindings without shift.
        let fallback_allowed = mouse_mode && mods.contains(ModifiersState::SHIFT);
        let mut exact_match_found = false;

        for binding in &mouse_bindings {
            // Don't trigger normal bindings in mouse mode unless Shift is pressed.
            if binding.is_triggered_by(mode, mods, &button) && (fallback_allowed || !mouse_mode) {
                binding.action.execute(&mut self.ctx);
                exact_match_found = true;
            }
        }

        if fallback_allowed && !exact_match_found {
            let fallback_mods = mods & !ModifiersState::SHIFT;
            for binding in &mouse_bindings {
                if binding.is_triggered_by(mode, fallback_mods, &button) {
                    binding.action.execute(&mut self.ctx);
                }
            }
        }
    }

    /// Check mouse icon state in relation to the message bar.
    fn message_bar_cursor_state(&self) -> Option<CursorIcon> {
        // Since search is above the message bar, the button is offset by search's height.
        let search_height = usize::from(self.ctx.search_active());

        // Calculate Y position of the end of the last terminal line.
        let size = self.ctx.size_info();
        let terminal_end = size.padding_y() as usize
            + size.cell_height() as usize * (size.screen_lines() + search_height);

        let mouse = self.ctx.mouse();
        let display_offset = self.ctx.terminal().grid().display_offset();
        let point = self
            .ctx
            .mouse()
            .point(&self.ctx.size_info(), display_offset);

        if self.ctx.message().is_none() || (mouse.y <= terminal_end) {
            None
        } else if mouse.y <= terminal_end + size.cell_height() as usize
            && point.column + message_bar::CLOSE_BUTTON_TEXT.len() >= size.columns()
        {
            Some(CursorIcon::Pointer)
        } else {
            Some(CursorIcon::Default)
        }
    }

    /// Icon state of the cursor.
    fn cursor_state(&mut self) -> CursorIcon {
        let display_offset = self.ctx.terminal().grid().display_offset();
        let point = self
            .ctx
            .mouse()
            .point(&self.ctx.size_info(), display_offset);
        let hyperlink = self.ctx.terminal().grid()[point].hyperlink();

        // Function to check if mouse is on top of a hint.
        let hint_highlighted = |hint: &HintMatch| hint.should_highlight(point, hyperlink.as_ref());

        if let Some(mouse_state) = self.message_bar_cursor_state() {
            mouse_state
        } else if cfg!(not(test))
            && self
                .ctx
                .display()
                .highlighted_hint
                .as_ref()
                .is_some_and(hint_highlighted)
        {
            CursorIcon::Pointer
        } else if !self.ctx.modifiers().state().shift_key() && self.ctx.mouse_mode() {
            CursorIcon::Default
        } else {
            CursorIcon::Text
        }
    }

    /// Handle automatic scrolling when selecting above/below the window.
    fn update_selection_scrolling(&mut self, mouse_y: i32) {
        let scale_factor = self.ctx.window().scale_factor;
        let size = self.ctx.size_info();
        let window_id = self.ctx.window().id();
        let scheduler = self.ctx.scheduler_mut();

        // Scale constants by DPI.
        let min_height = (MIN_SELECTION_SCROLLING_HEIGHT * scale_factor) as i32;
        let step = (SELECTION_SCROLLING_STEP * scale_factor) as i32;

        // Compute the height of the scrolling areas.
        let end_top = max(min_height, size.padding_y() as i32);
        let text_area_bottom = size.padding_y() + size.screen_lines() as f32 * size.cell_height();
        let start_bottom = min(size.height() as i32 - min_height, text_area_bottom as i32);

        // Get distance from closest window boundary.
        let delta = if mouse_y < end_top {
            end_top - mouse_y + step
        } else if mouse_y >= start_bottom {
            start_bottom - mouse_y - step
        } else {
            scheduler.unschedule(TimerId::new(Topic::SelectionScrolling, window_id));
            return;
        };

        // Scale number of lines scrolled based on distance to boundary.
        let event = Event::new(
            EventType::Scroll(Scroll::Delta(delta / step)),
            Some(window_id),
        );

        // Schedule event.
        let timer_id = TimerId::new(Topic::SelectionScrolling, window_id);
        scheduler.unschedule(timer_id);
        scheduler.schedule(event, SELECTION_SCROLLING_INTERVAL, true, timer_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_preview_strips_ansi_and_truncates() {
        let input = "\x1b[31mRED\x1b[0m\nline2\nline3\nline4";
        let out = sanitize_preview(input, 2, 6);
        // Two lines, limited chars; ellipsis present
        assert!(out.starts_with("RED\nli"));
        assert!(out.ends_with("…"));
    }

    #[test]
    fn sanitize_preview_redacts_bearer_and_keys() {
        let input = "Authorization: Bearer abcdefghijklmnop\napi_key = 12345\npassword: p@ss";
        let out = sanitize_preview(input, 5, 2000);
        assert!(out.contains("Authorization: Bearer {{REDACTED}}"));
        assert!(out.contains("api_key = {{REDACTED}}"));
        assert!(out.contains("password: {{REDACTED}}"));
    }

    use winit::event::{DeviceId, Event as WinitEvent, WindowEvent};
    use winit::keyboard::Key;
    use winit::window::WindowId;

    use openagent_terminal_core::event::Event as TerminalEvent;

    use crate::config::Binding;
    use crate::message_bar::MessageBuffer;

    const KEY: Key<&'static str> = Key::Character("0");

    struct MockEventProxy;
    impl EventListener for MockEventProxy {}

    struct ActionContext<'a, T> {
        pub terminal: &'a mut Term<T>,
        pub size_info: &'a SizeInfo,
        pub mouse: &'a mut Mouse,
        pub clipboard: &'a mut Clipboard,
        pub message_buffer: &'a mut MessageBuffer,
        pub modifiers: Modifiers,
        config: &'a UiConfig,
        inline_search_state: &'a mut InlineSearchState,
    }

    impl<T: EventListener> super::ActionContext<T> for ActionContext<'_, T> {
        fn search_next(
            &mut self,
            _origin: Point,
            _direction: Direction,
            _side: Side,
        ) -> Option<Match> {
            None
        }

        fn search_direction(&self) -> Direction {
            Direction::Right
        }

        fn inline_search_state(&mut self) -> &mut InlineSearchState {
            self.inline_search_state
        }

        fn search_active(&self) -> bool {
            false
        }

        fn terminal(&self) -> &Term<T> {
            self.terminal
        }

        fn terminal_mut(&mut self) -> &mut Term<T> {
            self.terminal
        }

        fn size_info(&self) -> SizeInfo {
            *self.size_info
        }

        fn selection_is_empty(&self) -> bool {
            true
        }

        fn scroll(&mut self, scroll: Scroll) {
            self.terminal.scroll_display(scroll);
        }

        fn mouse_mode(&self) -> bool {
            false
        }

        #[inline]
        fn mouse_mut(&mut self) -> &mut Mouse {
            self.mouse
        }

        #[inline]
        fn mouse(&self) -> &Mouse {
            self.mouse
        }

        #[inline]
        fn touch_purpose(&mut self) -> &mut TouchPurpose {
            unimplemented!();
        }

        fn modifiers(&mut self) -> &mut Modifiers {
            &mut self.modifiers
        }

        fn window(&mut self) -> &mut Window {
            unimplemented!();
        }

        fn display(&mut self) -> &mut Display {
            unimplemented!();
        }

        fn pop_message(&mut self) {
            self.message_buffer.pop();
        }

        fn message(&self) -> Option<&Message> {
            self.message_buffer.message()
        }

        fn config(&self) -> &UiConfig {
            self.config
        }

        fn clipboard_mut(&mut self) -> &mut Clipboard {
            self.clipboard
        }

        #[cfg(target_os = "macos")]
        fn event_loop(&self) -> &ActiveEventLoop {
            unimplemented!();
        }

        fn scheduler_mut(&mut self) -> &mut Scheduler {
            unimplemented!();
        }

        fn semantic_word(&self, _point: Point) -> String {
            unimplemented!();
        }
    }

    macro_rules! test_clickstate {
        {
            name: $name:ident,
            initial_state: $initial_state:expr,
            initial_button: $initial_button:expr,
            input: $input:expr,
            end_state: $end_state:expr,
            input_delay: $input_delay:expr,
        } => {
            #[test]
            fn $name() {
                let mut clipboard = Clipboard::new_nop();
                let cfg = UiConfig::default();
                let size = SizeInfo::new(
                    21.0,
                    51.0,
                    3.0,
                    3.0,
                    0.,
                    0.,
                    false,
                );

                let mut terminal = Term::new(cfg.term_options(), &size, MockEventProxy);

                let mut mouse = Mouse {
                    click_state: $initial_state,
                    last_click_button: $initial_button,
                    last_click_timestamp: Instant::now() - $input_delay,
                    ..Mouse::default()
                };

                let mut inline_search_state = InlineSearchState::default();
                let mut message_buffer = MessageBuffer::default();

                let context = ActionContext {
                    terminal: &mut terminal,
                    mouse: &mut mouse,
                    size_info: &size,
                    clipboard: &mut clipboard,
                    modifiers: Default::default(),
                    message_buffer: &mut message_buffer,
                    inline_search_state: &mut inline_search_state,
                    config: &cfg,
                };

                let mut processor = Processor::new(context);

                let event: WinitEvent::<TerminalEvent> = $input;
                if let WinitEvent::WindowEvent {
                    event: WindowEvent::MouseInput {
                        state,
                        button,
                        ..
                    },
                    ..
                } = event
                {
                    processor.mouse_input(state, button);
                };

                assert_eq!(processor.ctx.mouse.click_state, $end_state);
            }
        }
    }

    macro_rules! test_process_binding {
        {
            name: $name:ident,
            binding: $binding:expr,
            triggers: $triggers:expr,
            mode: $mode:expr,
            mods: $mods:expr,
        } => {
            #[test]
            fn $name() {
                if $triggers {
                    assert!($binding.is_triggered_by($mode, $mods, &KEY));
                } else {
                    assert!(!$binding.is_triggered_by($mode, $mods, &KEY));
                }
            }
        }
    }

    test_clickstate! {
        name: single_click,
        initial_state: ClickState::None,
        initial_button: MouseButton::Other(0),
        input: WinitEvent::WindowEvent {
            event: WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                device_id: DeviceId::dummy(),
            },
            window_id: WindowId::dummy(),
        },
        end_state: ClickState::Click,
        input_delay: Duration::ZERO,
    }

    test_clickstate! {
        name: single_right_click,
        initial_state: ClickState::None,
        initial_button: MouseButton::Other(0),
        input: WinitEvent::WindowEvent {
            event: WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                device_id: DeviceId::dummy(),
            },
            window_id: WindowId::dummy(),
        },
        end_state: ClickState::Click,
        input_delay: Duration::ZERO,
    }

    test_clickstate! {
        name: single_middle_click,
        initial_state: ClickState::None,
        initial_button: MouseButton::Other(0),
        input: WinitEvent::WindowEvent {
            event: WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                device_id: DeviceId::dummy(),
            },
            window_id: WindowId::dummy(),
        },
        end_state: ClickState::Click,
        input_delay: Duration::ZERO,
    }

    test_clickstate! {
        name: double_click,
        initial_state: ClickState::Click,
        initial_button: MouseButton::Left,
        input: WinitEvent::WindowEvent {
            event: WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                device_id: DeviceId::dummy(),
            },
            window_id: WindowId::dummy(),
        },
        end_state: ClickState::DoubleClick,
        input_delay: Duration::ZERO,
    }

    test_clickstate! {
        name: double_click_failed,
        initial_state: ClickState::Click,
        initial_button: MouseButton::Left,
        input: WinitEvent::WindowEvent {
            event: WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                device_id: DeviceId::dummy(),
            },
            window_id: WindowId::dummy(),
        },
        end_state: ClickState::Click,
        input_delay: CLICK_THRESHOLD,
    }

    test_clickstate! {
        name: triple_click,
        initial_state: ClickState::DoubleClick,
        initial_button: MouseButton::Left,
        input: WinitEvent::WindowEvent {
            event: WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                device_id:  DeviceId::dummy(),
            },
            window_id:  WindowId::dummy(),
        },
        end_state: ClickState::TripleClick,
        input_delay: Duration::ZERO,
    }

    test_clickstate! {
        name: triple_click_failed,
        initial_state: ClickState::DoubleClick,
        initial_button: MouseButton::Left,
        input: WinitEvent::WindowEvent {
            event: WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                device_id: DeviceId::dummy(),
            },
            window_id: WindowId::dummy(),
        },
        end_state: ClickState::Click,
        input_delay: CLICK_THRESHOLD,
    }

    test_clickstate! {
        name: multi_click_separate_buttons,
        initial_state: ClickState::DoubleClick,
        initial_button: MouseButton::Left,
        input: WinitEvent::WindowEvent {
            event: WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                device_id: DeviceId::dummy(),
            },
            window_id: WindowId::dummy(),
        },
        end_state: ClickState::Click,
        input_delay: Duration::ZERO,
    }

    test_process_binding! {
        name: process_binding_nomode_shiftmod_require_shift,
        binding: Binding { trigger: KEY, mods: ModifiersState::SHIFT, action: Action::from("\x1b[1;2D"), mode: BindingMode::empty(), notmode: BindingMode::empty() },
        triggers: true,
        mode: BindingMode::empty(),
        mods: ModifiersState::SHIFT,
    }

    test_process_binding! {
        name: process_binding_nomode_nomod_require_shift,
        binding: Binding { trigger: KEY, mods: ModifiersState::SHIFT, action: Action::from("\x1b[1;2D"), mode: BindingMode::empty(), notmode: BindingMode::empty() },
        triggers: false,
        mode: BindingMode::empty(),
        mods: ModifiersState::empty(),
    }

    test_process_binding! {
        name: process_binding_nomode_controlmod,
        binding: Binding { trigger: KEY, mods: ModifiersState::CONTROL, action: Action::from("\x1b[1;5D"), mode: BindingMode::empty(), notmode: BindingMode::empty() },
        triggers: true,
        mode: BindingMode::empty(),
        mods: ModifiersState::CONTROL,
    }

    test_process_binding! {
        name: process_binding_nomode_nomod_require_not_appcursor,
        binding: Binding { trigger: KEY, mods: ModifiersState::empty(), action: Action::from("\x1b[D"), mode: BindingMode::empty(), notmode: BindingMode::APP_CURSOR },
        triggers: true,
        mode: BindingMode::empty(),
        mods: ModifiersState::empty(),
    }

    test_process_binding! {
        name: process_binding_appcursormode_nomod_require_appcursor,
        binding: Binding { trigger: KEY, mods: ModifiersState::empty(), action: Action::from("\x1bOD"), mode: BindingMode::APP_CURSOR, notmode: BindingMode::empty() },
        triggers: true,
        mode: BindingMode::APP_CURSOR,
        mods: ModifiersState::empty(),
    }

    test_process_binding! {
        name: process_binding_nomode_nomod_require_appcursor,
        binding: Binding { trigger: KEY, mods: ModifiersState::empty(), action: Action::from("\x1bOD"), mode: BindingMode::APP_CURSOR, notmode: BindingMode::empty() },
        triggers: false,
        mode: BindingMode::empty(),
        mods: ModifiersState::empty(),
    }

    test_process_binding! {
        name: process_binding_appcursormode_appkeypadmode_nomod_require_appcursor,
        binding: Binding { trigger: KEY, mods: ModifiersState::empty(), action: Action::from("\x1bOD"), mode: BindingMode::APP_CURSOR, notmode: BindingMode::empty() },
        triggers: false,
        mode: BindingMode::empty(),
        mods: ModifiersState::empty(),
    }

    #[cfg(feature = "blocks")]
    #[test]
    fn blocks_search_binding_toggle_via_action() {
        // Build EventLoop for scheduler proxy

        // Minimal terminal and environment
        let mut clipboard = Clipboard::new_nop();
        let cfg = UiConfig::default();
        let size = SizeInfo::new(21.0, 51.0, 3.0, 3.0, 0., 0., false);
        let mut terminal = Term::new(cfg.term_options(), &size, MockEventProxy);

        // Test context implementing only what's needed
        struct BsCtx<'a, T: EventListener> {
            term: &'a mut Term<T>,
            size: SizeInfo,
            mouse: Mouse,
            clipboard: &'a mut Clipboard,
            msg: MessageBuffer,
            mods: Modifiers,
            cfg: &'a UiConfig,
            inline_state: InlineSearchState,
            blocks_active: bool,
            query: String,
        }

        impl<T: EventListener> super::ActionContext<T> for BsCtx<'_, T> {
            fn size_info(&self) -> SizeInfo {
                self.size
            }

            fn mouse_mode(&self) -> bool {
                false
            }

            fn search_next(
                &mut self,
                _origin: Point,
                _direction: Direction,
                _side: Side,
            ) -> Option<Match> {
                None
            }

            fn search_direction(&self) -> Direction {
                Direction::Right
            }

            fn search_active(&self) -> bool {
                false
            }

            fn selection_is_empty(&self) -> bool {
                true
            }

            fn mouse_mut(&mut self) -> &mut Mouse {
                &mut self.mouse
            }

            fn mouse(&self) -> &Mouse {
                &self.mouse
            }

            fn touch_purpose(&mut self) -> &mut TouchPurpose {
                unimplemented!()
            }

            fn modifiers(&mut self) -> &mut Modifiers {
                &mut self.mods
            }

            fn window(&mut self) -> &mut Window {
                unimplemented!()
            }

            fn display(&mut self) -> &mut Display {
                unimplemented!()
            }

            fn message(&self) -> Option<&Message> {
                self.msg.message()
            }

            fn config(&self) -> &UiConfig {
                self.cfg
            }

            fn clipboard_mut(&mut self) -> &mut Clipboard {
                self.clipboard
            }

            fn scheduler_mut(&mut self) -> &mut crate::scheduler::Scheduler {
                panic!("not used in this test")
            }

            fn semantic_word(&self, _point: Point) -> String {
                String::new()
            }

            fn inline_search_state(&mut self) -> &mut InlineSearchState {
                &mut self.inline_state
            }

            fn terminal(&self) -> &Term<T> {
                self.term
            }

            fn terminal_mut(&mut self) -> &mut Term<T> {
                self.term
            }

            // Blocks Search overrides
            fn open_blocks_search_panel(&mut self) {
                self.blocks_active = true;
            }

            fn close_blocks_search_panel(&mut self) {
                self.blocks_active = false;
            }

            fn blocks_search_active(&self) -> bool {
                self.blocks_active
            }

            fn blocks_search_input(&mut self, c: char) {
                self.query.push(c);
            }

            fn blocks_search_backspace(&mut self) {
                self.query.pop();
            }

            fn blocks_search_cancel(&mut self) {
                self.blocks_active = false;
            }
        }

        let mut ctx = BsCtx {
            term: &mut terminal,
            size,
            mouse: Mouse::default(),
            clipboard: &mut clipboard,
            msg: MessageBuffer::default(),
            mods: Default::default(),
            cfg: &cfg,
            inline_state: InlineSearchState::default(),
            blocks_active: false,
            query: String::new(),
        };

        // Open via action (simulating keybinding resolution)
        Action::OpenBlocksSearchPanel.execute(&mut ctx);
        assert!(ctx.blocks_active);

        // Toggle off
        Action::OpenBlocksSearchPanel.execute(&mut ctx);
        assert!(!ctx.blocks_active);
    }

    test_process_binding! {
        name: process_binding_fail_with_extra_mods,
        binding: Binding { trigger: KEY, mods: ModifiersState::SUPER, action: Action::from("arst"), mode: BindingMode::empty(), notmode: BindingMode::empty() },
        triggers: false,
        mode: BindingMode::empty(),
        mods: ModifiersState::ALT | ModifiersState::SUPER,
    }
}
