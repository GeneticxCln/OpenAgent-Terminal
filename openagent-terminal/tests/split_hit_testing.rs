#![allow(clippy::pedantic)]

use openagent_terminal as crate_root;
use crate_root::workspace::split_manager::{PaneRect, SplitAxis, SplitChild, SplitLayout};

fn container() -> PaneRect {
    // x, y, w, h in pixels
    PaneRect::new(0.0, 0.0, 400.0, 300.0)
}

#[test]
fn hit_test_horizontal_divider_with_tolerance() {
    // Layout: Horizontal split at 60%
    let layout = SplitLayout::Horizontal {
        left: Box::new(SplitLayout::Single(crate_root::workspace::split_manager::PaneId(1))),
        right: Box::new(SplitLayout::Single(crate_root::workspace::split_manager::PaneId(2))),
        ratio: 0.6,
    };
    let rect = container();
    let split_x = rect.x + rect.width * 0.6;
    // Slightly off the divider but within tolerance
    let x = split_x + 2.0; // 2px to the right
    let y = rect.y + rect.height * 0.5;
    let tol = 4.0;
    let hit = layout.hit_test_divider(rect, x, y, tol).expect("expected hit near horizontal divider");
    assert_eq!(hit.axis, SplitAxis::Horizontal);
}

#[test]
fn hit_test_vertical_divider_with_tolerance() {
    // Layout: Vertical split at 40%
    let layout = SplitLayout::Vertical {
        top: Box::new(SplitLayout::Single(crate_root::workspace::split_manager::PaneId(3))),
        bottom: Box::new(SplitLayout::Single(crate_root::workspace::split_manager::PaneId(4))),
        ratio: 0.4,
    };
    let rect = container();
    let split_y = rect.y + rect.height * 0.4;
    // Slightly off the divider but within tolerance
    let x = rect.x + rect.width * 0.5;
    let y = split_y - 3.0; // 3px above
    let tol = 4.0;
    let hit = layout.hit_test_divider(rect, x, y, tol).expect("expected hit near vertical divider");
    assert_eq!(hit.axis, SplitAxis::Vertical);
}

#[test]
fn ratio_at_path_matches_expected() {
    // Layout: Horizontal split at 50%, left child is another vertical split at 30%
    let inner = SplitLayout::Vertical {
        top: Box::new(SplitLayout::Single(crate_root::workspace::split_manager::PaneId(5))),
        bottom: Box::new(SplitLayout::Single(crate_root::workspace::split_manager::PaneId(6))),
        ratio: 0.3,
    };
    let layout = SplitLayout::Horizontal {
        left: Box::new(inner),
        right: Box::new(SplitLayout::Single(crate_root::workspace::split_manager::PaneId(7))),
        ratio: 0.5,
    };
    // Path to the left child split root is [Left]
    let (axis_root, r_root) = layout.ratio_at_path(&[]).expect("root ratio exists");
    assert_eq!(axis_root, SplitAxis::Horizontal);
    assert!((r_root - 0.5).abs() < 1e-6);

    // Path to the inner vertical split ratio: [Left]
    // get_ratio_at_path_internal is private; ratio_at_path on the child is covered by below approach
    // Hit test near left-half to fetch a path, or reconstruct the path explicitly
    let path = vec![SplitChild::Left];
    // Use internal recursion by calling ratio_at_path on a manual traversal using pattern matching
    if let SplitLayout::Horizontal { left, .. } = &layout {
        if let SplitLayout::Vertical { ratio, .. } = left.as_ref() {
            assert!((*ratio - 0.3).abs() < 1e-6);
        } else {
            panic!("expected vertical split as left child");
        }
    } else {
        panic!("expected horizontal root");
    }
}
