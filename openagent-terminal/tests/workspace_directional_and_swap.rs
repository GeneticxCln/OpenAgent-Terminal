#![allow(clippy::pedantic)]

#![allow(dead_code)]
//! Tests for directional pane focus and pane swap behavior.

use openagent_terminal as crate_root; // re-exported name in workspace

use crate_root::config::UiConfig;
use crate_root::display::SizeInfo;
use crate_root::workspace::{split_manager::SplitLayout, SplitManager, WorkspaceManager};

fn make_size_info() -> SizeInfo {
    // width, height, cell_w, cell_h, pad_x, pad_y, dynamic_padding
    SizeInfo::new(800.0, 600.0, 8.0, 16.0, 10.0, 10.0, false)
}

fn make_workspace() -> WorkspaceManager {
    let cfg = UiConfig::default();
    let si = make_size_info();
    WorkspaceManager::new(crate_root::workspace::WorkspaceId(0), std::rc::Rc::new(cfg), si)
}

#[test]
fn directional_focus_moves_to_nearest_in_direction() {
    let mut wm = make_workspace();
    let _tab = wm.create_tab("Test".into(), None);

    // Build 2x2 grid layout: V( H(A|B), H(C|D) )
    let a = crate_root::workspace::split_manager::PaneId(1);
    let b = crate_root::workspace::split_manager::PaneId(2);
    let c = crate_root::workspace::split_manager::PaneId(3);
    let d = crate_root::workspace::split_manager::PaneId(4);

    if let Some(tab) = wm.active_tab_mut() {
        tab.active_pane = a;
        tab.split_layout = SplitLayout::Vertical {
            top: Box::new(SplitLayout::Horizontal {
                left: Box::new(SplitLayout::Single(a)),
                right: Box::new(SplitLayout::Single(b)),
                ratio: 0.5,
            }),
            bottom: Box::new(SplitLayout::Horizontal {
                left: Box::new(SplitLayout::Single(c)),
                right: Box::new(SplitLayout::Single(d)),
                ratio: 0.5,
            }),
            ratio: 0.5,
        };
    }

    // Focus from A Right -> B
    assert!(wm.focus_pane_right());
    assert_eq!(wm.active_tab().unwrap().active_pane, b);
    // From B Down -> D
    assert!(wm.focus_pane_down());
    assert_eq!(wm.active_tab().unwrap().active_pane, d);
    // From D Left -> C
    assert!(wm.focus_pane_left());
    assert_eq!(wm.active_tab().unwrap().active_pane, c);
    // From C Up -> A
    assert!(wm.focus_pane_up());
    assert_eq!(wm.active_tab().unwrap().active_pane, a);
}

#[test]
fn swap_adjacent_panes_swaps_siblings_only() {
    let sm = SplitManager::new();
    // Horizontal split of two leaves A|B
    let a = crate_root::workspace::split_manager::PaneId(10);
    let b = crate_root::workspace::split_manager::PaneId(11);
    let mut layout = SplitLayout::Horizontal {
        left: Box::new(SplitLayout::Single(a)),
        right: Box::new(SplitLayout::Single(b)),
        ratio: 0.5,
    };

    // Swap should succeed for adjacent siblings
    let ok = sm.swap_adjacent_panes(&mut layout, a, b);
    assert!(ok, "swap should succeed for siblings");

    // Verify swapped: now left should be B, right should be A
    match &layout {
        SplitLayout::Horizontal { left, right, .. } => match (left.as_ref(), right.as_ref()) {
            (SplitLayout::Single(lid), SplitLayout::Single(rid)) => {
                assert_eq!((*lid, *rid), (b, a));
            }
            _ => panic!("unexpected structure after swap"),
        },
        _ => panic!("expected horizontal root"),
    }

    // Nested case: H( V(C|D), B ) attempt to swap non-siblings A<->C should fail
    let c = crate_root::workspace::split_manager::PaneId(12);
    let d = crate_root::workspace::split_manager::PaneId(13);
    let mut nested = SplitLayout::Horizontal {
        left: Box::new(SplitLayout::Vertical {
            top: Box::new(SplitLayout::Single(c)),
            bottom: Box::new(SplitLayout::Single(d)),
            ratio: 0.5,
        }),
        right: Box::new(SplitLayout::Single(b)),
        ratio: 0.5,
    };
    // Non-siblings swap should return false
    assert!(!sm.swap_adjacent_panes(&mut nested, c, b));
}
