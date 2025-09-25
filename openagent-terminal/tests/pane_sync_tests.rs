//! Pane sync broadcast tests.

#[cfg(test)]
mod pane_sync_broadcast {
    use std::rc::Rc;

    use openagent_terminal::config::UiConfig;
    use openagent_terminal::display::SizeInfo;
    use openagent_terminal::workspace::{SplitManager, WorkspaceId, WorkspaceManager};

    // Helper to make a SizeInfo
    fn size_info() -> SizeInfo {
        // width, height, cell_w, cell_h, pad_x, pad_y, dynamic_padding=false
        SizeInfo::new(800.0, 600.0, 8.0, 16.0, 4.0, 4.0, false)
    }

    #[test]
    fn broadcast_attempts_non_focused_panes() {
        let cfg = Rc::new(UiConfig::default());
        let si = size_info();
        // Enable Warp mode to get WarpIntegration
        let mut wm = WorkspaceManager::with_warp(WorkspaceId(1), cfg, si, None);
        let dummy_wid = winit::window::WindowId::dummy();
        // Initialize without an event loop (no PTYs will be spawned)
        wm.initialize_warp_for_tests_no_eventloop(dummy_wid, false).expect("initialize warp");

        // Ensure there is one tab and one pane; add a second pane to the layout
        let active_id = wm.tabs.active_tab_id().expect("active tab");
        let new_pid = wm.tabs.allocate_pane_id();
        // Rebuild split layout: Split horizontally to add the new pane
        if let Some(tab) = wm.tabs.get_tab_mut(active_id) {
            // current active pane
            let ap = tab.active_pane;
            // Build a 50/50 horizontal split: | ap | new |
            tab.split_layout = openagent_terminal::workspace::split_manager::SplitLayout::Horizontal {
                left: Box::new(openagent_terminal::workspace::split_manager::SplitLayout::Single(ap)),
                right: Box::new(openagent_terminal::workspace::split_manager::SplitLayout::Single(new_pid)),
                ratio: 0.5,
            };
        }
        // Normalize via SplitManager
        if let Some(tab) = wm.tabs.active_tab_mut() {
            let mut sm = SplitManager::new();
            sm.normalize(&mut tab.split_layout);
        }

        // Attempt broadcast from focused pane; expect one attempted (the other pane) and zero ok
        let (attempted, ok) = wm.broadcast_input_active_tab(b"echo test", wm.tabs.active_tab().map(|t| t.active_pane));
        assert_eq!(attempted, 1, "should attempt to write to the non-focused pane");
        assert_eq!(ok, 0, "no PTYs created in test mode, so writes should be zero-success");
    }
}
