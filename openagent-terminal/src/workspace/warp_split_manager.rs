//! Warp-style Split Pane Management for OpenAgent Terminal
//!
//! This module implements Warp Terminal's split pane behavior:
//! - Quick split shortcuts (Cmd+D for right, Cmd+Shift+D for down)
//! - Intuitive pane navigation (Cmd+Alt+arrows)
//! - Smart pane resizing with keyboard shortcuts
//! - Pane zoom/focus mode (Cmd+Shift+Enter)

#![allow(dead_code)]

use std::collections::VecDeque;

use super::split_manager::{PaneId, PaneRect, SplitLayout, SplitManager};

/// Enhanced split manager with Warp-style behavior
pub struct WarpSplitManager {
    /// Base split manager functionality
    base: SplitManager,

    /// Navigation history for pane focus
    focus_history: VecDeque<PaneId>,

    /// Zoom state - when a pane is "zoomed" to full size
    zoomed_pane: Option<PaneId>,
    zoomed_original_layout: Option<SplitLayout>,

    /// Resize step size for keyboard shortcuts
    resize_step: f32,

    /// Quick access to most recently used panes
    recent_panes: VecDeque<PaneId>,
}

impl WarpSplitManager {
    /// Create new Warp-style split manager
    pub fn new() -> Self {
        Self {
            base: SplitManager::new(),
            focus_history: VecDeque::new(),
            zoomed_pane: None,
            zoomed_original_layout: None,
            resize_step: 0.05, // 5% resize steps
            recent_panes: VecDeque::with_capacity(10),
        }
    }

    /// Split current pane to the right (Warp Cmd+D behavior)
    pub fn split_right(
        &mut self,
        layout: &mut SplitLayout,
        current_pane: PaneId,
        new_pane_id: PaneId,
    ) -> bool {
        self.base.split_pane(layout, current_pane, new_pane_id, 0.5, true)
    }

    /// Split current pane downward (Warp Cmd+Shift+D behavior)
    pub fn split_down(
        &mut self,
        layout: &mut SplitLayout,
        current_pane: PaneId,
        new_pane_id: PaneId,
    ) -> bool {
        self.base.split_pane(layout, current_pane, new_pane_id, 0.5, false)
    }

    /// Navigate to pane in direction (Warp Cmd+Alt+Arrow behavior)
    pub fn navigate_pane(
        &mut self,
        layout: &SplitLayout,
        current_pane: &mut PaneId,
        direction: WarpNavDirection,
    ) -> bool {
        let pane_rects =
            self.calculate_pane_positions(layout, PaneRect::new(0.0, 0.0, 100.0, 100.0));
        let current_rect =
            pane_rects.iter().find(|(id, _)| *id == *current_pane).map(|(_, rect)| *rect);

        let Some(current_rect) = current_rect else {
            return false;
        };

        // Find the best target pane in the specified direction
        let target_pane = self.find_pane_in_direction(&pane_rects, current_rect, direction);

        if let Some(target_id) = target_pane {
            self.update_focus_history(*current_pane);
            *current_pane = target_id;
            self.add_to_recent_panes(target_id);
            true
        } else {
            false
        }
    }

    /// Find the best pane in the specified direction
    fn find_pane_in_direction(
        &self,
        pane_rects: &[(PaneId, PaneRect)],
        current_rect: PaneRect,
        direction: WarpNavDirection,
    ) -> Option<PaneId> {
        let current_center_x = current_rect.x + current_rect.width / 2.0;
        let current_center_y = current_rect.y + current_rect.height / 2.0;

        let mut candidates: Vec<(PaneId, f32, f32)> = Vec::new();

        for &(pane_id, rect) in pane_rects {
            let pane_center_x = rect.x + rect.width / 2.0;
            let pane_center_y = rect.y + rect.height / 2.0;

            let is_valid_direction = match direction {
                WarpNavDirection::Left => pane_center_x < current_center_x,
                WarpNavDirection::Right => pane_center_x > current_center_x,
                WarpNavDirection::Up => pane_center_y < current_center_y,
                WarpNavDirection::Down => pane_center_y > current_center_y,
            };

            if is_valid_direction {
                // Calculate distance and alignment score
                let distance = ((pane_center_x - current_center_x).powi(2)
                    + (pane_center_y - current_center_y).powi(2))
                .sqrt();

                let alignment_score = match direction {
                    WarpNavDirection::Left | WarpNavDirection::Right => {
                        // For horizontal movement, prefer vertically aligned panes
                        1.0 / (1.0 + (pane_center_y - current_center_y).abs())
                    },
                    WarpNavDirection::Up | WarpNavDirection::Down => {
                        // For vertical movement, prefer horizontally aligned panes
                        1.0 / (1.0 + (pane_center_x - current_center_x).abs())
                    },
                };

                candidates.push((pane_id, distance, alignment_score));
            }
        }

        // Sort by alignment first, then distance
        candidates.sort_by(|a, b| {
            b.2.partial_cmp(&a.2)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        });

        candidates.first().map(|(id, ..)| *id)
    }

    /// Resize current pane (Warp Cmd+Alt+Arrow with modifiers behavior)
    pub fn resize_pane(
        &mut self,
        layout: &mut SplitLayout,
        current_pane: PaneId,
        direction: WarpResizeDirection,
    ) -> bool {
        let delta = match direction {
            WarpResizeDirection::ExpandLeft | WarpResizeDirection::ShrinkRight => -self.resize_step,
            WarpResizeDirection::ExpandRight | WarpResizeDirection::ShrinkLeft => self.resize_step,
            WarpResizeDirection::ExpandUp | WarpResizeDirection::ShrinkDown => -self.resize_step,
            WarpResizeDirection::ExpandDown | WarpResizeDirection::ShrinkUp => self.resize_step,
        };

        self.base.resize_split(layout, current_pane, delta)
    }

    /// Toggle pane zoom (Warp Cmd+Shift+Enter behavior)
    pub fn toggle_pane_zoom(&mut self, layout: &mut SplitLayout, current_pane: PaneId) -> bool {
        if let Some(zoomed) = self.zoomed_pane {
            if zoomed == current_pane {
                // Unzoom - restore original layout
                if let Some(original) = self.zoomed_original_layout.take() {
                    *layout = original;
                    self.zoomed_pane = None;
                    return true;
                }
            }
        }

        // Zoom current pane - save original layout and set to single pane
        if layout.find_pane(current_pane) {
            self.zoomed_original_layout = Some(layout.clone());
            *layout = SplitLayout::Single(current_pane);
            self.zoomed_pane = Some(current_pane);
            return true;
        }

        false
    }

    /// Check if a pane is currently zoomed
    pub fn is_pane_zoomed(&self, pane_id: PaneId) -> bool {
        self.zoomed_pane == Some(pane_id)
    }

    /// Cycle through recent panes (Warp Cmd+; behavior)
    pub fn cycle_recent_panes(&mut self, current_pane: &mut PaneId) -> bool {
        if self.recent_panes.len() < 2 {
            return false;
        }

        // Move current pane to end and get the next recent pane
        if let Some(pos) = self.recent_panes.iter().position(|&id| id == *current_pane) {
            let next_pos = (pos + 1) % self.recent_panes.len();
            let next_pane = self.recent_panes[next_pos];

            self.update_focus_history(*current_pane);
            *current_pane = next_pane;

            // Move the selected pane to the front of recent panes
            if let Some(idx) = self.recent_panes.iter().position(|&id| id == next_pane) {
                let pane = self.recent_panes.remove(idx).unwrap();
                self.recent_panes.push_front(pane);
            }

            true
        } else {
            false
        }
    }

    /// Close pane with smart focus handling
    pub fn close_pane_smart(
        &mut self,
        layout: &mut SplitLayout,
        pane_id: PaneId,
        current_pane: &mut PaneId,
    ) -> bool {
        // Before closing, determine where focus should go
        let next_focus = if *current_pane == pane_id {
            // Closing the focused pane - try to focus the most recent pane
            self.recent_panes
                .iter()
                .find(|&&id| id != pane_id && layout.find_pane(id))
                .copied()
                .or_else(|| {
                    // Fall back to first available pane
                    let panes = layout.collect_pane_ids();
                    panes.iter().find(|&&id| id != pane_id).copied()
                })
        } else {
            Some(*current_pane)
        };

        // Remove from our tracking
        self.remove_pane_from_tracking(pane_id);

        // Close the pane
        if self.base.close_pane(layout, pane_id) {
            // Update focus if needed
            if let Some(next) = next_focus {
                if layout.find_pane(next) {
                    *current_pane = next;
                }
            } else {
                // No panes left, this shouldn't happen in normal usage
                let remaining_panes = layout.collect_pane_ids();
                if let Some(&first_pane) = remaining_panes.first() {
                    *current_pane = first_pane;
                }
            }
            true
        } else {
            false
        }
    }

    /// Swap panes (Warp-style pane reordering)
    pub fn swap_panes(&mut self, layout: &mut SplitLayout, pane1: PaneId, pane2: PaneId) -> bool {
        if pane1 == pane2 {
            return false;
        }

        // This is a complex operation that would require deep layout manipulation
        // For now, we'll implement a simpler version that works for adjacent panes
        self.swap_adjacent_panes(layout, pane1, pane2)
    }

    /// Swap adjacent panes in a split
#[allow(clippy::only_used_in_recursion)]
    fn swap_adjacent_panes(&self, layout: &mut SplitLayout, pane1: PaneId, pane2: PaneId) -> bool {
        match layout {
            SplitLayout::Horizontal { left, right, .. } => {
                if let (SplitLayout::Single(id1), SplitLayout::Single(id2)) =
                    (left.as_ref(), right.as_ref())
                {
                    if (*id1 == pane1 && *id2 == pane2) || (*id1 == pane2 && *id2 == pane1) {
                        std::mem::swap(left, right);
                        return true;
                    }
                }
                // Recursively check children
                self.swap_adjacent_panes(left, pane1, pane2)
                    || self.swap_adjacent_panes(right, pane1, pane2)
            },
            SplitLayout::Vertical { top, bottom, .. } => {
                if let (SplitLayout::Single(id1), SplitLayout::Single(id2)) =
                    (top.as_ref(), bottom.as_ref())
                {
                    if (*id1 == pane1 && *id2 == pane2) || (*id1 == pane2 && *id2 == pane1) {
                        std::mem::swap(top, bottom);
                        return true;
                    }
                }
                // Recursively check children
                self.swap_adjacent_panes(top, pane1, pane2)
                    || self.swap_adjacent_panes(bottom, pane1, pane2)
            },
            SplitLayout::Single(_) => false,
        }
    }

    /// Equalize split ratios (Warp Cmd+= behavior)
    pub fn equalize_splits(&self, layout: &mut SplitLayout) {
        self.equalize_splits_recursive(layout);
    }

    /// Recursively equalize all split ratios
#[allow(clippy::only_used_in_recursion)]
    fn equalize_splits_recursive(&self, layout: &mut SplitLayout) {
        match layout {
            SplitLayout::Horizontal { left, right, ratio } => {
                *ratio = 0.5;
                self.equalize_splits_recursive(left);
                self.equalize_splits_recursive(right);
            },
            SplitLayout::Vertical { top, bottom, ratio } => {
                *ratio = 0.5;
                self.equalize_splits_recursive(top);
                self.equalize_splits_recursive(bottom);
            },
            SplitLayout::Single(_) => {
                // Nothing to equalize
            },
        }
    }

    /// Calculate pane positions for navigation
    fn calculate_pane_positions(
        &self,
        layout: &SplitLayout,
        container: PaneRect,
    ) -> Vec<(PaneId, PaneRect)> {
        self.base.calculate_pane_rects(layout, container)
    }

    /// Update focus history
    fn update_focus_history(&mut self, pane_id: PaneId) {
        // Remove if already exists
        if let Some(pos) = self.focus_history.iter().position(|&id| id == pane_id) {
            self.focus_history.remove(pos);
        }

        // Add to front
        self.focus_history.push_front(pane_id);

        // Keep reasonable history size
        if self.focus_history.len() > 20 {
            self.focus_history.pop_back();
        }
    }

    /// Add pane to recent panes list
    fn add_to_recent_panes(&mut self, pane_id: PaneId) {
        // Remove if already exists
        if let Some(pos) = self.recent_panes.iter().position(|&id| id == pane_id) {
            self.recent_panes.remove(pos);
        }

        // Add to front
        self.recent_panes.push_front(pane_id);

        // Keep reasonable size
        while self.recent_panes.len() > 10 {
            self.recent_panes.pop_back();
        }
    }

    /// Remove pane from all tracking structures
    fn remove_pane_from_tracking(&mut self, pane_id: PaneId) {
        // Remove from focus history
        self.focus_history.retain(|&id| id != pane_id);

        // Remove from recent panes
        self.recent_panes.retain(|&id| id != pane_id);

        // Clear zoom if this pane was zoomed
        if self.zoomed_pane == Some(pane_id) {
            self.zoomed_pane = None;
            self.zoomed_original_layout = None;
        }
    }

    /// Get focus history for debugging/UI
    pub fn get_focus_history(&self) -> &VecDeque<PaneId> {
        &self.focus_history
    }

    /// Go back to previous pane (Warp Cmd+[ behavior)
    pub fn focus_previous_pane(&mut self, current_pane: &mut PaneId, layout: &SplitLayout) -> bool {
        if self.focus_history.len() < 2 {
            return false;
        }

        // Find the most recent pane that still exists and isn't current
        for &pane_id in &self.focus_history {
            if pane_id != *current_pane && layout.find_pane(pane_id) {
                self.update_focus_history(*current_pane);
                *current_pane = pane_id;
                self.add_to_recent_panes(pane_id);
                return true;
            }
        }

        false
    }
}

/// Navigation directions for Warp-style pane navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpNavDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Resize directions for Warp-style pane resizing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpResizeDirection {
    ExpandLeft,
    ExpandRight,
    ExpandUp,
    ExpandDown,
    ShrinkLeft,
    ShrinkRight,
    ShrinkUp,
    ShrinkDown,
}

impl Default for WarpSplitManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper trait to extend base SplitManager functionality
trait SplitManagerExt {
    fn split_pane(
        &mut self,
        layout: &mut SplitLayout,
        target_id: PaneId,
        new_pane_id: PaneId,
        ratio: f32,
        horizontal: bool,
    ) -> bool;
}

impl SplitManagerExt for SplitManager {
    fn split_pane(
        &mut self,
        layout: &mut SplitLayout,
        target_id: PaneId,
        _new_pane_id: PaneId,
        ratio: f32,
        horizontal: bool,
    ) -> bool {
        if horizontal {
            self.split_horizontal(layout, target_id, ratio).is_some()
        } else {
            self.split_vertical(layout, target_id, ratio).is_some()
        }
    }
}
