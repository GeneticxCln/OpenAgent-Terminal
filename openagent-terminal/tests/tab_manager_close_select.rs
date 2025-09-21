#![allow(clippy::pedantic)]

use openagent_terminal::workspace::TabManager;

#[test]
fn tab_manager_close_updates_active_to_previous_or_first() {
    let mut tm = TabManager::new();

    // Create three tabs A, B, C
    let a = tm.create_tab("A".into(), None);
    let b = tm.create_tab("B".into(), None);
    let c = tm.create_tab("C".into(), None);

    assert_eq!(tm.tab_count(), 3);
    assert_eq!(tm.active_tab_id(), Some(a));

    // Visit B then C so history is [C, B, A]
    assert!(tm.switch_to_tab(b));
    assert!(tm.switch_to_tab(c));
    assert_eq!(tm.active_tab_id(), Some(c));

    // Close C -> should switch to previously visited (B)
    assert!(tm.close_tab(c));
    assert_eq!(tm.tab_count(), 2);
    assert_eq!(tm.active_tab_id(), Some(b));

    // Close B -> should fall back to first available (A)
    assert!(tm.close_tab(b));
    assert_eq!(tm.tab_count(), 1);
    assert_eq!(tm.active_tab_id(), Some(a));
}
