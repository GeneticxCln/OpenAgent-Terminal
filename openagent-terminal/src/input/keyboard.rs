use std::borrow::Cow;

use std::time::Instant;
use winit::event::{ElementState, KeyEvent};
#[cfg(target_os = "macos")]
use winit::keyboard::ModifiersKeyState;
use winit::keyboard::{Key, KeyLocation, ModifiersState, NamedKey};
#[cfg(target_os = "macos")]
use winit::platform::macos::OptionAsAlt;

use openagent_terminal_core::event::EventListener;
use openagent_terminal_core::term::TermMode;
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;

use crate::config::{Action, BindingKey, BindingMode, KeyBinding};
use crate::event::TYPING_SEARCH_DELAY;
use crate::input::{ActionContext, Execute, Processor};
use crate::scheduler::{TimerId, Topic};

impl<T: EventListener, A: ActionContext<T>> Processor<T, A> {
    /// Process key input.
    pub fn key_input(&mut self, key: KeyEvent) {
        // IME input will be applied on commit and shouldn't trigger key bindings.
        if self.ctx.display().ime.preedit().is_some() {
            return;
        }

        let mode = *self.ctx.terminal().mode();
        let mods = self.ctx.modifiers().state();

        if key.state == ElementState::Released {
            if self.ctx.inline_search_state().char_pending {
                self.ctx.window().set_ime_allowed(true);
            }
            self.key_release(key, mode, mods);
            return;
        }

        let text = key.text_with_all_modifiers().unwrap_or_default();

        // All key bindings are disabled while a hint is being selected.
        if self.ctx.display().hint_state.active() {
            for character in text.chars() {
                self.ctx.hint_input(character);
            }
            return;
        }

        // Quick blocks actions via keyboard (when not in AI panel / palette)
        // Ctrl+Alt+B toggle fold under cursor
        if mods.control_key() && mods.alt_key() {
            match key.logical_key.as_ref() {
                Key::Character(c) if (c.eq_ignore_ascii_case("b")) => {
                    self.ctx.send_user_event(crate::event::EventType::BlocksToggleFoldUnderCursor);
                    return;
                },
                Key::Character(c) if (c.eq_ignore_ascii_case("c")) => {
                    self.ctx.send_user_event(crate::event::EventType::BlocksCopyHeaderUnderCursor);
                    return;
                },
                Key::Character(c) if (c.eq_ignore_ascii_case("e")) => {
                    self.ctx
                        .send_user_event(crate::event::EventType::BlocksExportHeaderUnderCursor);
                    return;
                },
                // Ctrl+Alt+M: Cycle debug split overlay (None -> H -> V -> None)
                Key::Character(c) if (c.eq_ignore_ascii_case("m")) => {
                    let next = match self.ctx.display().debug_split_overlay {
                        None => Some(false),       // Horizontal first
                        Some(false) => Some(true), // Then vertical
                        Some(true) => None,        // Then off
                    };
                    self.ctx.display().debug_split_overlay = next;
                    self.ctx.display().pending_update.dirty = true;
                    return;
                },
                _ => {},
            }
        }

        // Confirmation overlay handling takes precedence.
        if self.ctx.confirm_overlay_active() {
            match key.logical_key.as_ref() {
                Key::Named(NamedKey::Enter) => {
                    self.ctx.confirm_overlay_confirm();
                    return;
                },
                Key::Named(NamedKey::Escape) => {
                    self.ctx.confirm_overlay_cancel();
                    return;
                },
                Key::Character(c) if (c.eq_ignore_ascii_case("y")) => {
                    self.ctx.confirm_overlay_confirm();
                    return;
                },
                Key::Character(c) if (c.eq_ignore_ascii_case("n")) => {
                    self.ctx.confirm_overlay_cancel();
                    return;
                },
                _ => {},
            }
            // Swallow other keys while confirm overlay is active
            return;
        }

        // Bottom composer: click-to-focus + rich text editing while palette is inactive
        if self.ctx.display().composer_focused && self.ctx.palette_active() == false {
            use openagent_terminal_core::term::ClipboardType;
            let mods = self.ctx.modifiers().state();
            let is_mac = cfg!(target_os = "macos");
            let theme = self
                .ctx
                .config()
                .resolved_theme
                .as_ref()
                .cloned()
                .unwrap_or_else(|| self.ctx.config().theme.resolve());
            let open_mode = theme.ui.composer_open_mode.clone();

            if matches!(open_mode, crate::config::theme::ComposerOpenMode::Instant) {
                // Allow Enter or Paste (Ctrl/Cmd+V) to open the AI panel as well
                let ctrl_or_cmd = mods.control_key() || mods.super_key();
                match key.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        self.ctx.display().composer_focused = false;
                        self.ctx.display().composer_sel_anchor = None;
                        self.ctx.mark_dirty();
                        return;
                    },
                    Key::Named(NamedKey::Enter) => {
                        #[cfg(feature = "ai")]
                        {
                            self.ctx.open_ai_panel();
                            // Empty seed is fine; user starts typing in panel
                            if let Some(runtime) = self.ctx.ai_runtime_mut() {
                                runtime.ui.cursor_position = runtime.ui.scratch.len();
                            }
                            self.ctx.display().composer_text.clear();
                            self.ctx.display().composer_cursor = 0;
                            self.ctx.display().composer_sel_anchor = None;
                            self.ctx.display().composer_view_col_offset = 0;
                            self.ctx.display().composer_focused = false;
                            self.ctx.mark_dirty();
                        }
                        return;
                    },
                    Key::Named(NamedKey::Backspace) => {
                        #[cfg(feature = "ai")]
                        {
                            self.ctx.open_ai_panel();
                            if let Some(runtime) = self.ctx.ai_runtime_mut() {
                                // Perform backspace in the panel if there is any text
                                runtime.backspace();
                            }
                            self.ctx.display().composer_text.clear();
                            self.ctx.display().composer_cursor = 0;
                            self.ctx.display().composer_sel_anchor = None;
                            self.ctx.display().composer_view_col_offset = 0;
                            self.ctx.display().composer_focused = false;
                            self.ctx.mark_dirty();
                        }
                        return;
                    },
                    Key::Named(NamedKey::Delete) => {
                        #[cfg(feature = "ai")]
                        {
                            // Open panel on Delete too; perform forward delete in panel
                            self.ctx.open_ai_panel();
                            if let Some(runtime) = self.ctx.ai_runtime_mut() {
                                runtime.delete_forward();
                            }
                            self.ctx.display().composer_text.clear();
                            self.ctx.display().composer_cursor = 0;
                            self.ctx.display().composer_sel_anchor = None;
                            self.ctx.display().composer_view_col_offset = 0;
                            self.ctx.display().composer_focused = false;
                            self.ctx.mark_dirty();
                        }
                        return;
                    },
                    Key::Character(c) if ctrl_or_cmd && c.eq_ignore_ascii_case("v") => {
                        #[cfg(feature = "ai")]
                        {
                            use openagent_terminal_core::term::ClipboardType;
                            let clip = self.ctx.clipboard_mut().load(ClipboardType::Clipboard);
                            if !clip.is_empty() {
                                self.ctx.open_ai_panel();
                                if let Some(runtime) = self.ctx.ai_runtime_mut() {
                                    runtime.ui.scratch = clip;
                                    runtime.ui.cursor_position = runtime.ui.scratch.len();
                                }
                                self.ctx.display().composer_text.clear();
                                self.ctx.display().composer_cursor = 0;
                                self.ctx.display().composer_sel_anchor = None;
                                self.ctx.display().composer_view_col_offset = 0;
                                self.ctx.display().composer_focused = false;
                                self.ctx.mark_dirty();
                            }
                        }
                        return;
                    },
                    _ => {},
                }
                if !text.is_empty() {
                    #[cfg(feature = "ai")]
                    {
                        self.ctx.open_ai_panel();
                        if let Some(runtime) = self.ctx.ai_runtime_mut() {
                            runtime.ui.scratch = text.clone();
                            runtime.ui.cursor_position = runtime.ui.scratch.len();
                        }
                        // Reset composer state
                        self.ctx.display().composer_text.clear();
                        self.ctx.display().composer_cursor = 0;
                        self.ctx.display().composer_sel_anchor = None;
                        self.ctx.display().composer_view_col_offset = 0;
                        self.ctx.display().composer_focused = false;
                        self.ctx.mark_dirty();
                    }
                    return;
                }
                // Other keys in instant mode are ignored (click-to-open already available)
                return;
            }

            // Commit mode: full text editing and commit on Enter
            let word_mod = if is_mac { mods.alt_key() } else { mods.control_key() };
            let shift = mods.shift_key();
            let ctrl_or_cmd = mods.control_key() || mods.super_key();

            // Helper closures operating on the composer buffer
            let ensure_caret_visible = |ctx: &mut A| {
                // Reset blink so caret is shown immediately after edits/moves
                ctx.display().composer_caret_visible = true;
                ctx.display().composer_caret_last_toggle = Some(Instant::now());
            };
            let clear_selection = |ctx: &mut A| {
                ctx.display().composer_sel_anchor = None;
            };
            let has_selection = |ctx: &mut A| -> bool {
                ctx.display()
                    .composer_sel_anchor
                    .map(|a| a != ctx.display().composer_cursor)
                    .unwrap_or(false)
            };
            let selection_range = |ctx: &mut A| -> Option<(usize, usize)> {
                ctx.display()
                    .composer_sel_anchor
                    .map(|a| {
                        let c = ctx.display().composer_cursor;
                        if a < c {
                            (a, c)
                        } else {
                            (c, a)
                        }
                    })
                    .filter(|(s, e)| e > s)
            };
            let delete_selection = |ctx: &mut A| {
                if let Some((s, e)) = selection_range(ctx) {
                    ctx.display().composer_text.replace_range(s..e, "");
                    ctx.display().composer_cursor = s;
                    clear_selection(ctx);
                    ensure_caret_visible(ctx);
                }
            };
            let prev_char = |s: &str, idx: usize| composer_prev_char_boundary(s, idx);
            let next_char = |s: &str, idx: usize| composer_next_char_boundary(s, idx);
            let word_style = theme.ui.composer_word_boundary_style.clone();
            let prev_word = |s: &str, idx: usize| composer_prev_word_boundary(s, idx, &word_style);
            let next_word = |s: &str, idx: usize| composer_next_word_boundary(s, idx, &word_style);

            // Handle command-style shortcuts (copy/cut/paste, select all, line-home/end)
            if ctrl_or_cmd {
                match key.logical_key.as_ref() {
                    Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                        if let Some((s, e)) = selection_range(&mut self.ctx) {
                            let text = self.ctx.display().composer_text[s..e].to_string();
                            self.ctx.clipboard_mut().store(ClipboardType::Clipboard, text);
                        }
                        return;
                    },
                    Key::Character(c) if c.eq_ignore_ascii_case("x") => {
                        if let Some((s, e)) = selection_range(&mut self.ctx) {
                            let text = self.ctx.display().composer_text[s..e].to_string();
                            self.ctx.clipboard_mut().store(ClipboardType::Clipboard, text);
                            delete_selection(&mut self.ctx);
                            self.ctx.mark_dirty();
                        }
                        return;
                    },
                    Key::Character(c) if c.eq_ignore_ascii_case("v") => {
                        let clip = self.ctx.clipboard_mut().load(ClipboardType::Clipboard);
                        if !clip.is_empty() {
                            if has_selection(&mut self.ctx) {
                                delete_selection(&mut self.ctx);
                            }
                            let cur = self.ctx.display().composer_cursor;
                            self.ctx.display().composer_text.insert_str(cur, &clip);
                            self.ctx.display().composer_cursor = cur + clip.len();
                            ensure_caret_visible(&mut self.ctx);
                            self.ctx.mark_dirty();
                        }
                        return;
                    },
                    Key::Character(c) if c.eq_ignore_ascii_case("a") => {
                        // Select all
                        if !self.ctx.display().composer_text.is_empty() {
                            self.ctx.display().composer_sel_anchor = Some(0);
                            self.ctx.display().composer_cursor =
                                self.ctx.display().composer_text.len();
                            ensure_caret_visible(&mut self.ctx);
                            self.ctx.mark_dirty();
                        }
                        return;
                    },
                    Key::Character(c) if c.eq_ignore_ascii_case("e") => {
                        // End of line
                        if shift && self.ctx.display().composer_sel_anchor.is_none() {
                            self.ctx.display().composer_sel_anchor =
                                Some(self.ctx.display().composer_cursor);
                        }
                        self.ctx.display().composer_cursor = self.ctx.display().composer_text.len();
                        if !shift {
                            clear_selection(&mut self.ctx);
                        }
                        ensure_caret_visible(&mut self.ctx);
                        self.ctx.mark_dirty();
                        return;
                    },
                    Key::Character(c) if c.eq_ignore_ascii_case("h") => {
                        // Home (common in shells)
                        if shift && self.ctx.display().composer_sel_anchor.is_none() {
                            self.ctx.display().composer_sel_anchor =
                                Some(self.ctx.display().composer_cursor);
                        }
                        self.ctx.display().composer_cursor = 0;
                        if !shift {
                            clear_selection(&mut self.ctx);
                        }
                        ensure_caret_visible(&mut self.ctx);
                        self.ctx.mark_dirty();
                        return;
                    },
                    _ => {},
                }
            }

            // Route keys to composer text buffer
            match key.logical_key.as_ref() {
                Key::Named(NamedKey::Escape) => {
                    self.ctx.display().composer_focused = false;
                    // Preserve current buffer; clear selection
                    clear_selection(&mut self.ctx);
                    self.ctx.mark_dirty();
                    return;
                },
                Key::Named(NamedKey::Enter) => {
                    #[cfg(feature = "ai")]
                    {
                        let text_to_send = self.ctx.display().composer_text.clone();
                        self.ctx.open_ai_panel();
                        if let Some(runtime) = self.ctx.ai_runtime_mut() {
                            if !text_to_send.is_empty() {
                                runtime.ui.scratch = text_to_send;
                                runtime.ui.cursor_position = runtime.ui.scratch.len();
                            }
                        }
                        // Reset composer state after commit
                        self.ctx.display().composer_text.clear();
                        self.ctx.display().composer_cursor = 0;
                        self.ctx.display().composer_sel_anchor = None;
                        self.ctx.display().composer_view_col_offset = 0;
                        self.ctx.display().composer_focused = false;
                        self.ctx.mark_dirty();
                    }
                    return;
                },
                Key::Named(NamedKey::Backspace) => {
                    if has_selection(&mut self.ctx) {
                        delete_selection(&mut self.ctx);
                        self.ctx.mark_dirty();
                    } else if self.ctx.display().composer_cursor > 0 {
                        let cur = self.ctx.display().composer_cursor;
                        let prev = if word_mod {
                            prev_word(&self.ctx.display().composer_text, cur)
                        } else {
                            prev_char(&self.ctx.display().composer_text, cur)
                        };
                        self.ctx.display().composer_text.replace_range(prev..cur, "");
                        self.ctx.display().composer_cursor = prev;
                        ensure_caret_visible(&mut self.ctx);
                        self.ctx.mark_dirty();
                    }
                    return;
                },
                Key::Named(NamedKey::Delete) => {
                    if has_selection(&mut self.ctx) {
                        delete_selection(&mut self.ctx);
                        self.ctx.mark_dirty();
                    } else {
                        let cur = self.ctx.display().composer_cursor;
                        if cur < self.ctx.display().composer_text.len() {
                            let next = if word_mod {
                                next_word(&self.ctx.display().composer_text, cur)
                            } else {
                                next_char(&self.ctx.display().composer_text, cur)
                            };
                            self.ctx.display().composer_text.replace_range(cur..next, "");
                            ensure_caret_visible(&mut self.ctx);
                            self.ctx.mark_dirty();
                        }
                    }
                    return;
                },
                Key::Named(NamedKey::ArrowLeft) => {
                    if shift && self.ctx.display().composer_sel_anchor.is_none() {
                        self.ctx.display().composer_sel_anchor =
                            Some(self.ctx.display().composer_cursor);
                    }
                    let cur = self.ctx.display().composer_cursor;
                    if cur > 0 {
                        let new_cur = if word_mod {
                            prev_word(&self.ctx.display().composer_text, cur)
                        } else {
                            prev_char(&self.ctx.display().composer_text, cur)
                        };
                        self.ctx.display().composer_cursor = new_cur;
                    }
                    if !shift {
                        clear_selection(&mut self.ctx);
                    }
                    ensure_caret_visible(&mut self.ctx);
                    self.ctx.mark_dirty();
                    return;
                },
                Key::Named(NamedKey::ArrowRight) => {
                    if shift && self.ctx.display().composer_sel_anchor.is_none() {
                        self.ctx.display().composer_sel_anchor =
                            Some(self.ctx.display().composer_cursor);
                    }
                    let cur = self.ctx.display().composer_cursor;
                    let len = self.ctx.display().composer_text.len();
                    if cur < len {
                        let new_cur = if word_mod {
                            next_word(&self.ctx.display().composer_text, cur)
                        } else {
                            next_char(&self.ctx.display().composer_text, cur)
                        };
                        self.ctx.display().composer_cursor = new_cur;
                    }
                    if !shift {
                        clear_selection(&mut self.ctx);
                    }
                    ensure_caret_visible(&mut self.ctx);
                    self.ctx.mark_dirty();
                    return;
                },
                Key::Named(NamedKey::Home) => {
                    if shift && self.ctx.display().composer_sel_anchor.is_none() {
                        self.ctx.display().composer_sel_anchor =
                            Some(self.ctx.display().composer_cursor);
                    }
                    self.ctx.display().composer_cursor = 0;
                    if !shift {
                        clear_selection(&mut self.ctx);
                    }
                    ensure_caret_visible(&mut self.ctx);
                    self.ctx.mark_dirty();
                    return;
                },
                Key::Named(NamedKey::End) => {
                    if shift && self.ctx.display().composer_sel_anchor.is_none() {
                        self.ctx.display().composer_sel_anchor =
                            Some(self.ctx.display().composer_cursor);
                    }
                    self.ctx.display().composer_cursor = self.ctx.display().composer_text.len();
                    if !shift {
                        clear_selection(&mut self.ctx);
                    }
                    ensure_caret_visible(&mut self.ctx);
                    self.ctx.mark_dirty();
                    return;
                },
                _ => {},
            }

            // Insert printable text (replace selection when active)
            if !text.is_empty() {
                if has_selection(&mut self.ctx) {
                    delete_selection(&mut self.ctx);
                }
                let cur = self.ctx.display().composer_cursor;
                self.ctx.display().composer_text.insert_str(cur, &text);
                self.ctx.display().composer_cursor = cur + text.len();
                ensure_caret_visible(&mut self.ctx);
                self.ctx.mark_dirty();
                return;
            }
        }

        // Command Palette handling takes precedence.
        if self.ctx.palette_active() {
            match key.logical_key.as_ref() {
                Key::Named(NamedKey::Enter) => {
                    self.ctx.palette_confirm();
                    return;
                },
                Key::Named(NamedKey::Escape) => {
                    self.ctx.palette_cancel();
                    return;
                },
                Key::Named(NamedKey::ArrowUp) => {
                    self.ctx.palette_move_selection(-1);
                    return;
                },
                Key::Named(NamedKey::ArrowDown) => {
                    self.ctx.palette_move_selection(1);
                    return;
                },
                Key::Named(NamedKey::Backspace) => {
                    self.ctx.palette_backspace();
                    return;
                },
                _ => {},
            }
            for ch in text.chars() {
                self.ctx.palette_input(ch);
            }
            return;
        }

        // Blocks Search panel input handling (if active)
        if self.ctx.blocks_search_active() {
            let mods = self.ctx.modifiers().state();

            // Handle actions menu input if active
            if self.ctx.blocks_search_actions_menu_active() {
                match key.logical_key.as_ref() {
                    Key::Named(NamedKey::Enter) => {
                        // Execute selected action
                        self.ctx.blocks_search_execute_action();
                        return;
                    },
                    Key::Named(NamedKey::Escape) => {
                        // Close actions menu
                        self.ctx.blocks_search_close_actions_menu();
                        return;
                    },
                    Key::Named(NamedKey::ArrowUp) => {
                        self.ctx.blocks_search_move_actions_selection(-1);
                        return;
                    },
                    Key::Named(NamedKey::ArrowDown) => {
                        self.ctx.blocks_search_move_actions_selection(1);
                        return;
                    },
                    Key::Character(c) if c == "k" => {
                        self.ctx.blocks_search_move_actions_selection(-1);
                        return;
                    },
                    Key::Character(c) if c == "j" => {
                        self.ctx.blocks_search_move_actions_selection(1);
                        return;
                    },
                    _ => {},
                }
                // Don't process other keys when actions menu is active
                return;
            }

            // Handle help overlay input if active
            if self.ctx.blocks_search_help_active() {
                match key.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        // Close help
                        self.ctx.blocks_search_close_help();
                        return;
                    },
                    Key::Character(c) if c == "?" => {
                        // Close help
                        self.ctx.blocks_search_close_help();
                        return;
                    },
                    Key::Named(NamedKey::Tab) | Key::Named(NamedKey::ArrowRight) => {
                        self.ctx.blocks_search_navigate_help(true);
                        return;
                    },
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.ctx.blocks_search_navigate_help(false);
                        return;
                    },
                    _ => {},
                }
                // Don't process other keys when help is active
                return;
            }

            match key.logical_key.as_ref() {
                Key::Named(NamedKey::Enter) => {
                    self.ctx.blocks_search_confirm();
                    return;
                },
                Key::Named(NamedKey::Escape) => {
                    self.ctx.blocks_search_cancel();
                    return;
                },
                Key::Named(NamedKey::Backspace) => {
                    self.ctx.blocks_search_backspace();
                    return;
                },
                Key::Named(NamedKey::ArrowUp) => {
                    self.ctx.blocks_search_move_selection(-1);
                    return;
                },
                Key::Named(NamedKey::ArrowDown) => {
                    self.ctx.blocks_search_move_selection(1);
                    return;
                },
                Key::Named(NamedKey::PageUp) if mods.is_empty() => {
                    // Page navigation when no modifiers
                    self.ctx.blocks_search_prev_page();
                    return;
                },
                Key::Named(NamedKey::PageDown) if mods.is_empty() => {
                    // Page navigation when no modifiers
                    self.ctx.blocks_search_next_page();
                    return;
                },
                Key::Named(NamedKey::PageUp) => {
                    // Selection movement when modifiers present
                    self.ctx.blocks_search_move_selection(-5);
                    return;
                },
                Key::Named(NamedKey::PageDown) => {
                    // Selection movement when modifiers present
                    self.ctx.blocks_search_move_selection(5);
                    return;
                },
                Key::Character(c) if mods.control_key() && (c.eq_ignore_ascii_case("n")) => {
                    self.ctx.blocks_search_move_selection(1);
                    return;
                },
                Key::Character(c) if mods.control_key() && (c.eq_ignore_ascii_case("p")) => {
                    self.ctx.blocks_search_move_selection(-1);
                    return;
                },
                // Advanced search mode controls
                Key::Named(NamedKey::Tab) => {
                    // Cycle search mode: Basic -> Command -> Output -> Advanced -> Basic
                    self.ctx.blocks_search_cycle_mode();
                    return;
                },
                Key::Character(c) if mods.control_key() && (c.eq_ignore_ascii_case("s")) => {
                    // Ctrl+S: Cycle sort field
                    self.ctx.blocks_search_cycle_sort_field();
                    return;
                },
                Key::Character(c) if mods.control_key() && (c.eq_ignore_ascii_case("r")) => {
                    // Ctrl+R: Toggle sort order (reverse)
                    self.ctx.blocks_search_toggle_sort_order();
                    return;
                },
                Key::Character(c) if mods.control_key() && (c.eq_ignore_ascii_case("f")) => {
                    // Ctrl+F: Toggle starred filter
                    self.ctx.blocks_search_toggle_starred();
                    return;
                },
                Key::Character(c) if mods.control_key() && (c.eq_ignore_ascii_case("c")) => {
                    // Ctrl+C: Clear all filters
                    self.ctx.blocks_search_clear_filters();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "*") => {
                    // * : Toggle star on selected item
                    self.ctx.blocks_search_toggle_star_selected();
                    return;
                },
                Key::Character(c)
                    if !mods.control_key() && !mods.alt_key() && (c == "j" || c == "k") =>
                {
                    let delta = if c == "j" { 1 } else { -1 };
                    self.ctx.blocks_search_move_selection(delta);
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "a") => {
                    // 'a': Show actions menu for selected item
                    self.ctx.blocks_search_show_actions();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "d") => {
                    // 'd': Delete selected item
                    self.ctx.blocks_search_delete_selected();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "c") => {
                    // 'c': Copy command of selected item
                    self.ctx.blocks_search_copy_command();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "o") => {
                    // 'o': Copy output of selected item
                    self.ctx.blocks_search_copy_output();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "r") => {
                    // 'r': Rerun selected command
                    self.ctx.blocks_search_rerun_selected();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "h") => {
                    // 'h': Insert block output as here-doc
                    self.ctx.blocks_search_insert_heredoc();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "?") => {
                    // '?': Show keyboard help
                    self.ctx.blocks_search_show_help();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "e") => {
                    // 'e': Export selected block
                    self.ctx.blocks_search_export_selected();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "t") => {
                    // 't': Toggle tag on selected item
                    self.ctx.blocks_search_toggle_tag();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "b") => {
                    // 'b': Copy both command and output
                    self.ctx.blocks_search_copy_both();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "i") => {
                    // 'i': Insert command into prompt
                    self.ctx.blocks_search_insert_command();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "v") => {
                    // 'v': View full output
                    self.ctx.blocks_search_view_output();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "s") => {
                    // 's': Share block
                    self.ctx.blocks_search_share_block();
                    return;
                },
                Key::Character(c) if !mods.control_key() && !mods.alt_key() && (c == "n") => {
                    // 'n': Create snippet from command
                    self.ctx.blocks_search_create_snippet();
                    return;
                },
                Key::Named(NamedKey::Delete) => {
                    // Delete: Delete selected block with confirmation
                    self.ctx.blocks_search_delete_selected();
                    return;
                },
                _ => {},
            }
            for ch in text.chars() {
                // Ignore non-printable controls
                if !ch.is_control() {
                    self.ctx.blocks_search_input(ch);
                }
            }
            return;
        }

        // Workflows panel input handling (if active)
        #[cfg(feature = "workflow")]
        if self.ctx.workflows_panel_active() {
            let mods = self.ctx.modifiers().state();
            match key.logical_key.as_ref() {
                Key::Named(NamedKey::Enter) => {
                    self.ctx.workflows_panel_confirm();
                    return;
                },
                Key::Named(NamedKey::Escape) => {
                    self.ctx.workflows_panel_cancel();
                    return;
                },
                Key::Named(NamedKey::Backspace) => {
                    self.ctx.workflows_panel_backspace();
                    return;
                },
                Key::Named(NamedKey::ArrowUp) => {
                    self.ctx.workflows_panel_move_selection(-1);
                    return;
                },
                Key::Named(NamedKey::ArrowDown) => {
                    self.ctx.workflows_panel_move_selection(1);
                    return;
                },
                Key::Named(NamedKey::PageUp) => {
                    self.ctx.workflows_panel_move_selection(-5);
                    return;
                },
                Key::Named(NamedKey::PageDown) => {
                    self.ctx.workflows_panel_move_selection(5);
                    return;
                },
                Key::Character(c) if mods.control_key() && (c.eq_ignore_ascii_case("n")) => {
                    self.ctx.workflows_panel_move_selection(1);
                    return;
                },
                Key::Character(c) if mods.control_key() && (c.eq_ignore_ascii_case("p")) => {
                    self.ctx.workflows_panel_move_selection(-1);
                    return;
                },
                _ => {},
            }
            for ch in text.chars() {
                if !ch.is_control() {
                    self.ctx.workflows_panel_input(ch);
                }
            }
            return;
        }

        // Inline AI suggestions: accept/dismiss when visible and panel not active
        #[cfg(feature = "ai")]
        if !self.ctx.ai_active() && self.ctx.inline_suggestion_visible() {
            match key.logical_key.as_ref() {
                Key::Named(NamedKey::Tab) => {
                    self.ctx.accept_inline_suggestion();
                    return;
                },
                Key::Named(NamedKey::ArrowRight) if mods.alt_key() || mods.control_key() => {
                    self.ctx.accept_inline_suggestion_word();
                    return;
                },
                Key::Named(NamedKey::ArrowRight) => {
                    self.ctx.accept_inline_suggestion_char();
                    return;
                },
                Key::Named(NamedKey::Escape) => {
                    self.ctx.dismiss_inline_suggestion();
                    return;
                },
                _ => {},
            }
        }

        // Global AI toggle: Ctrl+Shift+A (handle even when panel is active)
        #[cfg(feature = "ai")]
        if mods.control_key() && mods.shift_key() {
            match key.logical_key.as_ref() {
                Key::Character(c) if c.eq_ignore_ascii_case("a") => {
                    self.ctx.send_user_event(crate::event::EventType::AiToggle);
                    return;
                },
                _ => {},
            }
        }

        // AI panel input handling (if active). Never auto-run; only edit/propose.
        #[cfg(feature = "ai")]
        if self.ctx.ai_active() {
            let mods = self.ctx.modifiers().state();

            // Handle Ctrl+Shift combinations first
            if mods.control_key() && mods.shift_key() {
                match key.logical_key.as_ref() {
                    // Copy as code (Ctrl+Shift+C)
                    Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                        self.ctx.send_user_event(crate::event::EventType::AiCopyCode);
                        return;
                    },
                    // Copy all (Ctrl+Shift+T)
                    Key::Character(c) if c.eq_ignore_ascii_case("t") => {
                        self.ctx.send_user_event(crate::event::EventType::AiCopyAll);
                        return;
                    },
                    _ => {},
                }
            }

            // Keyboard shortcuts inside AI panel
            if mods.control_key() {
                match key.logical_key.as_ref() {
                    // Stop/cancel streaming
                    Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                        // Ctrl+C maps here reliably across platforms
                        self.ctx.send_user_event(crate::event::EventType::AiStop);
                        return;
                    },
                    // Regenerate
                    Key::Character(c) if c.eq_ignore_ascii_case("r") => {
                        self.ctx.send_user_event(crate::event::EventType::AiRegenerate);
                        return;
                    },
                    // Insert to prompt
                    Key::Character(c) if c.eq_ignore_ascii_case("i") => {
                        if let Some(runtime) = self.ctx.ai_runtime_mut() {
                            if let Some(text) = runtime.insert_to_prompt() {
                                self.ctx.send_user_event(
                                    crate::event::EventType::AiInsertToPrompt(text),
                                );
                            }
                        }
                        return;
                    },
                    // Apply as command (Safe-run: dry-run by default)
                    Key::Character(c) if c.eq_ignore_ascii_case("e") => {
                        self.ctx.send_user_event(crate::event::EventType::AiApplyDryRun);
                        return;
                    },
                    // Explain (Ctrl+X)
                    Key::Character(c) if c.eq_ignore_ascii_case("x") => {
                        let selected_text = self.ctx.terminal().selection_to_string();
                        let target = selected_text.filter(|s| !s.is_empty());
                        self.ctx.send_user_event(crate::event::EventType::AiExplain(target));
                        return;
                    },
                    // Fix (Ctrl+F)
                    Key::Character(c) if c.eq_ignore_ascii_case("f") => {
                        let selected_text = self.ctx.terminal().selection_to_string();
                        let target = selected_text.filter(|s| !s.is_empty());
                        self.ctx.send_user_event(crate::event::EventType::AiFix(target));
                        return;
                    },
                    _ => {},
                }
            }

            match key.logical_key.as_ref() {
                Key::Named(NamedKey::Enter) => {
                    self.ctx.send_user_event(crate::event::EventType::AiSubmit);
                    return;
                },
                Key::Named(NamedKey::Escape) => {
                    self.ctx.send_user_event(crate::event::EventType::AiClose);
                    return;
                },
                Key::Named(NamedKey::ArrowUp) => {
                    // If we have proposals, navigate them; otherwise navigate history
                    if let Some(runtime) = self.ctx.ai_runtime_ref() {
                        if !runtime.ui.proposals.is_empty() {
                            self.ctx.send_user_event(crate::event::EventType::AiSelectPrev);
                        } else {
                            if let Some(runtime) = self.ctx.ai_runtime_mut() {
                                runtime.history_previous();
                                self.ctx.mark_dirty();
                            }
                        }
                    }
                    return;
                },
                Key::Named(NamedKey::ArrowDown) => {
                    // If we have proposals, navigate them; otherwise navigate history
                    if let Some(runtime) = self.ctx.ai_runtime_ref() {
                        if !runtime.ui.proposals.is_empty() {
                            self.ctx.send_user_event(crate::event::EventType::AiSelectNext);
                        } else {
                            if let Some(runtime) = self.ctx.ai_runtime_mut() {
                                runtime.history_next();
                                self.ctx.mark_dirty();
                            }
                        }
                    }
                    return;
                },
                Key::Named(NamedKey::ArrowLeft) => {
                    if let Some(runtime) = self.ctx.ai_runtime_mut() {
                        runtime.cursor_left();
                        self.ctx.mark_dirty();
                    }
                    return;
                },
                Key::Named(NamedKey::ArrowRight) => {
                    if let Some(runtime) = self.ctx.ai_runtime_mut() {
                        runtime.cursor_right();
                        self.ctx.mark_dirty();
                    }
                    return;
                },
                Key::Named(NamedKey::Backspace) => {
                    if let Some(runtime) = self.ctx.ai_runtime_mut() {
                        runtime.backspace();
                        self.ctx.mark_dirty();
                    }
                    return;
                },
                Key::Named(NamedKey::Delete) => {
                    if let Some(runtime) = self.ctx.ai_runtime_mut() {
                        runtime.delete_forward();
                        self.ctx.mark_dirty();
                    }
                    return;
                },
                _ => {},
            }

            // Only allow printable characters to be input to AI panel
            for ch in text.chars() {
                if !ch.is_control() {
                    self.ctx.ai_input(ch);
                }
            }
            return;
        }

        // First key after inline search is captured.
        let inline_state = self.ctx.inline_search_state();
        if inline_state.char_pending {
            self.ctx.inline_search_input(text);
            return;
        }

        // Reset search delay when the user is still typing.
        self.reset_search_delay();

        // Key bindings suppress the character input.
        if self.process_key_bindings(&key) {
            return;
        }

        if self.ctx.search_active() {
            for character in text.chars() {
                self.ctx.search_input(character);
            }

            return;
        }

        // Workflows progress overlay: allow Esc to dismiss when visible
        #[cfg(feature = "workflow")]
        if self.ctx.workflows_progress_active() && self.ctx.workflows_progress_terminal() {
            match key.logical_key.as_ref() {
                Key::Named(NamedKey::Escape) => {
                    self.ctx.workflows_progress_dismiss();
                    return;
                },
                _ => {},
            }
        }

        // Vi mode on its own doesn't have any input, the search input was done before.
        if mode.contains(TermMode::VI) {
            return;
        }

        // Mask `Alt` modifier from input when we won't send esc.
        let mods = if self.alt_send_esc(&key, text) { mods } else { mods & !ModifiersState::ALT };

        let build_key_sequence = Self::should_build_sequence(&key, text, mode, mods);
        let is_modifier_key = Self::is_modifier_key(&key);

        let bytes = if build_key_sequence {
            build_sequence(key, mods, mode)
        } else {
            let mut bytes = Vec::with_capacity(text.len() + 1);
            if mods.alt_key() {
                bytes.push(b'\x1b');
            }

            bytes.extend_from_slice(text.as_bytes());
            bytes
        };

        // Write only if we have something to write.
        if !bytes.is_empty() {
            // Don't clear selection/scroll down when writing escaped modifier keys.
            if !is_modifier_key {
                self.ctx.on_terminal_input_start();
            }
            self.ctx.write_to_pty(bytes);

            // Schedule AI inline suggestion after typing (debounced), and clear any stale suggestion
            #[cfg(feature = "ai")]
            {
                if !self.ctx.ai_active() {
                    self.ctx.clear_inline_suggestion();
                    self.ctx.schedule_inline_suggest();
                }
            }
        }
    }

    fn alt_send_esc(&mut self, key: &KeyEvent, text: &str) -> bool {
        #[cfg(not(target_os = "macos"))]
        let alt_send_esc = self.ctx.modifiers().state().alt_key();

        #[cfg(target_os = "macos")]
        let alt_send_esc = {
            let option_as_alt = self.ctx.config().window.option_as_alt();
            self.ctx.modifiers().state().alt_key()
                && (option_as_alt == OptionAsAlt::Both
                    || (option_as_alt == OptionAsAlt::OnlyLeft
                        && self.ctx.modifiers().lalt_state() == ModifiersKeyState::Pressed)
                    || (option_as_alt == OptionAsAlt::OnlyRight
                        && self.ctx.modifiers().ralt_state() == ModifiersKeyState::Pressed))
        };

        match key.logical_key {
            Key::Named(named) => {
                if named.to_text().is_some() {
                    alt_send_esc
                } else {
                    // Treat `Alt` as modifier for named keys without text, like ArrowUp.
                    self.ctx.modifiers().state().alt_key()
                }
            },
            _ => alt_send_esc && text.chars().count() == 1,
        }
    }

    fn is_modifier_key(key: &KeyEvent) -> bool {
        matches!(
            key.logical_key.as_ref(),
            Key::Named(NamedKey::Shift)
                | Key::Named(NamedKey::Control)
                | Key::Named(NamedKey::Alt)
                | Key::Named(NamedKey::Super)
        )
    }

    /// Check whether we should try to build escape sequence for the [`KeyEvent`].
    fn should_build_sequence(
        key: &KeyEvent,
        text: &str,
        mode: TermMode,
        mods: ModifiersState,
    ) -> bool {
        if mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC) {
            return true;
        }

        let disambiguate = mode.contains(TermMode::DISAMBIGUATE_ESC_CODES)
            && (key.logical_key == Key::Named(NamedKey::Escape)
                || key.location == KeyLocation::Numpad
                || (!mods.is_empty()
                    && (mods != ModifiersState::SHIFT
                        || matches!(
                            key.logical_key,
                            Key::Named(NamedKey::Tab)
                                | Key::Named(NamedKey::Enter)
                                | Key::Named(NamedKey::Backspace)
                        ))));

        match key.logical_key {
            _ if disambiguate => true,
            // Exclude all the named keys unless they have textual representation.
            Key::Named(named) => named.to_text().is_none(),
            _ => text.is_empty(),
        }
    }

    /// Attempt to find a binding and execute its action.
    ///
    /// The provided mode, mods, and key must match what is allowed by a binding
    /// for its action to be executed.
    fn process_key_bindings(&mut self, key: &KeyEvent) -> bool {
        let mode = BindingMode::new(self.ctx.terminal().mode(), self.ctx.search_active());
        let mods = self.ctx.modifiers().state();

        // Don't suppress char if no bindings were triggered.
        let mut suppress_chars = None;

        // We don't want the key without modifier, because it means something else most of
        // the time. However what we want is to manually lowercase the character to account
        // for both small and capital letters on regular characters at the same time.
        let logical_key = if let Key::Character(ch) = key.logical_key.as_ref() {
            // Match `Alt` bindings without `Alt` being applied, otherwise they use the
            // composed chars, which are not intuitive to bind.
            //
            // On Windows, the `Ctrl + Alt` mangles `logical_key` to unidentified values, thus
            // preventing them from being used in bindings
            //
            // For more see https://github.com/rust-windowing/winit/issues/2945.
            if (cfg!(target_os = "macos") || (cfg!(windows) && mods.control_key()))
                && mods.alt_key()
            {
                key.key_without_modifiers()
            } else {
                Key::Character(ch.to_lowercase().into())
            }
        } else {
            key.logical_key.clone()
        };

        // Get the action of a key binding.
        let mut binding_action = |binding: &KeyBinding| {
            let key = match (&binding.trigger, &logical_key) {
                (BindingKey::Scancode(_), _) => BindingKey::Scancode(key.physical_key),
                (_, code) => {
                    BindingKey::Keycode { key: code.clone(), location: key.location.into() }
                },
            };

            if binding.is_triggered_by(mode, mods, &key) {
                // Pass through the key if any of the bindings has the `ReceiveChar` action.
                *suppress_chars.get_or_insert(true) &= binding.action != Action::ReceiveChar;

                // Binding was triggered; run the action.
                Some(binding.action.clone())
            } else {
                None
            }
        };

        // Trigger matching key bindings.
        for i in 0..self.ctx.config().key_bindings().len() {
            let binding = &self.ctx.config().key_bindings()[i];
            if let Some(action) = binding_action(binding) {
                action.execute(&mut self.ctx);
            }
        }

        // Trigger key bindings for hints.
        for i in 0..self.ctx.config().hints.enabled.len() {
            let hint = &self.ctx.config().hints.enabled[i];
            let binding = match hint.binding.as_ref() {
                Some(binding) => binding.key_binding(hint),
                None => continue,
            };

            if let Some(action) = binding_action(binding) {
                action.execute(&mut self.ctx);
            }
        }

        suppress_chars.unwrap_or(false)
    }

    /// Handle key release.
    fn key_release(&mut self, key: KeyEvent, mode: TermMode, mods: ModifiersState) {
        if !mode.contains(TermMode::REPORT_EVENT_TYPES)
            || mode.contains(TermMode::VI)
            || self.ctx.search_active()
            || self.ctx.display().hint_state.active()
        {
            return;
        }

        // Mask `Alt` modifier from input when we won't send esc.
        let text = key.text_with_all_modifiers().unwrap_or_default();
        let mods = if self.alt_send_esc(&key, text) { mods } else { mods & !ModifiersState::ALT };

        let bytes = match key.logical_key.as_ref() {
            Key::Named(NamedKey::Enter)
            | Key::Named(NamedKey::Tab)
            | Key::Named(NamedKey::Backspace)
                if !mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC) =>
            {
                return;
            },
            _ => build_sequence(key, mods, mode),
        };

        self.ctx.write_to_pty(bytes);
    }

    /// Reset search delay.
    fn reset_search_delay(&mut self) {
        if self.ctx.search_active() {
            let timer_id = TimerId::new(Topic::DelayedSearch, self.ctx.window().id());
            let scheduler = self.ctx.scheduler_mut();
            if let Some(timer) = scheduler.unschedule(timer_id) {
                scheduler.schedule(timer.event, TYPING_SEARCH_DELAY, false, timer.id);
            }
        }
    }
}

/// Build a key's keyboard escape sequence based on the given `key`, `mods`, and `mode`.
///
/// The key sequences for `APP_KEYPAD` and alike are handled inside the bindings.
#[inline(never)]
fn build_sequence(key: KeyEvent, mods: ModifiersState, mode: TermMode) -> Vec<u8> {
    let mut modifiers = mods.into();

    let kitty_seq = mode.intersects(
        TermMode::REPORT_ALL_KEYS_AS_ESC
            | TermMode::DISAMBIGUATE_ESC_CODES
            | TermMode::REPORT_EVENT_TYPES,
    );

    let kitty_encode_all = mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC);
    // The default parameter is 1, so we can omit it.
    let kitty_event_type = mode.contains(TermMode::REPORT_EVENT_TYPES)
        && (key.repeat || key.state == ElementState::Released);

    let context =
        SequenceBuilder { mode, modifiers, kitty_seq, kitty_encode_all, kitty_event_type };

    let associated_text = key.text_with_all_modifiers().filter(|text| {
        mode.contains(TermMode::REPORT_ASSOCIATED_TEXT)
            && key.state != ElementState::Released
            && !text.is_empty()
            && !is_control_character(text)
    });

    let sequence_base = context
        .try_build_numpad(&key)
        .or_else(|| context.try_build_named_kitty(&key))
        .or_else(|| context.try_build_named_normal(&key, associated_text.is_some()))
        .or_else(|| context.try_build_control_char_or_mod(&key, &mut modifiers))
        .or_else(|| context.try_build_textual(&key, associated_text));

    let (payload, terminator) = match sequence_base {
        Some(SequenceBase { payload, terminator }) => (payload, terminator),
        _ => return Vec::new(),
    };

    let mut payload = format!("\x1b[{payload}");

    // Add modifiers information.
    if kitty_event_type || !modifiers.is_empty() || associated_text.is_some() {
        payload.push_str(&format!(";{}", modifiers.encode_esc_sequence()));
    }

    // Push event type.
    if kitty_event_type {
        payload.push(':');
        let event_type = match key.state {
            _ if key.repeat => '2',
            ElementState::Pressed => '1',
            ElementState::Released => '3',
        };
        payload.push(event_type);
    }

    if let Some(text) = associated_text {
        let mut codepoints = text.chars().map(u32::from);
        if let Some(codepoint) = codepoints.next() {
            payload.push_str(&format!(";{codepoint}"));
        }
        for codepoint in codepoints {
            payload.push_str(&format!(":{codepoint}"));
        }
    }

    payload.push(terminator.encode_esc_sequence());

    payload.into_bytes()
}

/// Helper to build escape sequence payloads from [`KeyEvent`].
pub struct SequenceBuilder {
    mode: TermMode,
    /// The emitted sequence should follow the kitty keyboard protocol.
    kitty_seq: bool,
    /// Encode all the keys according to the protocol.
    kitty_encode_all: bool,
    /// Report event types.
    kitty_event_type: bool,
    modifiers: SequenceModifiers,
}

impl SequenceBuilder {
    /// Try building sequence from the event's emitting text.
    fn try_build_textual(
        &self,
        key: &KeyEvent,
        associated_text: Option<&str>,
    ) -> Option<SequenceBase> {
        let character = match key.logical_key.as_ref() {
            Key::Character(character) if self.kitty_seq => character,
            _ => return None,
        };

        if character.chars().count() == 1 {
            let shift = self.modifiers.contains(SequenceModifiers::SHIFT);

            let ch = character.chars().next().unwrap();
            let unshifted_ch = if shift { ch.to_lowercase().next().unwrap() } else { ch };

            let alternate_key_code = u32::from(ch);
            let mut unicode_key_code = u32::from(unshifted_ch);

            // Try to get the base for keys which change based on modifier, like `1` for `!`.
            //
            // However it should only be performed when `SHIFT` is pressed.
            if shift && alternate_key_code == unicode_key_code {
                if let Key::Character(unmodded) = key.key_without_modifiers().as_ref() {
                    unicode_key_code = u32::from(unmodded.chars().next().unwrap_or(unshifted_ch));
                }
            }

            // NOTE: Base layouts are ignored, since winit doesn't expose this information
            // yet.
            let payload = if self.mode.contains(TermMode::REPORT_ALTERNATE_KEYS)
                && alternate_key_code != unicode_key_code
            {
                format!("{unicode_key_code}:{alternate_key_code}")
            } else {
                unicode_key_code.to_string()
            };

            Some(SequenceBase::new(payload.into(), SequenceTerminator::Kitty))
        } else if self.kitty_encode_all && associated_text.is_some() {
            // Fallback when need to report text, but we don't have any key associated with this
            // text.
            Some(SequenceBase::new("0".into(), SequenceTerminator::Kitty))
        } else {
            None
        }
    }

    /// Try building from numpad key.
    ///
    /// `None` is returned when the key is neither known nor numpad.
    fn try_build_numpad(&self, key: &KeyEvent) -> Option<SequenceBase> {
        if !self.kitty_seq || key.location != KeyLocation::Numpad {
            return None;
        }

        let base = match key.logical_key.as_ref() {
            Key::Character("0") => "57399",
            Key::Character("1") => "57400",
            Key::Character("2") => "57401",
            Key::Character("3") => "57402",
            Key::Character("4") => "57403",
            Key::Character("5") => "57404",
            Key::Character("6") => "57405",
            Key::Character("7") => "57406",
            Key::Character("8") => "57407",
            Key::Character("9") => "57408",
            Key::Character(".") => "57409",
            Key::Character("/") => "57410",
            Key::Character("*") => "57411",
            Key::Character("-") => "57412",
            Key::Character("+") => "57413",
            Key::Character("=") => "57415",
            Key::Named(named) => match named {
                NamedKey::Enter => "57414",
                NamedKey::ArrowLeft => "57417",
                NamedKey::ArrowRight => "57418",
                NamedKey::ArrowUp => "57419",
                NamedKey::ArrowDown => "57420",
                NamedKey::PageUp => "57421",
                NamedKey::PageDown => "57422",
                NamedKey::Home => "57423",
                NamedKey::End => "57424",
                NamedKey::Insert => "57425",
                NamedKey::Delete => "57426",
                _ => return None,
            },
            _ => return None,
        };

        Some(SequenceBase::new(base.into(), SequenceTerminator::Kitty))
    }

    /// Try building from [`NamedKey`] using the kitty keyboard protocol encoding
    /// for functional keys.
    fn try_build_named_kitty(&self, key: &KeyEvent) -> Option<SequenceBase> {
        let named = match key.logical_key {
            Key::Named(named) if self.kitty_seq => named,
            _ => return None,
        };

        let (base, terminator) = match named {
            // F3 in kitty protocol diverges from OpenAgent Terminal's terminfo.
            NamedKey::F3 => ("13", SequenceTerminator::Normal('~')),
            NamedKey::F13 => ("57376", SequenceTerminator::Kitty),
            NamedKey::F14 => ("57377", SequenceTerminator::Kitty),
            NamedKey::F15 => ("57378", SequenceTerminator::Kitty),
            NamedKey::F16 => ("57379", SequenceTerminator::Kitty),
            NamedKey::F17 => ("57380", SequenceTerminator::Kitty),
            NamedKey::F18 => ("57381", SequenceTerminator::Kitty),
            NamedKey::F19 => ("57382", SequenceTerminator::Kitty),
            NamedKey::F20 => ("57383", SequenceTerminator::Kitty),
            NamedKey::F21 => ("57384", SequenceTerminator::Kitty),
            NamedKey::F22 => ("57385", SequenceTerminator::Kitty),
            NamedKey::F23 => ("57386", SequenceTerminator::Kitty),
            NamedKey::F24 => ("57387", SequenceTerminator::Kitty),
            NamedKey::F25 => ("57388", SequenceTerminator::Kitty),
            NamedKey::F26 => ("57389", SequenceTerminator::Kitty),
            NamedKey::F27 => ("57390", SequenceTerminator::Kitty),
            NamedKey::F28 => ("57391", SequenceTerminator::Kitty),
            NamedKey::F29 => ("57392", SequenceTerminator::Kitty),
            NamedKey::F30 => ("57393", SequenceTerminator::Kitty),
            NamedKey::F31 => ("57394", SequenceTerminator::Kitty),
            NamedKey::F32 => ("57395", SequenceTerminator::Kitty),
            NamedKey::F33 => ("57396", SequenceTerminator::Kitty),
            NamedKey::F34 => ("57397", SequenceTerminator::Kitty),
            NamedKey::F35 => ("57398", SequenceTerminator::Kitty),
            NamedKey::ScrollLock => ("57359", SequenceTerminator::Kitty),
            NamedKey::PrintScreen => ("57361", SequenceTerminator::Kitty),
            NamedKey::Pause => ("57362", SequenceTerminator::Kitty),
            NamedKey::ContextMenu => ("57363", SequenceTerminator::Kitty),
            NamedKey::MediaPlay => ("57428", SequenceTerminator::Kitty),
            NamedKey::MediaPause => ("57429", SequenceTerminator::Kitty),
            NamedKey::MediaPlayPause => ("57430", SequenceTerminator::Kitty),
            NamedKey::MediaStop => ("57432", SequenceTerminator::Kitty),
            NamedKey::MediaFastForward => ("57433", SequenceTerminator::Kitty),
            NamedKey::MediaRewind => ("57434", SequenceTerminator::Kitty),
            NamedKey::MediaTrackNext => ("57435", SequenceTerminator::Kitty),
            NamedKey::MediaTrackPrevious => ("57436", SequenceTerminator::Kitty),
            NamedKey::MediaRecord => ("57437", SequenceTerminator::Kitty),
            NamedKey::AudioVolumeDown => ("57438", SequenceTerminator::Kitty),
            NamedKey::AudioVolumeUp => ("57439", SequenceTerminator::Kitty),
            NamedKey::AudioVolumeMute => ("57440", SequenceTerminator::Kitty),
            _ => return None,
        };

        Some(SequenceBase::new(base.into(), terminator))
    }

    /// Try building from [`NamedKey`].
    fn try_build_named_normal(
        &self,
        key: &KeyEvent,
        has_associated_text: bool,
    ) -> Option<SequenceBase> {
        let named = match key.logical_key {
            Key::Named(named) => named,
            _ => return None,
        };

        // The default parameter is 1, so we can omit it.
        let one_based =
            if self.modifiers.is_empty() && !self.kitty_event_type && !has_associated_text {
                ""
            } else {
                "1"
            };
        let (base, terminator) = match named {
            NamedKey::PageUp => ("5", SequenceTerminator::Normal('~')),
            NamedKey::PageDown => ("6", SequenceTerminator::Normal('~')),
            NamedKey::Insert => ("2", SequenceTerminator::Normal('~')),
            NamedKey::Delete => ("3", SequenceTerminator::Normal('~')),
            NamedKey::Home => (one_based, SequenceTerminator::Normal('H')),
            NamedKey::End => (one_based, SequenceTerminator::Normal('F')),
            NamedKey::ArrowLeft => (one_based, SequenceTerminator::Normal('D')),
            NamedKey::ArrowRight => (one_based, SequenceTerminator::Normal('C')),
            NamedKey::ArrowUp => (one_based, SequenceTerminator::Normal('A')),
            NamedKey::ArrowDown => (one_based, SequenceTerminator::Normal('B')),
            NamedKey::F1 => (one_based, SequenceTerminator::Normal('P')),
            NamedKey::F2 => (one_based, SequenceTerminator::Normal('Q')),
            NamedKey::F3 => (one_based, SequenceTerminator::Normal('R')),
            NamedKey::F4 => (one_based, SequenceTerminator::Normal('S')),
            NamedKey::F5 => ("15", SequenceTerminator::Normal('~')),
            NamedKey::F6 => ("17", SequenceTerminator::Normal('~')),
            NamedKey::F7 => ("18", SequenceTerminator::Normal('~')),
            NamedKey::F8 => ("19", SequenceTerminator::Normal('~')),
            NamedKey::F9 => ("20", SequenceTerminator::Normal('~')),
            NamedKey::F10 => ("21", SequenceTerminator::Normal('~')),
            NamedKey::F11 => ("23", SequenceTerminator::Normal('~')),
            NamedKey::F12 => ("24", SequenceTerminator::Normal('~')),
            NamedKey::F13 => ("25", SequenceTerminator::Normal('~')),
            NamedKey::F14 => ("26", SequenceTerminator::Normal('~')),
            NamedKey::F15 => ("28", SequenceTerminator::Normal('~')),
            NamedKey::F16 => ("29", SequenceTerminator::Normal('~')),
            NamedKey::F17 => ("31", SequenceTerminator::Normal('~')),
            NamedKey::F18 => ("32", SequenceTerminator::Normal('~')),
            NamedKey::F19 => ("33", SequenceTerminator::Normal('~')),
            NamedKey::F20 => ("34", SequenceTerminator::Normal('~')),
            _ => return None,
        };

        Some(SequenceBase::new(base.into(), terminator))
    }

    /// Try building escape from control characters (e.g. Enter) and modifiers.
    fn try_build_control_char_or_mod(
        &self,
        key: &KeyEvent,
        mods: &mut SequenceModifiers,
    ) -> Option<SequenceBase> {
        if !self.kitty_encode_all && !self.kitty_seq {
            return None;
        }

        let named = match key.logical_key {
            Key::Named(named) => named,
            _ => return None,
        };

        let base = match named {
            NamedKey::Tab => "9",
            NamedKey::Enter => "13",
            NamedKey::Escape => "27",
            NamedKey::Space => "32",
            NamedKey::Backspace => "127",
            _ => "",
        };

        // Fail when the key is not a named control character and the active mode prohibits us
        // from encoding modifier keys.
        if !self.kitty_encode_all && base.is_empty() {
            return None;
        }

        let base = match (named, key.location) {
            (NamedKey::Shift, KeyLocation::Left) => "57441",
            (NamedKey::Control, KeyLocation::Left) => "57442",
            (NamedKey::Alt, KeyLocation::Left) => "57443",
            (NamedKey::Super, KeyLocation::Left) => "57444",
            (NamedKey::Hyper, KeyLocation::Left) => "57445",
            (NamedKey::Meta, KeyLocation::Left) => "57446",
            (NamedKey::Shift, _) => "57447",
            (NamedKey::Control, _) => "57448",
            (NamedKey::Alt, _) => "57449",
            (NamedKey::Super, _) => "57450",
            (NamedKey::Hyper, _) => "57451",
            (NamedKey::Meta, _) => "57452",
            (NamedKey::CapsLock, _) => "57358",
            (NamedKey::NumLock, _) => "57360",
            _ => base,
        };

        // NOTE: Kitty's protocol mandates that the modifier state is applied before
        // key press, however winit sends them after the key press, so for modifiers
        // itself apply the state based on keysyms and not the _actual_ modifiers
        // state, which is how kitty is doing so and what is suggested in such case.
        let press = key.state.is_pressed();
        match named {
            NamedKey::Shift => mods.set(SequenceModifiers::SHIFT, press),
            NamedKey::Control => mods.set(SequenceModifiers::CONTROL, press),
            NamedKey::Alt => mods.set(SequenceModifiers::ALT, press),
            NamedKey::Super => mods.set(SequenceModifiers::SUPER, press),
            _ => (),
        }

        if base.is_empty() {
            None
        } else {
            Some(SequenceBase::new(base.into(), SequenceTerminator::Kitty))
        }
    }
}

pub struct SequenceBase {
    /// The base of the payload, which is the `number` and optionally an alt base from the kitty
    /// spec.
    payload: Cow<'static, str>,
    terminator: SequenceTerminator,
}

impl SequenceBase {
    fn new(payload: Cow<'static, str>, terminator: SequenceTerminator) -> Self {
        Self { payload, terminator }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceTerminator {
    /// The normal key esc sequence terminator defined by xterm/dec.
    Normal(char),
    /// The terminator is for kitty escape sequence.
    Kitty,
}

impl SequenceTerminator {
    fn encode_esc_sequence(self) -> char {
        match self {
            SequenceTerminator::Normal(char) => char,
            SequenceTerminator::Kitty => 'u',
        }
    }
}

bitflags::bitflags! {
    /// The modifiers encoding for escape sequence.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct SequenceModifiers : u8 {
        const SHIFT   = 0b0000_0001;
        const ALT     = 0b0000_0010;
        const CONTROL = 0b0000_0100;
        const SUPER   = 0b0000_1000;
        // NOTE: Kitty protocol defines additional modifiers to what is present here, like
        // Capslock, but it's not a modifier as per winit.
    }
}

impl SequenceModifiers {
    /// Get the value which should be passed to escape sequence.
    pub fn encode_esc_sequence(self) -> u8 {
        self.bits() + 1
    }
}

impl From<ModifiersState> for SequenceModifiers {
    fn from(mods: ModifiersState) -> Self {
        let mut modifiers = Self::empty();
        modifiers.set(Self::SHIFT, mods.shift_key());
        modifiers.set(Self::ALT, mods.alt_key());
        modifiers.set(Self::CONTROL, mods.control_key());
        modifiers.set(Self::SUPER, mods.super_key());
        modifiers
    }
}

/// Check whether the `text` is `0x7f`, `C0` or `C1` control code.
fn is_control_character(text: &str) -> bool {
    // 0x7f (DEL) is included here since it has a dedicated control code (`^?`) which generally
    // does not match the reported text (`^H`), despite not technically being part of C0 or C1.
    let codepoint = text.bytes().next().unwrap();
    text.len() == 1 && (codepoint < 0x20 || (0x7f..=0x9f).contains(&codepoint))
}

// === Composer text editing helpers ===
use crate::config::theme::WordBoundaryStyle;

fn composer_prev_char_boundary(s: &str, idx: usize) -> usize {
    if idx == 0 {
        return 0;
    }
    let mut i = idx;
    while i > 0 {
        i -= 1;
        if s.is_char_boundary(i) {
            return i;
        }
    }
    0
}

fn composer_next_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = idx;
    // Advance to next char boundary by taking current char
    if let Some(ch) = s[idx..].chars().next() {
        i += ch.len_utf8();
    } else {
        i = s.len();
    }
    i
}

fn is_word_char(ch: char, style: &WordBoundaryStyle) -> bool {
    match style {
        WordBoundaryStyle::Alnum => ch.is_alphanumeric() || ch == '_',
        WordBoundaryStyle::Unicode => ch.is_alphanumeric(),
    }
}

fn composer_prev_word_boundary(s: &str, mut idx: usize, style: &WordBoundaryStyle) -> usize {
    if idx == 0 {
        return 0;
    }
    // Skip initial whitespace
    while idx > 0 {
        let j = composer_prev_char_boundary(s, idx);
        let ch = s[j..].chars().next().unwrap();
        if ch.is_whitespace() {
            idx = j;
        } else {
            break;
        }
    }
    if idx == 0 {
        return 0;
    }
    // Determine class of the run to skip
    let mut i = idx;
    let j = composer_prev_char_boundary(s, i);
    let ch = s[j..].chars().next().unwrap();
    let target_is_word = is_word_char(ch, style);
    i = j;
    while i > 0 {
        let k = composer_prev_char_boundary(s, i);
        let ch2 = s[k..].chars().next().unwrap();
        if ch2.is_whitespace() {
            break;
        }
        if is_word_char(ch2, style) != target_is_word {
            break;
        }
        i = k;
    }
    i
}

fn composer_next_word_boundary(s: &str, mut idx: usize, style: &WordBoundaryStyle) -> usize {
    let len = s.len();
    if idx >= len {
        return len;
    }
    // Skip initial whitespace
    while idx < len {
        if let Some(ch) = s[idx..].chars().next() {
            if ch.is_whitespace() {
                idx = composer_next_char_boundary(s, idx);
            } else {
                break;
            }
        } else {
            return len;
        }
    }
    if idx >= len {
        return len;
    }
    // Determine class of the run to skip
    let ch = s[idx..].chars().next().unwrap();
    let target_is_word = is_word_char(ch, style);
    let mut i = idx;
    while i < len {
        let ch2 = s[i..].chars().next().unwrap();
        if ch2.is_whitespace() {
            break;
        }
        if is_word_char(ch2, style) != target_is_word {
            break;
        }
        i = composer_next_char_boundary(s, i);
    }
    i
}
