//! Split pane management for OpenAgent Terminal
//!
//! This module handles the creation, resizing, and navigation of split panes within tabs.

#![allow(dead_code)]

use std::collections::HashMap;

const RATIO_EPS: f32 = 0.05; // Clamp split ratios to avoid degenerate sizes

/// Unique identifier for a pane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PaneId(pub usize);

/// Layout structure for split panes
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
            }
            SplitLayout::Vertical { top, bottom, .. } => {
                top.find_pane(pane_id) || bottom.find_pane(pane_id)
            }
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
            }
            SplitLayout::Vertical { top, bottom, .. } => {
                top.collect_pane_ids_recursive(ids);
                bottom.collect_pane_ids_recursive(ids);
            }
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

/// Rectangle representing a pane's position
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

type SplitEventCallback = Box<dyn Fn(&SplitEvent) + Send + Sync>;

/// Native split manager handles pane layouts and operations without lazy fallbacks
pub struct SplitManager {
    /// Minimum pane size in cells
    minimum_pane_size: usize,

    /// Default split ratio for new splits
    default_split_ratio: f32,

    /// Native event callbacks for real-time split updates
    event_callbacks: Vec<SplitEventCallback>,

    /// Split animation states for immediate rendering
    animation_states: HashMap<PaneId, SplitAnimation>,

    /// Native pane state cache
    cached_state: SplitManagerState,

    /// Real-time split history for undo/redo
    split_history: SplitHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

/// Native split events for real-time processing
#[derive(Debug, Clone)]
pub enum SplitEvent {
    PaneCreated(PaneId),
    PaneClosed(PaneId),
    SplitCreated(PaneId, PaneId, SplitAxis),
    LayoutChanged(Vec<PaneId>),
    PaneFocused(PaneId),
    PaneResized(PaneId, f32),
    ZoomToggled(PaneId),
}

/// Split animation for native rendering
#[derive(Debug, Clone)]
pub struct SplitAnimation {
    pub animation_type: SplitAnimationType,
    pub start_time: std::time::Instant,
    pub duration: std::time::Duration,
    pub progress: f32,
    pub from_rect: Option<PaneRect>,
    pub to_rect: Option<PaneRect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitAnimationType {
    Create,
    Close,
    Resize,
    Focus,
    Split,
    Merge,
}

/// Cached split manager state for immediate access
#[derive(Debug, Clone)]
pub struct SplitManagerState {
    pub total_panes: usize,
    pub active_pane: Option<PaneId>,
    pub pane_rects: HashMap<PaneId, PaneRect>,
    pub split_ratios: HashMap<String, f32>, // path -> ratio
    pub last_update: std::time::Instant,
}

impl Default for SplitManagerState {
    fn default() -> Self {
        Self {
            total_panes: 0,
            active_pane: None,
            pane_rects: HashMap::new(),
            split_ratios: HashMap::new(),
            last_update: std::time::Instant::now(),
        }
    }
}

/// Split history for native undo/redo
#[derive(Debug, Default)]
pub struct SplitHistory {
    pub snapshots: Vec<SplitLayout>,
    pub current_index: usize,
    pub max_history: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitChild {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone)]
pub struct SplitDividerHit {
    pub axis: SplitAxis,
    /// Path from root to the split node which owns this divider
    pub path: Vec<SplitChild>,
    /// Rectangle of the container for this split node
    pub rect: PaneRect,
}

impl SplitLayout {
    fn hit_test_divider_internal(
        &self,
        rect: PaneRect,
        x: f32,
        y: f32,
        tol: f32,
        out_path: &mut Vec<SplitChild>,
    ) -> Option<SplitDividerHit> {
        match self {
            SplitLayout::Horizontal { left, right, ratio } => {
                let split_x = rect.x + rect.width * ratio;
                let within_y = y >= rect.y - tol && y <= rect.y + rect.height + tol;
                let within_x = (x - split_x).abs() <= tol;
                if within_x && within_y {
                    return Some(SplitDividerHit {
                        axis: SplitAxis::Horizontal,
                        path: out_path.clone(),
                        rect,
                    });
                }
                // Recurse
                let (left_rect, right_rect) = rect.split_horizontal(*ratio);
                out_path.push(SplitChild::Left);
                if let Some(hit) = left.hit_test_divider_internal(left_rect, x, y, tol, out_path) {
                    return Some(hit);
                }
                out_path.pop();
                out_path.push(SplitChild::Right);
                let hit = right.hit_test_divider_internal(right_rect, x, y, tol, out_path);
                out_path.pop();
                hit
            }
            SplitLayout::Vertical { top, bottom, ratio } => {
                let split_y = rect.y + rect.height * ratio;
                let within_x = x >= rect.x - tol && x <= rect.x + rect.width + tol;
                let within_y = (y - split_y).abs() <= tol;
                if within_y && within_x {
                    return Some(SplitDividerHit {
                        axis: SplitAxis::Vertical,
                        path: out_path.clone(),
                        rect,
                    });
                }
                // Recurse
                let (top_rect, bottom_rect) = rect.split_vertical(*ratio);
                out_path.push(SplitChild::Top);
                if let Some(hit) = top.hit_test_divider_internal(top_rect, x, y, tol, out_path) {
                    return Some(hit);
                }
                out_path.pop();
                out_path.push(SplitChild::Bottom);
                let hit = bottom.hit_test_divider_internal(bottom_rect, x, y, tol, out_path);
                out_path.pop();
                hit
            }
            SplitLayout::Single(_) => None,
        }
    }

    /// Hit test the layout for a divider near (x,y) within tolerance tol (px)
    pub fn hit_test_divider(
        &self,
        container: PaneRect,
        x: f32,
        y: f32,
        tol: f32,
    ) -> Option<SplitDividerHit> {
        let mut path = Vec::new();
        self.hit_test_divider_internal(container, x, y, tol, &mut path)
    }

    fn get_ratio_at_path_internal<'a>(
        &'a self,
        path: &[SplitChild],
    ) -> Option<(SplitAxis, &'a f32)> {
        if path.is_empty() {
            match self {
                SplitLayout::Horizontal { ratio, .. } => Some((SplitAxis::Horizontal, ratio)),
                SplitLayout::Vertical { ratio, .. } => Some((SplitAxis::Vertical, ratio)),
                SplitLayout::Single(_) => None,
            }
        } else {
            match (self, path[0]) {
                (SplitLayout::Horizontal { left, right: _, .. }, SplitChild::Left) => {
                    left.get_ratio_at_path_internal(&path[1..])
                }
                (SplitLayout::Horizontal { left: _, right, .. }, SplitChild::Right) => {
                    right.get_ratio_at_path_internal(&path[1..])
                }
                (SplitLayout::Vertical { top, bottom: _, .. }, SplitChild::Top) => {
                    top.get_ratio_at_path_internal(&path[1..])
                }
                (SplitLayout::Vertical { top: _, bottom, .. }, SplitChild::Bottom) => {
                    bottom.get_ratio_at_path_internal(&path[1..])
                }
                _ => None,
            }
        }
    }

    /// Public helper to fetch the ratio at a given divider path (copy of value)
    pub fn ratio_at_path(&self, path: &[SplitChild]) -> Option<(SplitAxis, f32)> {
        self.get_ratio_at_path_internal(path).map(|(ax, r)| (ax, *r))
    }

    /// Build a map of split ratios keyed by a stable path string.
    ///
    /// Keys are formatted as:
    /// - "H:" for the root horizontal split, "V:" for the root vertical split
    /// - Children append "/L", "/R", "/T", or "/B" for Left/Right/Top/Bottom traversals respectively
    ///   e.g., "H:R" means the right child of the root is a split whose ratio is recorded.
    pub fn collect_split_ratios_map(&self) -> std::collections::HashMap<String, f32> {
        fn path_key(axis: SplitAxis, path: &[SplitChild]) -> String {
            let mut s = String::new();
            s.push(match axis {
                SplitAxis::Horizontal => 'H',
                SplitAxis::Vertical => 'V',
            });
            s.push(':');
            for (i, c) in path.iter().enumerate() {
                if i > 0 {
                    s.push('/');
                }
                s.push(match c {
                    SplitChild::Left => 'L',
                    SplitChild::Right => 'R',
                    SplitChild::Top => 'T',
                    SplitChild::Bottom => 'B',
                });
            }
            s
        }
        fn rec(
            node: &SplitLayout,
            path: &mut Vec<SplitChild>,
            out: &mut std::collections::HashMap<String, f32>,
        ) {
            match node {
                SplitLayout::Horizontal { left, right, ratio } => {
                    out.insert(path_key(SplitAxis::Horizontal, path), *ratio);
                    path.push(SplitChild::Left);
                    rec(left, path, out);
                    path.pop();
                    path.push(SplitChild::Right);
                    rec(right, path, out);
                    path.pop();
                }
                SplitLayout::Vertical { top, bottom, ratio } => {
                    out.insert(path_key(SplitAxis::Vertical, path), *ratio);
                    path.push(SplitChild::Top);
                    rec(top, path, out);
                    path.pop();
                    path.push(SplitChild::Bottom);
                    rec(bottom, path, out);
                    path.pop();
                }
                SplitLayout::Single(_) => {}
            }
        }
        let mut out = std::collections::HashMap::new();
        rec(self, &mut Vec::new(), &mut out);
        out
    }

    pub fn set_ratio_at_path_internal(
        &mut self,
        path: &[SplitChild],
        axis: SplitAxis,
        new_ratio: f32,
    ) -> bool {
        if path.is_empty() {
            match self {
                SplitLayout::Horizontal { ratio, .. } if axis == SplitAxis::Horizontal => {
                    *ratio = new_ratio.clamp(0.1, 0.9);
                    true
                }
                SplitLayout::Vertical { ratio, .. } if axis == SplitAxis::Vertical => {
                    *ratio = new_ratio.clamp(0.1, 0.9);
                    true
                }
                _ => false,
            }
        } else {
            match (self, path[0]) {
                (SplitLayout::Horizontal { left, right: _, .. }, SplitChild::Left) => {
                    left.set_ratio_at_path_internal(&path[1..], axis, new_ratio)
                }
                (SplitLayout::Horizontal { left: _, right, .. }, SplitChild::Right) => {
                    right.set_ratio_at_path_internal(&path[1..], axis, new_ratio)
                }
                (SplitLayout::Vertical { top, bottom: _, .. }, SplitChild::Top) => {
                    top.set_ratio_at_path_internal(&path[1..], axis, new_ratio)
                }
                (SplitLayout::Vertical { top: _, bottom, .. }, SplitChild::Bottom) => {
                    bottom.set_ratio_at_path_internal(&path[1..], axis, new_ratio)
                }
                _ => false,
            }
        }
    }
}

impl SplitHistory {
    pub fn new(max_history: usize) -> Self {
        Self { snapshots: Vec::new(), current_index: 0, max_history }
    }

    pub fn save_snapshot(&mut self, layout: &SplitLayout) {
        // Remove any snapshots after current index
        self.snapshots.truncate(self.current_index + 1);

        // Add new snapshot
        self.snapshots.push(layout.clone());

        // Limit history size
        if self.snapshots.len() > self.max_history {
            self.snapshots.remove(0);
        } else {
            self.current_index += 1;
        }
    }

    pub fn can_undo(&self) -> bool {
        self.current_index > 0
    }

    pub fn can_redo(&self) -> bool {
        self.current_index < self.snapshots.len() - 1
    }

    pub fn undo(&mut self) -> Option<SplitLayout> {
        if self.can_undo() {
            self.current_index -= 1;
            self.snapshots.get(self.current_index).cloned()
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<SplitLayout> {
        if self.can_redo() {
            self.current_index += 1;
            self.snapshots.get(self.current_index).cloned()
        } else {
            None
        }
    }
}

impl SplitManager {
    /// Normalize the split layout by:
    /// - recursively normalizing children
    /// - clamping ratios away from extremes
    /// - rotating away adjacent splits of the same orientation into a right-leaning normal form
    /// - ensuring focus points to a valid leaf if tracked
    pub fn normalize(&self, layout: &mut SplitLayout) {
        Self::normalize_tree(layout);
    }

    fn clamp_ratio(value: f32) -> f32 {
        value.clamp(RATIO_EPS, 1.0 - RATIO_EPS)
    }

    fn normalize_tree(node: &mut SplitLayout) {
        // First, normalize children
        match node {
            SplitLayout::Single(_) => {}
            SplitLayout::Horizontal { left, right, ratio } => {
                Self::normalize_tree(left);
                Self::normalize_tree(right);

                // Attempt to rotate/merge with same-orientation children into right-leaning form
                // Case 1: Left child is also Horizontal => rotate left child up
                let mut changed = true;
                while changed {
                    changed = false;
                    // Clamp ratio each iteration
                    *ratio = Self::clamp_ratio(*ratio);

                    if let SplitLayout::Horizontal {
                        left: l_left,
                        right: l_right,
                        ratio: l_ratio,
                    } = left.as_ref()
                    {
                        // Only rotate when the right child is not a simple leaf to preserve
                        // adjacency semantics
                        let right_is_leaf = matches!(right.as_ref(), SplitLayout::Single(_));
                        if !right_is_leaf {
                            // Compute new ratios preserving widths
                            let r_p = *ratio;
                            let r_l = *l_ratio;
                            let r_p_prime = Self::clamp_ratio(r_p * r_l);
                            let denom = 1.0 - r_p * r_l;
                            if denom > 1e-6 {
                                let r2 = Self::clamp_ratio((r_p * (1.0 - r_l)) / denom);
                                // Build new structure: H( a, H(b, c) )
                                let a = l_left.as_ref().clone();
                                let b = l_right.as_ref().clone();
                                let c = right.as_ref().clone();
                                let new_right = SplitLayout::Horizontal {
                                    left: Box::new(b),
                                    right: Box::new(c),
                                    ratio: r2,
                                };
                                *left = Box::new(a);
                                *right = Box::new(new_right);
                                *ratio = r_p_prime;
                                changed = true;
                                continue;
                            }
                        }
                    }
                }
            }
            SplitLayout::Vertical { top, bottom, ratio } => {
                Self::normalize_tree(top);
                Self::normalize_tree(bottom);

                let mut changed = true;
                while changed {
                    changed = false;
                    *ratio = Self::clamp_ratio(*ratio);

                    // Case 1: Top child is also Vertical => rotate top child up
                    if let SplitLayout::Vertical { top: t_top, bottom: t_bottom, ratio: t_ratio } =
                        top.as_ref()
                    {
                        // Only rotate when the bottom child is not a simple leaf to preserve
                        // adjacency semantics
                        let bottom_is_leaf = matches!(bottom.as_ref(), SplitLayout::Single(_));
                        if !bottom_is_leaf {
                            let r_p = *ratio;
                            let r_t = *t_ratio;
                            let r_p_prime = Self::clamp_ratio(r_p * r_t);
                            let denom = 1.0 - r_p * r_t;
                            if denom > 1e-6 {
                                let r2 = Self::clamp_ratio((r_p * (1.0 - r_t)) / denom);
                                let a = t_top.as_ref().clone();
                                let b = t_bottom.as_ref().clone();
                                let c = bottom.as_ref().clone();
                                let new_bottom = SplitLayout::Vertical {
                                    top: Box::new(b),
                                    bottom: Box::new(c),
                                    ratio: r2,
                                };
                                *top = Box::new(a);
                                *bottom = Box::new(new_bottom);
                                *ratio = r_p_prime;
                                changed = true;
                                continue;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Insert an existing pane next to a target pane by creating a split at the target location.
    ///
    /// This does not create a new pane; it re-parents `moving` next to `target` inside the
    /// current layout tree. The split axis and whether the `moving` pane is placed before/after
    /// the target are controlled by `axis` and `before`.
    pub fn insert_pane_with_split(
        &self,
        layout: &mut SplitLayout,
        target: PaneId,
        moving: PaneId,
        axis: SplitAxis,
        before: bool,
    ) -> bool {
        fn insert_recursive(
            node: &mut SplitLayout,
            target: PaneId,
            moving: PaneId,
            axis: SplitAxis,
            before: bool,
            default_ratio: f32,
        ) -> bool {
            match node {
                SplitLayout::Single(id) if *id == target => {
                    // Replace leaf with a split node containing target + moving
                    let (a, b) = if before { (moving, *id) } else { (*id, moving) };
                    let new_node = match axis {
                        SplitAxis::Horizontal => SplitLayout::Horizontal {
                            left: Box::new(SplitLayout::Single(a)),
                            right: Box::new(SplitLayout::Single(b)),
                            ratio: default_ratio,
                        },
                        SplitAxis::Vertical => SplitLayout::Vertical {
                            top: Box::new(SplitLayout::Single(a)),
                            bottom: Box::new(SplitLayout::Single(b)),
                            ratio: default_ratio,
                        },
                    };
                    *node = new_node;
                    true
                }
                SplitLayout::Horizontal { left, right, .. } => {
                    insert_recursive(left, target, moving, axis, before, default_ratio)
                        || insert_recursive(right, target, moving, axis, before, default_ratio)
                }
                SplitLayout::Vertical { top, bottom, .. } => {
                    insert_recursive(top, target, moving, axis, before, default_ratio)
                        || insert_recursive(bottom, target, moving, axis, before, default_ratio)
                }
                _ => false,
            }
        }
        insert_recursive(layout, target, moving, axis, before, self.default_split_ratio)
    }

    /// Move an existing pane to a location next to `target` by re-parenting inside the tree.
    ///
    /// This will remove `pane_id` from its current position and then insert it next to `target`
    /// using a split of the specified axis and orientation.
    pub fn move_pane_to_split(
        &mut self,
        layout: &mut SplitLayout,
        pane_id: PaneId,
        target: PaneId,
        axis: SplitAxis,
        before: bool,
    ) -> bool {
        if pane_id == target {
            return false;
        }
        // First, remove the pane from the tree; if it's the last pane, abort
        let count = layout.pane_count();
        if count <= 1 {
            return false;
        }
        if !self.close_pane(layout, pane_id) {
            // Could not remove (maybe not found)
            return false;
        }
        // Then insert it next to the target
        let inserted = self.insert_pane_with_split(layout, target, pane_id, axis, before);
        if inserted {
            // Normalize after structural mutation
            self.normalize(layout);
        }
        inserted
    }

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

    /// Create a new native split manager with immediate operations
    pub fn new() -> Self {
        Self {
            minimum_pane_size: 10,
            default_split_ratio: 0.5,
            event_callbacks: Vec::new(),
            animation_states: HashMap::new(),
            cached_state: SplitManagerState::default(),
            split_history: SplitHistory::new(50), // Keep 50 undo levels
        }
    }

    /// Register native event callback for real-time updates
    pub fn register_event_callback<F>(&mut self, callback: F)
    where
        F: Fn(&SplitEvent) + Send + Sync + 'static,
    {
        self.event_callbacks.push(Box::new(callback));
    }

    /// Emit split event immediately to all registered callbacks
    fn emit_event(&self, event: SplitEvent) {
        for callback in &self.event_callbacks {
            callback(&event);
        }
    }

    /// Update cached state immediately
    fn update_cached_state(&mut self, layout: &SplitLayout, container: PaneRect) {
        let pane_rects = self.calculate_pane_rects(layout, container);
        let total_panes = layout.pane_count();

        self.cached_state = SplitManagerState {
            total_panes,
            active_pane: self.cached_state.active_pane, // Preserve active pane
            pane_rects: pane_rects.into_iter().collect(),
            split_ratios: layout.collect_split_ratios_map(),
            last_update: std::time::Instant::now(),
        };
    }

    /// Get cached state for immediate access
    pub fn get_cached_state(&self) -> &SplitManagerState {
        &self.cached_state
    }

    /// Start split animation immediately
    fn start_split_animation(&mut self, pane_id: PaneId, animation_type: SplitAnimationType) {
        let animation = SplitAnimation {
            animation_type,
            start_time: std::time::Instant::now(),
            duration: match animation_type {
                SplitAnimationType::Create => std::time::Duration::from_millis(200),
                SplitAnimationType::Close => std::time::Duration::from_millis(150),
                SplitAnimationType::Resize => std::time::Duration::from_millis(100),
                SplitAnimationType::Focus => std::time::Duration::from_millis(80),
                SplitAnimationType::Split => std::time::Duration::from_millis(250),
                SplitAnimationType::Merge => std::time::Duration::from_millis(200),
            },
            progress: 0.0,
            from_rect: None,
            to_rect: None,
        };

        self.animation_states.insert(pane_id, animation);
    }

    /// Update split animations and return panes that need rerendering
    pub fn update_animations(&mut self) -> Vec<PaneId> {
        let mut changed_panes = Vec::new();
        let now = std::time::Instant::now();

        let keys: Vec<PaneId> = self.animation_states.keys().cloned().collect();
        for pane_id in keys {
            if let Some(anim) = self.animation_states.get(&pane_id).cloned() {
                let elapsed = now.duration_since(anim.start_time);
                let progress =
                    (elapsed.as_secs_f32() / anim.duration.as_secs_f32()).clamp(0.0, 1.0);
                if progress >= 1.0 {
                    self.animation_states.remove(&pane_id);
                    changed_panes.push(pane_id);
                } else if let Some(anim_mut) = self.animation_states.get_mut(&pane_id) {
                    anim_mut.progress = progress;
                    changed_panes.push(pane_id);
                }
            }
        }

        changed_panes
    }

    /// Create a new split manager with configuration
    pub fn with_config(minimum_pane_size: usize, default_split_ratio: f32) -> Self {
        Self {
            minimum_pane_size,
            default_split_ratio,
            event_callbacks: Vec::new(),
            animation_states: HashMap::new(),
            cached_state: SplitManagerState::default(),
            split_history: SplitHistory::new(50),
        }
    }

    /// Split a pane horizontally using an explicit PaneId for the new pane
    pub fn split_horizontal_with_id(
        &mut self,
        layout: &mut SplitLayout,
        pane_id: PaneId,
        ratio: f32,
        new_pane_id: PaneId,
    ) -> bool {
        // Save snapshot for undo
        self.split_history.save_snapshot(layout);

        if self.split_pane(layout, pane_id, new_pane_id, ratio, true) {
            // Start split animation immediately
            self.start_split_animation(new_pane_id, SplitAnimationType::Create);
            self.start_split_animation(pane_id, SplitAnimationType::Split);

            // Emit immediate events
            self.emit_event(SplitEvent::PaneCreated(new_pane_id));
            self.emit_event(SplitEvent::SplitCreated(pane_id, new_pane_id, SplitAxis::Horizontal));
            self.emit_event(SplitEvent::LayoutChanged(layout.collect_pane_ids()));
            true
        } else {
            false
        }
    }

    /// Split a pane vertically using an explicit PaneId for the new pane
    pub fn split_vertical_with_id(
        &mut self,
        layout: &mut SplitLayout,
        pane_id: PaneId,
        ratio: f32,
        new_pane_id: PaneId,
    ) -> bool {
        // Save snapshot for undo
        self.split_history.save_snapshot(layout);

        if self.split_pane(layout, pane_id, new_pane_id, ratio, false) {
            // Start split animation immediately
            self.start_split_animation(new_pane_id, SplitAnimationType::Create);
            self.start_split_animation(pane_id, SplitAnimationType::Split);

            // Emit immediate events
            self.emit_event(SplitEvent::PaneCreated(new_pane_id));
            self.emit_event(SplitEvent::SplitCreated(pane_id, new_pane_id, SplitAxis::Vertical));
            self.emit_event(SplitEvent::LayoutChanged(layout.collect_pane_ids()));
            true
        } else {
            false
        }
    }

    /// Split a pane horizontally with immediate native operations
    pub fn split_horizontal(
        &mut self,
        layout: &mut SplitLayout,
        pane_id: PaneId,
        ratio: f32,
    ) -> Option<PaneId> {
        let new_pane_id = PaneId(pane_id.0 + 1000); // Legacy simple ID generation
        if self.split_horizontal_with_id(layout, pane_id, ratio, new_pane_id) {
            Some(new_pane_id)
        } else {
            None
        }
    }

    /// Split a pane vertically with immediate native operations
    pub fn split_vertical(
        &mut self,
        layout: &mut SplitLayout,
        pane_id: PaneId,
        ratio: f32,
    ) -> Option<PaneId> {
        let new_pane_id = PaneId(pane_id.0 + 1000); // Legacy simple ID generation
        if self.split_vertical_with_id(layout, pane_id, ratio, new_pane_id) {
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
            }
            SplitLayout::Horizontal { left, right, .. } => {
                self.split_pane(left, target_id, new_pane_id, ratio, horizontal)
                    || self.split_pane(right, target_id, new_pane_id, ratio, horizontal)
            }
            SplitLayout::Vertical { top, bottom, .. } => {
                self.split_pane(top, target_id, new_pane_id, ratio, horizontal)
                    || self.split_pane(bottom, target_id, new_pane_id, ratio, horizontal)
            }
            _ => false,
        }
    }

    /// Close a pane and rebalance the layout with immediate native operations
    pub fn close_pane(&mut self, layout: &mut SplitLayout, pane_id: PaneId) -> bool {
        // Save snapshot for undo
        self.split_history.save_snapshot(layout);

        // Start close animation immediately
        self.start_split_animation(pane_id, SplitAnimationType::Close);

        let success = self.remove_pane(layout, pane_id).is_some();

        if success {
            // Normalize after structural mutation
            self.normalize(layout);
            // Emit immediate events
            self.emit_event(SplitEvent::PaneClosed(pane_id));
            self.emit_event(SplitEvent::LayoutChanged(layout.collect_pane_ids()));
        }

        success
    }

    /// Remove a pane from the layout tree
    #[allow(clippy::only_used_in_recursion)]
    fn remove_pane(&self, layout: &mut SplitLayout, pane_id: PaneId) -> Option<SplitLayout> {
        match layout {
            SplitLayout::Single(id) if *id == pane_id => {
                // Can't remove the last pane
                None
            }
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
            }
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
            }
            _ => None,
        }
    }

    /// Focus the next pane in the layout with immediate native operations
    pub fn focus_next_pane(&mut self, layout: &SplitLayout, current_pane: &mut PaneId) -> bool {
        let panes = layout.collect_pane_ids();
        if let Some(current_index) = panes.iter().position(|&id| id == *current_pane) {
            let next_index = (current_index + 1) % panes.len();
            let new_pane = panes[next_index];

            // Start focus animation immediately
            self.start_split_animation(new_pane, SplitAnimationType::Focus);

            // Update cached state
            self.cached_state.active_pane = Some(new_pane);

            // Emit immediate focus event
            self.emit_event(SplitEvent::PaneFocused(new_pane));

            *current_pane = new_pane;
            true
        } else {
            false
        }
    }

    /// Focus the previous pane in the layout with immediate native operations
    pub fn focus_previous_pane(&mut self, layout: &SplitLayout, current_pane: &mut PaneId) -> bool {
        let panes = layout.collect_pane_ids();
        if let Some(current_index) = panes.iter().position(|&id| id == *current_pane) {
            let prev_index = if current_index == 0 { panes.len() - 1 } else { current_index - 1 };
            let new_pane = panes[prev_index];

            // Start focus animation immediately
            self.start_split_animation(new_pane, SplitAnimationType::Focus);

            // Update cached state
            self.cached_state.active_pane = Some(new_pane);

            // Emit immediate focus event
            self.emit_event(SplitEvent::PaneFocused(new_pane));

            *current_pane = new_pane;
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
            }
            SplitLayout::Horizontal { left, right, ratio } => {
                let (left_rect, right_rect) = rect.split_horizontal(*ratio);
                self.calculate_rects_recursive(left, left_rect, rects);
                self.calculate_rects_recursive(right, right_rect, rects);
            }
            SplitLayout::Vertical { top, bottom, ratio } => {
                let (top_rect, bottom_rect) = rect.split_vertical(*ratio);
                self.calculate_rects_recursive(top, top_rect, rects);
                self.calculate_rects_recursive(bottom, bottom_rect, rects);
            }
        }
    }

    /// Resize a split by adjusting its ratio with immediate native operations
    pub fn resize_split(&mut self, layout: &mut SplitLayout, pane_id: PaneId, delta: f32) -> bool {
        // Save snapshot for undo
        self.split_history.save_snapshot(layout);

        // Start resize animation immediately
        self.start_split_animation(pane_id, SplitAnimationType::Resize);

        let success = self.adjust_split_ratio(layout, pane_id, delta);

        if success {
            // Emit immediate resize event
            self.emit_event(SplitEvent::PaneResized(pane_id, delta));
            self.emit_event(SplitEvent::LayoutChanged(layout.collect_pane_ids()));
        }

        success
    }

    /// Drag a divider located by screen point (x,y) by a pixel delta, translating to ratio delta.
    /// Returns true if a divider was found and updated.
    pub fn drag_divider(
        &mut self,
        layout: &mut SplitLayout,
        container: PaneRect,
        x: f32,
        y: f32,
        delta_pixels: f32,
    ) -> bool {
        // Hit-test near point with a small tolerance
        let tol = 4.0;
        if let Some(hit) = layout.hit_test_divider(container, x, y, tol) {
            // Determine current ratio at path
            if let Some((axis, ratio)) = layout.ratio_at_path(&hit.path) {
                let new_ratio = match axis {
                    SplitAxis::Horizontal => {
                        Self::clamp_ratio(ratio + delta_pixels / hit.rect.width)
                    }
                    SplitAxis::Vertical => {
                        Self::clamp_ratio(ratio + delta_pixels / hit.rect.height)
                    }
                };
                let updated = layout.set_ratio_at_path_internal(&hit.path, axis, new_ratio);
                if updated {
                    self.emit_event(SplitEvent::LayoutChanged(layout.collect_pane_ids()));
                }
                return updated;
            }
        }
        false
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
            }
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
            }
            _ => false,
        }
    }

    /// Swap two adjacent leaf panes if they are siblings under a split node.
    pub fn swap_adjacent_panes(
        &self,
        layout: &mut SplitLayout,
        pane1: PaneId,
        pane2: PaneId,
    ) -> bool {
        fn rec(node: &mut SplitLayout, a: PaneId, b: PaneId) -> bool {
            match node {
                SplitLayout::Horizontal { left, right, .. } => {
                    // Check direct children first
                    if let (SplitLayout::Single(id1), SplitLayout::Single(id2)) =
                        (left.as_ref(), right.as_ref())
                    {
                        if (*id1 == a && *id2 == b) || (*id1 == b && *id2 == a) {
                            std::mem::swap(left, right);
                            return true;
                        }
                    }
                    // Recurse
                    rec(left, a, b) || rec(right, a, b)
                }
                SplitLayout::Vertical { top, bottom, .. } => {
                    if let (SplitLayout::Single(id1), SplitLayout::Single(id2)) =
                        (top.as_ref(), bottom.as_ref())
                    {
                        if (*id1 == a && *id2 == b) || (*id1 == b && *id2 == a) {
                            std::mem::swap(top, bottom);
                            return true;
                        }
                    }
                    rec(top, a, b) || rec(bottom, a, b)
                }
                SplitLayout::Single(_) => false,
            }
        }
        rec(layout, pane1, pane2)
    }

    /// Get the count of panes in a layout
    pub fn pane_count(&self, layout: &SplitLayout) -> usize {
        layout.pane_count()
    }

    /// Undo last split operation
    pub fn undo_split(&mut self, layout: &mut SplitLayout) -> bool {
        if let Some(previous_layout) = self.split_history.undo() {
            *layout = previous_layout;

            // Emit layout change event
            self.emit_event(SplitEvent::LayoutChanged(layout.collect_pane_ids()));

            true
        } else {
            false
        }
    }

    /// Redo last undone split operation
    pub fn redo_split(&mut self, layout: &mut SplitLayout) -> bool {
        if let Some(next_layout) = self.split_history.redo() {
            *layout = next_layout;

            // Emit layout change event
            self.emit_event(SplitEvent::LayoutChanged(layout.collect_pane_ids()));

            true
        } else {
            false
        }
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        self.split_history.can_undo()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        self.split_history.can_redo()
    }

    /// Toggle zoom on a pane (maximize/restore)
    pub fn toggle_zoom(&mut self, layout: &mut SplitLayout, pane_id: PaneId) -> bool {
        // Save current layout before zooming
        self.split_history.save_snapshot(layout);

        // Start zoom animation
        self.start_split_animation(pane_id, SplitAnimationType::Create);

        // Emit zoom event
        self.emit_event(SplitEvent::ZoomToggled(pane_id));

        // For now, just return true - actual zoom logic would go here
        true
    }
}

impl Default for SplitManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    // Property testing
    use proptest::prelude::*;

    fn make_complex_layout() -> SplitLayout {
        // Build a layout:
        // Vertical
        //  - Top:    Single(A)
        //  - Bottom: Horizontal( Single(B) | Single(C) )
        let a = PaneId(1);
        let b = PaneId(2);
        let c = PaneId(3);
        SplitLayout::Vertical {
            top: Box::new(SplitLayout::Single(a)),
            bottom: Box::new(SplitLayout::Horizontal {
                left: Box::new(SplitLayout::Single(b)),
                right: Box::new(SplitLayout::Single(c)),
                ratio: 0.5,
            }),
            ratio: 0.6,
        }
    }

    #[test]
    fn move_pane_into_split_reparents_correctly() {
        let mut sm = SplitManager::new();
        let mut layout = make_complex_layout();
        let a = PaneId(1);
        let c = PaneId(3);

        // Move A next to C horizontally, after C
        let ok = sm.move_pane_to_split(&mut layout, a, c, SplitAxis::Horizontal, false);
        assert!(ok, "move_pane_to_split should succeed");

        // Ensure panes are all present
        assert!(layout.find_pane(a));
        assert!(layout.find_pane(c));
        assert_eq!(layout.pane_count(), 3);

        // Normalize to clamp ratios and check structure is still valid
        sm.normalize(&mut layout);
        assert!(layout.find_pane(a));
    }

    #[test]
    fn move_pane_to_itself_is_noop() {
        let mut sm = SplitManager::new();
        let mut layout = SplitLayout::Horizontal {
            left: Box::new(SplitLayout::Single(PaneId(1))),
            right: Box::new(SplitLayout::Single(PaneId(2))),
            ratio: 0.5,
        };
        let ok =
            sm.move_pane_to_split(&mut layout, PaneId(1), PaneId(1), SplitAxis::Horizontal, true);
        assert!(!ok, "moving a pane next to itself should be a no-op and return false");
    }

    #[test]
    fn insert_pane_with_split_missing_target_fails() {
        let sm = SplitManager::new();
        let mut layout = SplitLayout::Single(PaneId(1));
        let ok = sm.insert_pane_with_split(
            &mut layout,
            PaneId(99),
            PaneId(2),
            SplitAxis::Horizontal,
            true,
        );
        assert!(!ok, "inserting next to non-existent target should fail");
    }

    #[test]
    fn close_pane_collapses_parent() {
        let mut sm = SplitManager::new();
        // Start with Horizontal split of two leaves
        let mut layout = SplitLayout::Horizontal {
            left: Box::new(SplitLayout::Single(PaneId(1))),
            right: Box::new(SplitLayout::Single(PaneId(2))),
            ratio: 0.5,
        };
        assert!(sm.close_pane(&mut layout, PaneId(1)));
        // After closing one leaf, layout should collapse to the remaining leaf
        match layout {
            SplitLayout::Single(id) => assert_eq!(id, PaneId(2)),
            _ => panic!("layout did not collapse to single pane"),
        }
    }

    #[test]
    fn move_pane_vertical_before_and_after() {
        let mut sm = SplitManager::new();
        // Build top(A) / bottom(B)
        let mut layout = SplitLayout::Vertical {
            top: Box::new(SplitLayout::Single(PaneId(1))),
            bottom: Box::new(SplitLayout::Single(PaneId(2))),
            ratio: 0.5,
        };
        // Insert C next to A vertically (before=false places below A)
        let c = PaneId(3);
        assert!(sm.insert_pane_with_split(&mut layout, PaneId(1), c, SplitAxis::Vertical, false));
        assert!(layout.find_pane(c));
        // Now move C next to B vertically before=true (above B)
        assert!(sm.move_pane_to_split(&mut layout, c, PaneId(2), SplitAxis::Vertical, true));
        assert!(layout.find_pane(c));
        // Normalize for good measure
        sm.normalize(&mut layout);
        // Ensure all three panes remain
        let ids = layout.collect_pane_ids();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn horizontal_insert_before_and_after_root_leaf() {
        let sm = SplitManager::new();
        // Start with a single target leaf B
        let b = PaneId(10);
        let mut layout = SplitLayout::Single(b);
        // Insert M before B horizontally
        let m = PaneId(11);
        assert!(sm.insert_pane_with_split(&mut layout, b, m, SplitAxis::Horizontal, true));
        // Expect Horizontal { left: M, right: B }
        match &layout {
            SplitLayout::Horizontal { left, right, .. } => match (left.as_ref(), right.as_ref()) {
                (SplitLayout::Single(lid), SplitLayout::Single(rid)) => {
                    assert_eq!((*lid, *rid), (m, b));
                }
                _ => panic!("unexpected structure after horizontal insert before"),
            },
            _ => panic!("expected horizontal split at root"),
        }
        // Now insert N after B horizontally (target B now is the right child)
        let n = PaneId(12);
        assert!(sm.insert_pane_with_split(&mut layout, b, n, SplitAxis::Horizontal, false));
        // Verify that there exists a horizontal node containing (B, N) with B on the left
        fn has_pair(node: &SplitLayout, left_id: PaneId, right_id: PaneId) -> bool {
            match node {
                SplitLayout::Horizontal { left, right, .. } => {
                    matches!((left.as_ref(), right.as_ref()), (SplitLayout::Single(l), SplitLayout::Single(r)) if *l == left_id && *r == right_id)
                        || has_pair(left, left_id, right_id)
                        || has_pair(right, left_id, right_id)
                }
                SplitLayout::Vertical { top, bottom, .. } => {
                    has_pair(top, left_id, right_id) || has_pair(bottom, left_id, right_id)
                }
                SplitLayout::Single(_) => false,
            }
        }
        assert!(has_pair(&layout, b, n), "expected (B,N) adjacency after insert-after");
    }

    #[test]
    fn horizontal_move_into_nested_target_before_after() {
        let mut sm = SplitManager::new();
        // Build nested layout: Vertical(top=A, bottom=Horizontal(left=B, right=C))
        let mut layout = SplitLayout::Vertical {
            top: Box::new(SplitLayout::Single(PaneId(1))),
            bottom: Box::new(SplitLayout::Horizontal {
                left: Box::new(SplitLayout::Single(PaneId(2))),
                right: Box::new(SplitLayout::Single(PaneId(3))),
                ratio: 0.5,
            }),
            ratio: 0.6,
        };
        // Move A before C horizontally (A should become left of C)
        assert!(sm.move_pane_to_split(
            &mut layout,
            PaneId(1),
            PaneId(3),
            SplitAxis::Horizontal,
            true
        ));
        // Verify adjacency (A,C) exists with A on the left
        fn has_pair_left_right(node: &SplitLayout, l: PaneId, r: PaneId) -> bool {
            match node {
                SplitLayout::Horizontal { left, right, .. } => {
                    matches!((left.as_ref(), right.as_ref()), (SplitLayout::Single(lid), SplitLayout::Single(rid)) if *lid == l && *rid == r)
                        || has_pair_left_right(left, l, r)
                        || has_pair_left_right(right, l, r)
                }
                SplitLayout::Vertical { top, bottom, .. } => {
                    has_pair_left_right(top, l, r) || has_pair_left_right(bottom, l, r)
                }
                SplitLayout::Single(_) => false,
            }
        }
        assert!(has_pair_left_right(&layout, PaneId(1), PaneId(3)));
        // Now move A after B horizontally (A should become right of B)
        assert!(sm.move_pane_to_split(
            &mut layout,
            PaneId(1),
            PaneId(2),
            SplitAxis::Horizontal,
            false
        ));
        assert!(has_pair_left_right(&layout, PaneId(2), PaneId(1)));
        // Keep tree valid
        sm.normalize(&mut layout);
        assert_eq!(layout.pane_count(), 3);
    }

    #[test]
    fn normalize_rotates_left_horizontal_child_preserving_widths() {
        let sm = SplitManager::new();
        let a = PaneId(1);
        let b = PaneId(2);
        let c = PaneId(3);
        // H( H(a,b; rL), c; rP )
        let r_l = 0.3f32;
        let r_p = 0.7f32;
        let mut layout = SplitLayout::Horizontal {
            left: Box::new(SplitLayout::Horizontal {
                left: Box::new(SplitLayout::Single(a)),
                right: Box::new(SplitLayout::Single(b)),
                ratio: r_l,
            }),
            right: Box::new(SplitLayout::Single(c)),
            ratio: r_p,
        };
        let container = PaneRect::new(0.0, 0.0, 1000.0, 100.0);
        let before: std::collections::HashMap<PaneId, f32> = sm
            .calculate_pane_rects(&layout, container)
            .into_iter()
            .map(|(id, rect)| (id, rect.width))
            .collect();

        sm.normalize(&mut layout);
        let after: std::collections::HashMap<PaneId, f32> = sm
            .calculate_pane_rects(&layout, container)
            .into_iter()
            .map(|(id, rect)| (id, rect.width))
            .collect();

        for id in [a, b, c] {
            assert!(approx_eq(before[&id], after[&id], 1e-3), "width mismatch for {:?}", id);
        }

        // Basic structure sanity: still a horizontal root
        assert!(matches!(layout, SplitLayout::Horizontal { .. }));
    }

    #[test]
    fn normalize_rotates_right_horizontal_child_preserving_widths() {
        let sm = SplitManager::new();
        let a = PaneId(1);
        let b = PaneId(2);
        let c = PaneId(3);
        // H( a, H(b,c; rR); rP )
        let r_r = 0.4f32;
        let r_p = 0.6f32;
        let mut layout = SplitLayout::Horizontal {
            left: Box::new(SplitLayout::Single(a)),
            right: Box::new(SplitLayout::Horizontal {
                left: Box::new(SplitLayout::Single(b)),
                right: Box::new(SplitLayout::Single(c)),
                ratio: r_r,
            }),
            ratio: r_p,
        };
        let container = PaneRect::new(0.0, 0.0, 1000.0, 100.0);
        let before: std::collections::HashMap<PaneId, f32> = sm
            .calculate_pane_rects(&layout, container)
            .into_iter()
            .map(|(id, rect)| (id, rect.width))
            .collect();

        sm.normalize(&mut layout);
        let after: std::collections::HashMap<PaneId, f32> = sm
            .calculate_pane_rects(&layout, container)
            .into_iter()
            .map(|(id, rect)| (id, rect.width))
            .collect();

        for id in [a, b, c] {
            assert!(approx_eq(before[&id], after[&id], 1e-3), "width mismatch for {:?}", id);
        }

        // Basic structure sanity: still a horizontal root
        assert!(matches!(layout, SplitLayout::Horizontal { .. }));
    }

    #[test]
    fn normalize_vertical_rotations_preserve_heights() {
        let sm = SplitManager::new();
        let a = PaneId(1);
        let b = PaneId(2);
        let c = PaneId(3);
        // V( V(a,b; rT), c; rP )
        let r_t = 0.65f32;
        let r_p = 0.55f32;
        let mut layout = SplitLayout::Vertical {
            top: Box::new(SplitLayout::Vertical {
                top: Box::new(SplitLayout::Single(a)),
                bottom: Box::new(SplitLayout::Single(b)),
                ratio: r_t,
            }),
            bottom: Box::new(SplitLayout::Single(c)),
            ratio: r_p,
        };
        let container = PaneRect::new(0.0, 0.0, 100.0, 1000.0);
        let before: std::collections::HashMap<PaneId, f32> = sm
            .calculate_pane_rects(&layout, container)
            .into_iter()
            .map(|(id, rect)| (id, rect.height))
            .collect();

        sm.normalize(&mut layout);
        let after: std::collections::HashMap<PaneId, f32> = sm
            .calculate_pane_rects(&layout, container)
            .into_iter()
            .map(|(id, rect)| (id, rect.height))
            .collect();

        for id in [a, b, c] {
            assert!(approx_eq(before[&id], after[&id], 1e-3), "height mismatch for {:?}", id);
        }

        // Basic structure sanity: still a vertical root
        assert!(matches!(layout, SplitLayout::Vertical { .. }));
    }

    #[test]
    fn move_then_insert_triggers_normalization() {
        let mut sm = SplitManager::new();
        // Start with H( H(a,b), c )
        let a = PaneId(1);
        let b = PaneId(2);
        let c = PaneId(3);
        let mut layout = SplitLayout::Horizontal {
            left: Box::new(SplitLayout::Horizontal {
                left: Box::new(SplitLayout::Single(a)),
                right: Box::new(SplitLayout::Single(b)),
                ratio: 0.5,
            }),
            right: Box::new(SplitLayout::Single(c)),
            ratio: 0.6,
        };
        // Move c next to a before, which will remove c (collapsing root), then insert near a
        assert!(sm.move_pane_to_split(&mut layout, c, a, SplitAxis::Horizontal, true));
        // After move, root should be Horizontal
        assert!(matches!(layout, SplitLayout::Horizontal { .. }));
    }

    proptest! {
            #[test]
            fn prop_normalize_preserves_rects_for_horizontal_chain(
                r_p in 0.25f32..0.85f32,
                r_c in 0.25f32..0.85f32,
            ) {
    let sm = SplitManager::new();
                let a = PaneId(1);
                let b = PaneId(2);
                let c = PaneId(3);
                // H( H(a,b; r_c), c; r_p )
                let mut layout = SplitLayout::Horizontal {
                    left: Box::new(SplitLayout::Horizontal {
                        left: Box::new(SplitLayout::Single(a)),
                        right: Box::new(SplitLayout::Single(b)),
                        ratio: r_c,
                    }),
                    right: Box::new(SplitLayout::Single(c)),
                    ratio: r_p,
                };
                let container = PaneRect::new(0.0, 0.0, 1000.0, 100.0);
                let before: std::collections::HashMap<PaneId, (f32, f32)> = sm
                    .calculate_pane_rects(&layout, container)
                    .into_iter()
                    .map(|(id, rect)| (id, (rect.x, rect.width)))
                    .collect();

                sm.normalize(&mut layout);
                let after: std::collections::HashMap<PaneId, (f32, f32)> = sm
                    .calculate_pane_rects(&layout, container)
                    .into_iter()
                    .map(|(id, rect)| (id, (rect.x, rect.width)))
                    .collect();

                for id in [a, b, c] {
                    let (bx, bw) = before[&id];
                    let (ax, aw) = after[&id];
                    assert!(approx_eq(bw, aw, 1e-3), "width mismatch for {:?}", id);
                    assert!(approx_eq(bx, ax, 1e-3), "x mismatch for {:?}", id);
                }

                // Structural sanity
                assert!(matches!(layout, SplitLayout::Horizontal { .. }));
        }
        }
}
