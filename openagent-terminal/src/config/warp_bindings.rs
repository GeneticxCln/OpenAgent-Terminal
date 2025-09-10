#![allow(dead_code)]
//! Warp-style keyboard bindings for OpenAgent Terminal
//!
//! This module defines keyboard shortcuts that match Warp Terminal's behavior and integrates
//! them based on a config toggle. On macOS, the Command (Super) modifier is used; on other
//! platforms, Control is used.

use crate::config::bindings::KeyLocation;
use crate::config::{Action, BindingKey, BindingMode, KeyBinding};
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// Create a KeyBinding for a character key.
fn kb_char(ch: &str, mods: ModifiersState, action: Action) -> KeyBinding {
    KeyBinding {
        trigger: BindingKey::Keycode {
            key: Key::Character(ch.into()),
            location: KeyLocation::Any,
        },
        mods,
        mode: BindingMode::empty(),
        notmode: BindingMode::SEARCH,
        action,
    }
}

/// Create a KeyBinding for a named key (Tab, Enter, Arrow keys).
fn kb_named(key: NamedKey, mods: ModifiersState, action: Action) -> KeyBinding {
    KeyBinding {
        trigger: BindingKey::Keycode {
            key: Key::Named(key),
            location: KeyLocation::Any,
        },
        mods,
        mode: BindingMode::empty(),
        notmode: BindingMode::SEARCH,
        action,
    }
}

/// Push binding if there is no conflicting existing binding (same trigger+mods with overlapping
/// modes).
fn push_unique(bindings: &mut Vec<KeyBinding>, binding: KeyBinding) {
    if !bindings.iter().any(|b| b.triggers_match(&binding)) {
        bindings.push(binding);
    }
}

/// Build Warp-style keybindings for macOS (Cmd-based).
#[cfg(target_os = "macos")]
fn build_warp_macos_bindings() -> Vec<KeyBinding> {
    let super_m = ModifiersState::SUPER;
    let super_shift = ModifiersState::SUPER | ModifiersState::SHIFT;
    let super_alt = ModifiersState::SUPER | ModifiersState::ALT;
    let super_ctrl = ModifiersState::SUPER | ModifiersState::CONTROL;

    let mut v = Vec::new();

    // Tabs
    push_unique(&mut v, kb_char("t", super_m, Action::CreateTab));
    push_unique(&mut v, kb_char("w", super_m, Action::CloseTab));
    push_unique(&mut v, kb_named(NamedKey::Tab, super_m, Action::NextTab));
    push_unique(
        &mut v,
        kb_named(NamedKey::Tab, super_shift, Action::PreviousTab),
    );
    push_unique(&mut v, kb_char("]", super_shift, Action::NextTab));
    push_unique(&mut v, kb_char("[", super_shift, Action::PreviousTab));
    // Tab numbers 1..9
    push_unique(&mut v, kb_char("1", super_m, Action::SelectTab1));
    push_unique(&mut v, kb_char("2", super_m, Action::SelectTab2));
    push_unique(&mut v, kb_char("3", super_m, Action::SelectTab3));
    push_unique(&mut v, kb_char("4", super_m, Action::SelectTab4));
    push_unique(&mut v, kb_char("5", super_m, Action::SelectTab5));
    push_unique(&mut v, kb_char("6", super_m, Action::SelectTab6));
    push_unique(&mut v, kb_char("7", super_m, Action::SelectTab7));
    push_unique(&mut v, kb_char("8", super_m, Action::SelectTab8));
    push_unique(&mut v, kb_char("9", super_m, Action::SelectTab9));

    // Splits (Warp: Cmd+D right, Cmd+Shift+D down)
    push_unique(&mut v, kb_char("d", super_m, Action::SplitHorizontal));
    push_unique(&mut v, kb_char("d", super_shift, Action::SplitVertical));

    // Pane navigation (Cmd+Alt+Arrows). We approximate with next/previous.
    push_unique(
        &mut v,
        kb_named(NamedKey::ArrowLeft, super_alt, Action::FocusPreviousPane),
    );
    push_unique(
        &mut v,
        kb_named(NamedKey::ArrowUp, super_alt, Action::FocusPreviousPane),
    );
    push_unique(
        &mut v,
        kb_named(NamedKey::ArrowRight, super_alt, Action::FocusNextPane),
    );
    push_unique(
        &mut v,
        kb_named(NamedKey::ArrowDown, super_alt, Action::FocusNextPane),
    );

    // Pane resizing (Cmd+Ctrl+Arrows)
    push_unique(
        &mut v,
        kb_named(NamedKey::ArrowLeft, super_ctrl, Action::ResizePaneLeft),
    );
    push_unique(
        &mut v,
        kb_named(NamedKey::ArrowRight, super_ctrl, Action::ResizePaneRight),
    );
    push_unique(
        &mut v,
        kb_named(NamedKey::ArrowUp, super_ctrl, Action::ResizePaneUp),
    );
    push_unique(
        &mut v,
        kb_named(NamedKey::ArrowDown, super_ctrl, Action::ResizePaneDown),
    );

    // Zoom (Cmd+Shift+Enter)
    push_unique(
        &mut v,
        kb_named(NamedKey::Enter, super_shift, Action::ToggleZoom),
    );

    v
}

/// Return approximate navigation action for a given logical direction.
/// Today we map Left/Up to Previous and Right/Down to Next. When true directional
/// focus actions land in Action, switch these to directional variants.
#[cfg(not(target_os = "macos"))]
fn nav_action_left() -> Action {
    Action::FocusPreviousPane
}
#[cfg(not(target_os = "macos"))]
fn nav_action_up() -> Action {
    Action::FocusPreviousPane
}
#[cfg(not(target_os = "macos"))]
fn nav_action_right() -> Action {
    Action::FocusNextPane
}
#[cfg(not(target_os = "macos"))]
fn nav_action_down() -> Action {
    Action::FocusNextPane
}

/// Build Warp-style keybindings for non-macOS platforms (Ctrl-based).
#[cfg(not(target_os = "macos"))]
fn build_warp_non_macos_bindings(existing: &[KeyBinding]) -> (Vec<KeyBinding>, bool) {
    let ctrl = ModifiersState::CONTROL;
    let ctrl_shift = ModifiersState::CONTROL | ModifiersState::SHIFT;
    let ctrl_alt = ModifiersState::CONTROL | ModifiersState::ALT;

    let mut v = Vec::new();

    // Tabs
    push_unique(&mut v, kb_char("t", ctrl, Action::CreateTab));
    push_unique(&mut v, kb_char("w", ctrl, Action::CloseTab));
    push_unique(&mut v, kb_named(NamedKey::Tab, ctrl, Action::NextTab));
    push_unique(
        &mut v,
        kb_named(NamedKey::Tab, ctrl_shift, Action::PreviousTab),
    );
    push_unique(&mut v, kb_char("]", ctrl_shift, Action::NextTab));
    push_unique(&mut v, kb_char("[", ctrl_shift, Action::PreviousTab));
    // Tab numbers 1..9
    push_unique(&mut v, kb_char("1", ctrl, Action::SelectTab1));
    push_unique(&mut v, kb_char("2", ctrl, Action::SelectTab2));
    push_unique(&mut v, kb_char("3", ctrl, Action::SelectTab3));
    push_unique(&mut v, kb_char("4", ctrl, Action::SelectTab4));
    push_unique(&mut v, kb_char("5", ctrl, Action::SelectTab5));
    push_unique(&mut v, kb_char("6", ctrl, Action::SelectTab6));
    push_unique(&mut v, kb_char("7", ctrl, Action::SelectTab7));
    push_unique(&mut v, kb_char("8", ctrl, Action::SelectTab8));
    push_unique(&mut v, kb_char("9", ctrl, Action::SelectTab9));

    // Splits (Warp: Ctrl+D right, Ctrl+Shift+D down)
    push_unique(&mut v, kb_char("d", ctrl, Action::SplitHorizontal));
    push_unique(&mut v, kb_char("d", ctrl_shift, Action::SplitVertical));

    // Determine whether default bindings already use Ctrl+Alt+Arrows for resize.
    let existing_ctrl_alt_arrows_for_resize = existing.iter().any(|b| {
        b.mods == ctrl_alt
            && matches!(
                &b.trigger,
                BindingKey::Keycode {
                    key: Key::Named(NamedKey::ArrowLeft),
                    ..
                } | BindingKey::Keycode {
                    key: Key::Named(NamedKey::ArrowRight),
                    ..
                } | BindingKey::Keycode {
                    key: Key::Named(NamedKey::ArrowUp),
                    ..
                } | BindingKey::Keycode {
                    key: Key::Named(NamedKey::ArrowDown),
                    ..
                }
            )
            && matches!(
                b.action,
                Action::ResizePaneLeft
                    | Action::ResizePaneRight
                    | Action::ResizePaneUp
                    | Action::ResizePaneDown
            )
    });

    // If default uses Ctrl+Alt+Arrows for resize, we'll adopt Warp-style by moving resize to
    // Ctrl+Shift+Arrows and using Ctrl+Alt+Arrows for navigation. We'll signal that caller
    // should strip the conflicting defaults.
    let mut should_remove_default_ctrl_alt_resize = false;
    if existing_ctrl_alt_arrows_for_resize {
        should_remove_default_ctrl_alt_resize = true;

        // Navigation (Ctrl+Alt+Arrows) -> currently approximate via prev/next
        push_unique(
            &mut v,
            kb_named(NamedKey::ArrowLeft, ctrl_alt, nav_action_left()),
        );
        push_unique(
            &mut v,
            kb_named(NamedKey::ArrowUp, ctrl_alt, nav_action_up()),
        );
        push_unique(
            &mut v,
            kb_named(NamedKey::ArrowRight, ctrl_alt, nav_action_right()),
        );
        push_unique(
            &mut v,
            kb_named(NamedKey::ArrowDown, ctrl_alt, nav_action_down()),
        );

        // Resizing (Ctrl+Shift+Arrows)
        push_unique(
            &mut v,
            kb_named(NamedKey::ArrowLeft, ctrl_shift, Action::ResizePaneLeft),
        );
        push_unique(
            &mut v,
            kb_named(NamedKey::ArrowRight, ctrl_shift, Action::ResizePaneRight),
        );
        push_unique(
            &mut v,
            kb_named(NamedKey::ArrowUp, ctrl_shift, Action::ResizePaneUp),
        );
        push_unique(
            &mut v,
            kb_named(NamedKey::ArrowDown, ctrl_shift, Action::ResizePaneDown),
        );
    } else {
        // Otherwise, if Ctrl+Alt+Arrows are free, use them for navigation as Warp suggests
        push_unique(
            &mut v,
            kb_named(NamedKey::ArrowLeft, ctrl_alt, nav_action_left()),
        );
        push_unique(
            &mut v,
            kb_named(NamedKey::ArrowUp, ctrl_alt, nav_action_up()),
        );
        push_unique(
            &mut v,
            kb_named(NamedKey::ArrowRight, ctrl_alt, nav_action_right()),
        );
        push_unique(
            &mut v,
            kb_named(NamedKey::ArrowDown, ctrl_alt, nav_action_down()),
        );
        // And keep any existing resize bindings intact.
    }

    // Zoom (Ctrl+Shift+Enter)
    push_unique(
        &mut v,
        kb_named(NamedKey::Enter, ctrl_shift, Action::ToggleZoom),
    );

    (v, should_remove_default_ctrl_alt_resize)
}

/// Integrate Warp-style keybindings into the given bindings list, avoiding conflicts and
/// respecting platform conventions.
pub fn integrate_warp_bindings(existing_bindings: &mut Vec<KeyBinding>) {
    #[cfg(target_os = "macos")]
    {
        let new_bindings = build_warp_macos_bindings();
        for b in new_bindings {
            push_unique(existing_bindings, b);
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let (new_bindings, strip_conflicting_resize) =
            build_warp_non_macos_bindings(existing_bindings);

        if strip_conflicting_resize {
            // Remove default Ctrl+Alt+Arrow resize bindings to free them for navigation.
            let ctrl_alt = ModifiersState::CONTROL | ModifiersState::ALT;
            existing_bindings.retain(|b| {
                if b.mods != ctrl_alt {
                    return true;
                }
                !matches!(
                    (&b.trigger, &b.action),
                    (
                        BindingKey::Keycode {
                            key: Key::Named(NamedKey::ArrowLeft),
                            ..
                        },
                        Action::ResizePaneLeft,
                    ) | (
                        BindingKey::Keycode {
                            key: Key::Named(NamedKey::ArrowRight),
                            ..
                        },
                        Action::ResizePaneRight,
                    ) | (
                        BindingKey::Keycode {
                            key: Key::Named(NamedKey::ArrowUp),
                            ..
                        },
                        Action::ResizePaneUp,
                    ) | (
                        BindingKey::Keycode {
                            key: Key::Named(NamedKey::ArrowDown),
                            ..
                        },
                        Action::ResizePaneDown,
                    )
                )
            });
        }

        for b in new_bindings {
            push_unique(existing_bindings, b);
        }
    }
}
