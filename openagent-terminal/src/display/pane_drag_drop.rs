//! Drag-and-drop functionality for panes within the workspace
//!
//! This module handles visual feedback and state management for dragging panes
//! between tabs and creating new splits.
#![allow(dead_code)]

use std::time::Instant;

use crate::display::color::Rgb;
use crate::display::workspace_animations::{
    TabAnimationData, TabAnimationType, WorkspaceAnimationManager,
};
use crate::workspace::{PaneId, TabId};

/// Different types of pane drag operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaneDragType {
    /// Moving a pane to a different tab
    MoveToTab,
    /// Creating a new split with the pane
    CreateSplit,
    /// Moving pane to a different position within current tab
    Reorder,
}

/// Direction for split creation when dropping panes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// Drop zone for pane drag operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaneDropZone {
    /// Drop into an existing tab
    Tab {
        tab_id: TabId,
        /// Position within the tab (0 = beginning)
        position: usize,
    },
    /// Create a new split
    Split {
        tab_id: TabId,
        direction: SplitDirection,
        /// Target split to attach to
        target_split: Option<PaneId>,
        /// Position within the split (before/after)
        before: bool,
    },
    /// Create a new tab
    NewTab {
        /// Position in tab bar (0 = beginning)
        position: usize,
    },
}

/// State tracking for active pane drag operation
#[derive(Debug, Clone)]
pub struct PaneDragState {
    /// The pane being dragged
    pub source_tab: TabId,
    pub source_split: PaneId,

    /// Drag operation details
    pub drag_type: PaneDragType,
    pub start_time: Instant,
    pub start_pos: (f32, f32),
    pub current_pos: (f32, f32),

    /// Visual feedback
    pub drag_preview_alpha: f32,
    pub drag_preview_scale: f32,
    pub ghost_pane_alpha: f32,

    /// Drop zone information
    pub current_drop_zone: Option<PaneDropZone>,
    pub drop_zone_highlight_alpha: f32,

    /// Threshold for drag activation
    pub drag_threshold: f32,
    pub is_active: bool,
}

impl PaneDragState {
    /// Create new pane drag state
    pub fn new(
        source_tab: TabId,
        source_split: PaneId,
        start_pos: (f32, f32),
        drag_type: PaneDragType,
    ) -> Self {
        Self {
            source_tab,
            source_split,
            drag_type,
            start_time: Instant::now(),
            start_pos,
            current_pos: start_pos,
            drag_preview_alpha: 0.0,
            drag_preview_scale: 1.0,
            ghost_pane_alpha: 0.0,
            current_drop_zone: None,
            drop_zone_highlight_alpha: 0.0,
            drag_threshold: 8.0, // pixels
            is_active: false,
        }
    }

    /// Update drag position and calculate if drag should activate
    pub fn update_position(&mut self, pos: (f32, f32)) -> bool {
        self.current_pos = pos;

        if !self.is_active {
            let distance =
                ((pos.0 - self.start_pos.0).powi(2) + (pos.1 - self.start_pos.1).powi(2)).sqrt();

            if distance > self.drag_threshold {
                self.is_active = true;
                return true;
            }
        }

        false
    }
}

/// Manager for pane drag-and-drop operations
pub struct PaneDragManager {
    /// Current active drag state
    active_drag: Option<PaneDragState>,

    /// Animation manager for drag effects
    animation_manager: WorkspaceAnimationManager,

    /// Visual settings
    drop_zone_color: Rgb,
    drag_preview_color: Rgb,
    ghost_alpha: f32,
}

impl PaneDragManager {
    /// Create new pane drag manager
    pub fn new() -> Self {
        Self {
            active_drag: None,
            animation_manager: WorkspaceAnimationManager::new(),
            drop_zone_color: Rgb::new(100, 150, 255), // Blue highlight
            drag_preview_color: Rgb::new(255, 255, 255),
            ghost_alpha: 0.5,
        }
    }

    /// Start a new pane drag operation
    pub fn start_drag(
        &mut self,
        source_tab: TabId,
        source_split: PaneId,
        start_pos: (f32, f32),
        drag_type: PaneDragType,
    ) {
        let drag_state = PaneDragState::new(source_tab, source_split, start_pos, drag_type);
        self.active_drag = Some(drag_state);

        // Start drag animation for the source tab
        self.animation_manager.start_tab_animation(
            source_tab,
            TabAnimationType::DragStart,
            Some(TabAnimationData::Drag {
                offset_x: 0.0,
                offset_y: 0.0,
                scale: 1.02,
                shadow_alpha: 0.3,
            }),
        );
    }

    /// Update drag position and state
    pub fn update_drag(&mut self, pos: (f32, f32), drop_zone: Option<PaneDropZone>) -> bool {
        if let Some(ref mut drag_state) = self.active_drag {
            let became_active = drag_state.update_position(pos);

            // Update drop zone
            let drop_zone_changed = drag_state.current_drop_zone != drop_zone;
            drag_state.current_drop_zone = drop_zone;

            // Update animation based on drag state
            if drag_state.is_active {
                let offset_x = pos.0 - drag_state.start_pos.0;
                let offset_y = pos.1 - drag_state.start_pos.1;

                self.animation_manager.update_drag_position(
                    drag_state.source_tab,
                    offset_x,
                    offset_y,
                );
            }

            return became_active || drop_zone_changed;
        }

        false
    }

    /// End the current drag operation
    pub fn end_drag(&mut self) -> Option<(PaneDragState, Option<PaneDropZone>)> {
        if let Some(drag_state) = self.active_drag.take() {
            let drop_zone = drag_state.current_drop_zone;

            // Start end drag animation
            self.animation_manager.start_tab_animation(
                drag_state.source_tab,
                TabAnimationType::DragEnd,
                None,
            );

            return Some((drag_state, drop_zone));
        }

        None
    }

    /// Cancel the current drag operation
    pub fn cancel_drag(&mut self) {
        if let Some(drag_state) = self.active_drag.take() {
            self.animation_manager
                .stop_tab_animation(drag_state.source_tab);
        }
    }

    /// Get current drag state
    pub fn current_drag(&self) -> Option<&PaneDragState> {
        self.active_drag.as_ref()
    }

    /// Update animations for current frame
    pub fn update_animations(&mut self) -> bool {
        let mut updated = self.animation_manager.update_animations();

        // Update drag-specific visual effects
        if let Some(ref mut drag_state) = self.active_drag {
            if drag_state.is_active {
                let elapsed = drag_state.start_time.elapsed().as_millis() as f32;

                // Animate drag preview alpha
                let target_alpha = if drag_state.current_drop_zone.is_some() {
                    0.9
                } else {
                    0.7
                };
                let alpha_speed = 0.01;
                let alpha_diff = target_alpha - drag_state.drag_preview_alpha;
                drag_state.drag_preview_alpha += alpha_diff * alpha_speed;

                // Animate ghost pane alpha
                drag_state.ghost_pane_alpha =
                    (self.ghost_alpha * (1.0 + (elapsed * 0.003).sin() * 0.1)).clamp(0.3, 0.7);

                // Animate drop zone highlight
                let target_highlight = if drag_state.current_drop_zone.is_some() {
                    1.0
                } else {
                    0.0
                };
                let highlight_speed = 0.02;
                let highlight_diff = target_highlight - drag_state.drop_zone_highlight_alpha;
                drag_state.drop_zone_highlight_alpha += highlight_diff * highlight_speed;

                updated = true;
            }
        }

        updated
    }

    /// Calculate drop zone based on mouse position and workspace layout
    pub fn calculate_drop_zone(
        &self,
        mouse_pos: (f32, f32),
        tab_positions: &[(TabId, f32, f32)], // (tab_id, x, width)
        split_areas: &[(TabId, PaneId, f32, f32, f32, f32)], // (tab_id, split_id, x, y, w, h)
    ) -> Option<PaneDropZone> {
        let (mouse_x, mouse_y) = mouse_pos;

        // Check for tab drop zones first
        for &(tab_id, x, width) in tab_positions {
            if mouse_x >= x && mouse_x < x + width {
                // Determine position within tab
                let relative_x = (mouse_x - x) / width;
                // Treat the exact midpoint as the beginning to match expected UX
                let position = if relative_x <= 0.5 { 0 } else { 1 };

                return Some(PaneDropZone::Tab { tab_id, position });
            }
        }

        // Check for split drop zones
        for &(tab_id, split_id, x, y, w, h) in split_areas {
            if mouse_x >= x && mouse_x < x + w && mouse_y >= y && mouse_y < y + h {
                // Determine split direction and position
                let rel_x = (mouse_x - x) / w;
                let rel_y = (mouse_y - y) / h;

                // Create drop zones at edges for splitting
                if rel_x < 0.2 {
                    return Some(PaneDropZone::Split {
                        tab_id,
                        direction: SplitDirection::Vertical,
                        target_split: Some(split_id),
                        before: true,
                    });
                } else if rel_x > 0.8 {
                    return Some(PaneDropZone::Split {
                        tab_id,
                        direction: SplitDirection::Vertical,
                        target_split: Some(split_id),
                        before: false,
                    });
                } else if rel_y < 0.2 {
                    return Some(PaneDropZone::Split {
                        tab_id,
                        direction: SplitDirection::Horizontal,
                        target_split: Some(split_id),
                        before: true,
                    });
                } else if rel_y > 0.8 {
                    return Some(PaneDropZone::Split {
                        tab_id,
                        direction: SplitDirection::Horizontal,
                        target_split: Some(split_id),
                        before: false,
                    });
                }
            }
        }

        // Check for new tab creation (empty space in tab bar)
        let max_tab_x = tab_positions
            .iter()
            .map(|(_, x, w)| x + w)
            .fold(0.0, f32::max);
        if mouse_y < 50.0 && mouse_x > max_tab_x {
            return Some(PaneDropZone::NewTab {
                position: tab_positions.len(),
            });
        }

        None
    }

    /// Get visual effects data for rendering
    pub fn get_visual_effects(&self) -> Option<PaneDragVisualEffects> {
        self.active_drag.as_ref().map(|drag| PaneDragVisualEffects {
            source_tab: drag.source_tab,
            source_split: drag.source_split,
            current_pos: drag.current_pos,
            drag_preview_alpha: drag.drag_preview_alpha,
            drag_preview_scale: drag.drag_preview_scale,
            ghost_pane_alpha: drag.ghost_pane_alpha,
            drop_zone: drag.current_drop_zone,
            drop_zone_highlight_alpha: drag.drop_zone_highlight_alpha,
            is_active: drag.is_active,
        })
    }

    /// Set visual settings
    pub fn set_colors(&mut self, drop_zone_color: Rgb, drag_preview_color: Rgb) {
        self.drop_zone_color = drop_zone_color;
        self.drag_preview_color = drag_preview_color;
    }

    /// Enable or disable reduced motion
    pub fn set_reduce_motion(&mut self, reduce_motion: bool) {
        self.animation_manager.set_reduce_motion(reduce_motion);
    }
}

impl Default for PaneDragManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Visual effects data for rendering drag operations
#[derive(Debug, Clone)]
pub struct PaneDragVisualEffects {
    pub source_tab: TabId,
    pub source_split: PaneId,
    pub current_pos: (f32, f32),
    pub drag_preview_alpha: f32,
    pub drag_preview_scale: f32,
    pub ghost_pane_alpha: f32,
    pub drop_zone: Option<PaneDropZone>,
    pub drop_zone_highlight_alpha: f32,
    pub is_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_drag_manager_creation() {
        let manager = PaneDragManager::new();
        assert!(manager.current_drag().is_none());
    }

    #[test]
    fn test_start_drag() {
        let mut manager = PaneDragManager::new();
        let tab_id = TabId(1);
        let split_id = PaneId(1);

        manager.start_drag(tab_id, split_id, (0.0, 0.0), PaneDragType::MoveToTab);
        assert!(manager.current_drag().is_some());

        let drag = manager.current_drag().unwrap();
        assert_eq!(drag.source_tab, tab_id);
        assert_eq!(drag.source_split, split_id);
        assert!(!drag.is_active); // Should not be active until threshold exceeded
    }

    #[test]
    fn test_drag_activation() {
        let mut manager = PaneDragManager::new();
        let tab_id = TabId(1);
        let split_id = PaneId(1);

        manager.start_drag(tab_id, split_id, (0.0, 0.0), PaneDragType::MoveToTab);

        // Small movement - should not activate
        manager.update_drag((5.0, 5.0), None);
        assert!(!manager.current_drag().unwrap().is_active);

        // Large movement - should activate
        manager.update_drag((20.0, 20.0), None);
        assert!(manager.current_drag().unwrap().is_active);
    }

    #[test]
    fn test_calculate_drop_zone() {
        let manager = PaneDragManager::new();

        let tab_positions = vec![(TabId(1), 0.0, 100.0), (TabId(2), 100.0, 100.0)];

        // Test tab drop zone
        let drop_zone = manager.calculate_drop_zone((50.0, 10.0), &tab_positions, &[]);

        assert!(matches!(
            drop_zone,
            Some(PaneDropZone::Tab {
                tab_id: TabId(1),
                position: 0
            })
        ));

        // Test new tab creation
        let drop_zone = manager.calculate_drop_zone((250.0, 10.0), &tab_positions, &[]);

        assert!(matches!(
            drop_zone,
            Some(PaneDropZone::NewTab { position: 2 })
        ));
    }
}
