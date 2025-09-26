#![allow(clippy::pedantic)]

#[test]
fn tab_bar_hit_test_integration() {
    use openagent_terminal as termapp;
    use termapp::display::modern_ui::{hit_test_tab_bar_cached, TabBarAction};
    use termapp::workspace::TabBarPosition;

    let mut cfg = termapp::config::UiConfig::default();
    cfg.workspace.tab_bar.show = true;
    cfg.workspace.tab_bar.show_close_button = true;
    cfg.workspace.tab_bar.position = TabBarPosition::Top;

    let total_height = 600.0f32;
    let tid = termapp::workspace::TabId(42);
    let tabs = vec![(tid, 10.0, 150.0)];

    // Center click -> select
    let sel =
        hit_test_tab_bar_cached(total_height, &tabs, None, &cfg, TabBarPosition::Top, 80.0, 10.0);
    assert!(matches!(sel, Some(TabBarAction::SelectTab(id)) if id == tid));

    // Right edge -> close
    let close = hit_test_tab_bar_cached(
        total_height,
        &tabs,
        None,
        &cfg,
        TabBarPosition::Top,
        10.0 + 150.0 - 3.0,
        10.0,
    );
    assert!(matches!(close, Some(TabBarAction::CloseTab(id)) if id == tid));

    // New tab button -> create
    let btn = Some((200.0, 4.0, 20.0, 20.0));
    let create =
        hit_test_tab_bar_cached(total_height, &tabs, btn, &cfg, TabBarPosition::Top, 208.0, 10.0);
    assert!(matches!(create, Some(TabBarAction::CreateTab)));
}
