//! Tab bar types for OpenAgent Terminal
//!
//! This module intentionally contains only data types used by the display layer.
//! Rendering and interactions are implemented in display/warp_ui.rs.

use std::time::Instant;

use crate::workspace::TabId;

/// Geometry information for a rendered tab bar
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct TabBarGeometry {
    pub start_line: usize,
    pub height: usize,
    pub tab_width: usize,
    pub visible_tabs: usize,
}

/// Tab animation state
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TabAnimation {
    pub tab_id: TabId,
    pub start_time: Instant,
    pub duration_ms: u32,
    pub animation_type: TabAnimationType,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum TabAnimationType {
    Open,
    Close,
    Switch,
}
