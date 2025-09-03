//! Split pane management for OpenAgent Terminal
//!
//! This module handles the creation, resizing, and navigation of split panes within tabs.

#![allow(dead_code)]

/// Unique identifier for a pane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PaneId(pub usize);

/// Layout structure for split panes
#[derive(Debug, Clone)]
pub enum SplitLayout {
    /// Single pane with no splits
    Single(PaneId),

    /// Horizontal split (left | right)
    Horizontal { left: Box<SplitLayout>, right: Box<SplitLayout>, ratio: f32 },

    /// Vertical split (top / bottom)
    Vertical { top: Box<SplitLayout>, bottom: Box<SplitLayout>, ratio: f32 },
}

impl SplitLayout {
    /// Find a pane by ID in the layout tree
    pub fn find_pane(&self, pane_id: PaneId) -> bool {
        match self {
            SplitLayout::Single(id) => *id == pane_id,
            SplitLayout::Horizontal { left, right, .. } => {
                left.find_pane(pane_id) || right.find_pane(pane_id)
            },
            SplitLayout::Vertical { top, bottom, .. } => {
                top.find_pane(pane_id) || bottom.find_pane(pane_id)
            },
        }
    }

    /// Collect all pane IDs in the layout
    pub fn collect_pane_ids(&self) -> Vec<PaneId> {
        let mut ids = Vec::new();
        self.collect_pane_ids_recursive(&mut ids);
        ids
    }

    fn collect_pane_ids_recursive(&self, ids: &mut Vec<PaneId>) {
        match self {
            SplitLayout::Single(id) => ids.push(*id),
            SplitLayout::Horizontal { left, right, .. } => {
                left.collect_pane_ids_recursive(ids);
                right.collect_pane_ids_recursive(ids);
            },
            SplitLayout::Vertical { top, bottom, .. } => {
                top.collect_pane_ids_recursive(ids);
                bottom.collect_pane_ids_recursive(ids);
            },
        }
    }

    /// Count the number of panes in the layout
    pub fn pane_count(&self) -> usize {
        match self {
            SplitLayout::Single(_) => 1,
            SplitLayout::Horizontal { left, right, .. } => left.pane_count() + right.pane_count(),
            SplitLayout::Vertical { top, bottom, .. } => top.pane_count() + bottom.pane_count(),
        }
    }
}

/// Rectangle representing a pane's position and size
#[derive(Debug, Clone, Copy)]
pub struct PaneRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PaneRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    /// Split this rectangle horizontally at the given ratio
    pub fn split_horizontal(&self, ratio: f32) -> (PaneRect, PaneRect) {
        let left_width = self.width * ratio;
        let right_width = self.width * (1.0 - ratio);

        let left = PaneRect::new(self.x, self.y, left_width, self.height);
        let right = PaneRect::new(self.x + left_width, self.y, right_width, self.height);

        (left, right)
    }

    /// Split this rectangle vertically at the given ratio
    pub fn split_vertical(&self, ratio: f32) -> (PaneRect, PaneRect) {
        let top_height = self.height * ratio;
        let bottom_height = self.height * (1.0 - ratio);

        let top = PaneRect::new(self.x, self.y, self.width, top_height);
        let bottom = PaneRect::new(self.x, self.y + top_height, self.width, bottom_height);

        (top, bottom)
    }
}

/// Split manager handles pane layouts and operations
pub struct SplitManager {
    /// Minimum pane size in cells
    minimum_pane_size: usize,

    /// Default split ratio for new splits
    default_split_ratio: f32,
}

impl SplitManager {
    /// Static convenience: split horizontally without borrowing a SplitManager instance.
    pub fn split_horizontal_static(
        layout: &mut SplitLayout,
        pane_id: PaneId,
        ratio: f32,
    ) -> Option<PaneId> {
        SplitManager::new().split_horizontal(layout, pane_id, ratio)
    }

    /// Static convenience: split vertically without borrowing a SplitManager instance.
    pub fn split_vertical_static(
        layout: &mut SplitLayout,
        pane_id: PaneId,
        ratio: f32,
    ) -> Option<PaneId> {
        SplitManager::new().split_vertical(layout, pane_id, ratio)
    }

    /// Static convenience: focus next pane without borrowing a SplitManager instance.
    pub fn focus_next_pane_static(layout: &SplitLayout, current_pane: &mut PaneId) -> bool {
        SplitManager::new().focus_next_pane(layout, current_pane)
    }

    /// Static convenience: focus previous pane without borrowing a SplitManager instance.
    pub fn focus_previous_pane_static(layout: &SplitLayout, current_pane: &mut PaneId) -> bool {
        SplitManager::new().focus_previous_pane(layout, current_pane)
    }

    /// Static convenience: close a pane without borrowing a SplitManager instance.
    pub fn close_pane_static(layout: &mut SplitLayout, pane_id: PaneId) -> bool {
        SplitManager::new().close_pane(layout, pane_id)
    }

    /// Static convenience: count panes.
    pub fn pane_count_static(layout: &SplitLayout) -> usize {
        layout.pane_count()
    }

    /// Static convenience: resize split without borrowing a SplitManager instance.
    pub fn resize_split_static(layout: &mut SplitLayout, pane_id: PaneId, delta: f32) -> bool {
        SplitManager::new().resize_split(layout, pane_id, delta)
    }
    /// Create a new split manager
    pub fn new() -> Self {
        Self { minimum_pane_size: 10, default_split_ratio: 0.5 }
    }

    /// Create a new split manager with configuration
    pub fn with_config(minimum_pane_size: usize, default_split_ratio: f32) -> Self {
        Self { minimum_pane_size, default_split_ratio }
    }

    /// Split a pane horizontally
    pub fn split_horizontal(
        &self,
        layout: &mut SplitLayout,
        pane_id: PaneId,
        ratio: f32,
    ) -> Option<PaneId> {
        let new_pane_id = PaneId(pane_id.0 + 1000); // Simple ID generation, should be improved

        if self.split_pane(layout, pane_id, new_pane_id, ratio, true) {
            Some(new_pane_id)
        } else {
            None
        }
    }

    /// Split a pane vertically
    pub fn split_vertical(
        &self,
        layout: &mut SplitLayout,
        pane_id: PaneId,
        ratio: f32,
    ) -> Option<PaneId> {
        let new_pane_id = PaneId(pane_id.0 + 1000); // Simple ID generation, should be improved

        if self.split_pane(layout, pane_id, new_pane_id, ratio, false) {
            Some(new_pane_id)
        } else {
            None
        }
    }

    /// Internal method to split a pane
    #[allow(clippy::only_used_in_recursion)]
    fn split_pane(
        &self,
        layout: &mut SplitLayout,
        target_id: PaneId,
        new_pane_id: PaneId,
        ratio: f32,
        horizontal: bool,
    ) -> bool {
        match layout {
            SplitLayout::Single(id) if *id == target_id => {
                let new_layout = if horizontal {
                    SplitLayout::Horizontal {
                        left: Box::new(SplitLayout::Single(target_id)),
                        right: Box::new(SplitLayout::Single(new_pane_id)),
                        ratio,
                    }
                } else {
                    SplitLayout::Vertical {
                        top: Box::new(SplitLayout::Single(target_id)),
                        bottom: Box::new(SplitLayout::Single(new_pane_id)),
                        ratio,
                    }
                };
                *layout = new_layout;
                true
            },
            SplitLayout::Horizontal { left, right, .. } => {
                self.split_pane(left, target_id, new_pane_id, ratio, horizontal)
                    || self.split_pane(right, target_id, new_pane_id, ratio, horizontal)
            },
            SplitLayout::Vertical { top, bottom, .. } => {
                self.split_pane(top, target_id, new_pane_id, ratio, horizontal)
                    || self.split_pane(bottom, target_id, new_pane_id, ratio, horizontal)
            },
            _ => false,
        }
    }

    /// Close a pane and rebalance the layout
    pub fn close_pane(&self, layout: &mut SplitLayout, pane_id: PaneId) -> bool {
        self.remove_pane(layout, pane_id).is_some()
    }

    /// Remove a pane from the layout tree
    #[allow(clippy::only_used_in_recursion)]
    fn remove_pane(&self, layout: &mut SplitLayout, pane_id: PaneId) -> Option<SplitLayout> {
        match layout {
            SplitLayout::Single(id) if *id == pane_id => {
                // Can't remove the last pane
                None
            },
            SplitLayout::Horizontal { left, right, .. } => {
                if let SplitLayout::Single(id) = left.as_ref() {
                    if *id == pane_id {
                        let new_layout = *right.clone();
                        *layout = new_layout;
                        return Some(layout.clone());
                    }
                }
                if let SplitLayout::Single(id) = right.as_ref() {
                    if *id == pane_id {
                        let new_layout = *left.clone();
                        *layout = new_layout;
                        return Some(layout.clone());
                    }
                }

                // Recursively search in children
                self.remove_pane(left, pane_id).or_else(|| self.remove_pane(right, pane_id))
            },
            SplitLayout::Vertical { top, bottom, .. } => {
                if let SplitLayout::Single(id) = top.as_ref() {
                    if *id == pane_id {
                        let new_layout = *bottom.clone();
                        *layout = new_layout;
                        return Some(layout.clone());
                    }
                }
                if let SplitLayout::Single(id) = bottom.as_ref() {
                    if *id == pane_id {
                        let new_layout = *top.clone();
                        *layout = new_layout;
                        return Some(layout.clone());
                    }
                }

                // Recursively search in children
                self.remove_pane(top, pane_id).or_else(|| self.remove_pane(bottom, pane_id))
            },
            _ => None,
        }
    }

    /// Focus the next pane in the layout
    pub fn focus_next_pane(&self, layout: &SplitLayout, current_pane: &mut PaneId) -> bool {
        let panes = layout.collect_pane_ids();
        if let Some(current_index) = panes.iter().position(|&id| id == *current_pane) {
            let next_index = (current_index + 1) % panes.len();
            *current_pane = panes[next_index];
            true
        } else {
            false
        }
    }

    /// Focus the previous pane in the layout
    pub fn focus_previous_pane(&self, layout: &SplitLayout, current_pane: &mut PaneId) -> bool {
        let panes = layout.collect_pane_ids();
        if let Some(current_index) = panes.iter().position(|&id| id == *current_pane) {
            let prev_index = if current_index == 0 { panes.len() - 1 } else { current_index - 1 };
            *current_pane = panes[prev_index];
            true
        } else {
            false
        }
    }

    /// Calculate pane rectangles for a given layout and container size
    pub fn calculate_pane_rects(
        &self,
        layout: &SplitLayout,
        container: PaneRect,
    ) -> Vec<(PaneId, PaneRect)> {
        let mut rects = Vec::new();
        self.calculate_rects_recursive(layout, container, &mut rects);
        rects
    }

    #[allow(clippy::only_used_in_recursion)]
    fn calculate_rects_recursive(
        &self,
        layout: &SplitLayout,
        rect: PaneRect,
        rects: &mut Vec<(PaneId, PaneRect)>,
    ) {
        match layout {
            SplitLayout::Single(id) => {
                rects.push((*id, rect));
            },
            SplitLayout::Horizontal { left, right, ratio } => {
                let (left_rect, right_rect) = rect.split_horizontal(*ratio);
                self.calculate_rects_recursive(left, left_rect, rects);
                self.calculate_rects_recursive(right, right_rect, rects);
            },
            SplitLayout::Vertical { top, bottom, ratio } => {
                let (top_rect, bottom_rect) = rect.split_vertical(*ratio);
                self.calculate_rects_recursive(top, top_rect, rects);
                self.calculate_rects_recursive(bottom, bottom_rect, rects);
            },
        }
    }

    /// Resize a split by adjusting its ratio
    pub fn resize_split(&self, layout: &mut SplitLayout, pane_id: PaneId, delta: f32) -> bool {
        self.adjust_split_ratio(layout, pane_id, delta)
    }

    #[allow(clippy::only_used_in_recursion)]
    fn adjust_split_ratio(&self, layout: &mut SplitLayout, pane_id: PaneId, delta: f32) -> bool {
        match layout {
            SplitLayout::Horizontal { left, right, ratio } => {
                if left.find_pane(pane_id) {
                    *ratio = (*ratio + delta).clamp(0.1, 0.9);
                    return true;
                } else if right.find_pane(pane_id) {
                    *ratio = (*ratio - delta).clamp(0.1, 0.9);
                    return true;
                }

                self.adjust_split_ratio(left, pane_id, delta)
                    || self.adjust_split_ratio(right, pane_id, delta)
            },
            SplitLayout::Vertical { top, bottom, ratio } => {
                if top.find_pane(pane_id) {
                    *ratio = (*ratio + delta).clamp(0.1, 0.9);
                    return true;
                } else if bottom.find_pane(pane_id) {
                    *ratio = (*ratio - delta).clamp(0.1, 0.9);
                    return true;
                }

                self.adjust_split_ratio(top, pane_id, delta)
                    || self.adjust_split_ratio(bottom, pane_id, delta)
            },
            _ => false,
        }
    }

    /// Get the count of panes in a layout
    pub fn pane_count(&self, layout: &SplitLayout) -> usize {
        layout.pane_count()
    }
}

impl Default for SplitManager {
    fn default() -> Self {
        Self::new()
    }
}
