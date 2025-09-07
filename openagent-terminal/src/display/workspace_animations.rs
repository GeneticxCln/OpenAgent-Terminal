#![allow(dead_code)]
//! Animation system for workspace operations (tabs, splits, etc.)
//!
//! This module provides smooth, performant animations for all workspace interactions
//! including tab creation/deletion, drag-and-drop, split operations, and transitions.

use std::collections::HashMap;
use std::time::Instant;

use super::animation::ease_out_cubic;
use crate::workspace::TabId;

/// Duration for different animation types in milliseconds
pub const DURATION_TAB_OPEN_MS: u32 = 200;
pub const DURATION_TAB_CLOSE_MS: u32 = 150;
pub const DURATION_TAB_SWITCH_MS: u32 = 100;
pub const DURATION_DRAG_START_MS: u32 = 80;
pub const DURATION_DRAG_END_MS: u32 = 120;
pub const DURATION_HOVER_MS: u32 = 60;

/// Animation state for a single tab
#[derive(Debug, Clone)]
pub struct TabAnimationState {
    pub tab_id: TabId,
    pub animation_type: TabAnimationType,
    pub start_time: Instant,
    pub duration_ms: u32,
    pub progress: f32,
    pub is_complete: bool,
    // Animation-specific data
    pub data: TabAnimationData,
}

/// Different types of tab animations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TabAnimationType {
    Open,
    Close,
    Switch,
    DragStart,
    DragMove,
    DragEnd,
    Hover,
    Focus,
}

/// Animation-specific data
#[derive(Debug, Clone)]
pub enum TabAnimationData {
    Open { target_width: f32, current_width: f32 },
    Close { initial_width: f32, current_width: f32 },
    Switch { highlight_alpha: f32 },
    Drag { offset_x: f32, offset_y: f32, scale: f32, shadow_alpha: f32 },
    Hover { background_alpha: f32, border_alpha: f32 },
}

impl Default for TabAnimationData {
    fn default() -> Self {
        TabAnimationData::Switch { highlight_alpha: 0.0 }
    }
}

/// Animation manager for all workspace animations
pub struct WorkspaceAnimationManager {
    /// Active tab animations
    tab_animations: HashMap<TabId, TabAnimationState>,
    /// Global animation settings
    reduce_motion: bool,
    /// Performance tracking
    frame_count: u64,
    last_frame_time: Option<Instant>,
}

impl WorkspaceAnimationManager {
    /// Create new animation manager
    pub fn new() -> Self {
        Self {
            tab_animations: HashMap::new(),
            reduce_motion: false,
            frame_count: 0,
            last_frame_time: None,
        }
    }

    /// Update reduce motion setting
    pub fn set_reduce_motion(&mut self, reduce_motion: bool) {
        self.reduce_motion = reduce_motion;

        // If reduce motion is enabled, complete all animations immediately
        if reduce_motion {
            for animation in self.tab_animations.values_mut() {
                animation.is_complete = true;
                animation.progress = 1.0;
            }
        }
    }

    /// Start a new tab animation
    pub fn start_tab_animation(
        &mut self,
        tab_id: TabId,
        animation_type: TabAnimationType,
        data: Option<TabAnimationData>,
    ) {
        let duration_ms = match animation_type {
            TabAnimationType::Open => DURATION_TAB_OPEN_MS,
            TabAnimationType::Close => DURATION_TAB_CLOSE_MS,
            TabAnimationType::Switch => DURATION_TAB_SWITCH_MS,
            TabAnimationType::DragStart => DURATION_DRAG_START_MS,
            TabAnimationType::DragMove => 0, // Immediate
            TabAnimationType::DragEnd => DURATION_DRAG_END_MS,
            TabAnimationType::Hover => DURATION_HOVER_MS,
            TabAnimationType::Focus => DURATION_TAB_SWITCH_MS,
        };

        let animation_data = data.unwrap_or(match animation_type {
            TabAnimationType::Open => TabAnimationData::Open {
                target_width: 200.0, // Will be updated with actual width
                current_width: 0.0,
            },
            TabAnimationType::Close => TabAnimationData::Close {
                initial_width: 200.0, // Will be updated with actual width
                current_width: 200.0,
            },
            TabAnimationType::Switch => TabAnimationData::Switch { highlight_alpha: 0.0 },
            TabAnimationType::DragStart
            | TabAnimationType::DragMove
            | TabAnimationType::DragEnd => TabAnimationData::Drag {
                offset_x: 0.0,
                offset_y: 0.0,
                scale: 1.0,
                shadow_alpha: 0.0,
            },
            TabAnimationType::Hover => {
                TabAnimationData::Hover { background_alpha: 0.0, border_alpha: 0.0 }
            },
            TabAnimationType::Focus => TabAnimationData::Switch { highlight_alpha: 1.0 },
        });

        let animation = TabAnimationState {
            tab_id,
            animation_type,
            start_time: Instant::now(),
            duration_ms,
            progress: 0.0,
            is_complete: false,
            data: animation_data,
        };

        self.tab_animations.insert(tab_id, animation);
    }

    /// Update all animations for the current frame
    pub fn update_animations(&mut self) -> bool {
        self.frame_count += 1;
        let now = Instant::now();

        // Track frame timing for performance
        if let Some(last_frame) = self.last_frame_time {
            let _frame_duration = now.duration_since(last_frame);
            // Could be used for adaptive quality if needed
        }
        self.last_frame_time = Some(now);

        let mut animations_updated = false;
        let mut completed_animations = Vec::new();

        for (tab_id, animation) in self.tab_animations.iter_mut() {
            if animation.is_complete {
                continue;
            }

            // Calculate progress
            let elapsed_ms = animation.start_time.elapsed().as_millis() as u32;
            let raw_progress = if animation.duration_ms == 0 {
                1.0
            } else {
                (elapsed_ms as f32 / animation.duration_ms as f32).clamp(0.0, 1.0)
            };

            // Apply easing
            animation.progress = if self.reduce_motion {
                1.0 // Instant completion
            } else {
                ease_out_cubic(raw_progress)
            };

            // Update animation data based on progress
            Self::update_animation_data(animation);

            animations_updated = true;

            // Check if animation is complete
            if animation.progress >= 1.0 {
                animation.is_complete = true;
                completed_animations.push(*tab_id);
            }
        }

        // Remove completed animations (except persistent ones like hover)
        for tab_id in completed_animations {
            if let Some(animation) = self.tab_animations.get(&tab_id) {
                match animation.animation_type {
                    TabAnimationType::Hover => {}, // Keep hover animations
                    _ => {
                        self.tab_animations.remove(&tab_id);
                    },
                }
            }
        }

        animations_updated
    }

    /// Update animation-specific data based on current progress
    fn update_animation_data(animation: &mut TabAnimationState) {
        let progress = animation.progress;

        match &mut animation.data {
            TabAnimationData::Open { target_width, current_width } => {
                *current_width = *target_width * progress;
            },
            TabAnimationData::Close { initial_width, current_width } => {
                *current_width = *initial_width * (1.0 - progress);
            },
            TabAnimationData::Switch { highlight_alpha } => {
                *highlight_alpha = progress;
            },
            TabAnimationData::Drag { scale, shadow_alpha, .. } => {
                match animation.animation_type {
                    TabAnimationType::DragStart => {
                        *scale = 1.0 + progress * 0.05; // Slight scale up
                        *shadow_alpha = progress * 0.3;
                    },
                    TabAnimationType::DragEnd => {
                        *scale = 1.05 - progress * 0.05; // Scale back down
                        *shadow_alpha = 0.3 - progress * 0.3;
                    },
                    _ => {},
                }
            },
            TabAnimationData::Hover { background_alpha, border_alpha } => {
                *background_alpha = progress * 0.8;
                *border_alpha = progress;
            },
        }
    }

    /// Get current animation state for a tab
    pub fn get_tab_animation(&self, tab_id: TabId) -> Option<&TabAnimationState> {
        self.tab_animations.get(&tab_id)
    }

    /// Stop animation for a tab
    pub fn stop_tab_animation(&mut self, tab_id: TabId) -> bool {
        self.tab_animations.remove(&tab_id).is_some()
    }

    /// Get all active animations (for debugging)
    pub fn active_animations(&self) -> impl Iterator<Item = &TabAnimationState> {
        self.tab_animations.values()
    }

    /// Check if any animations are currently running
    pub fn has_active_animations(&self) -> bool {
        !self.tab_animations.is_empty()
    }

    /// Get performance metrics
    pub fn get_performance_info(&self) -> AnimationPerformanceInfo {
        AnimationPerformanceInfo {
            active_animations: self.tab_animations.len(),
            frame_count: self.frame_count,
            reduce_motion_enabled: self.reduce_motion,
        }
    }

    /// Update drag position for active drag animation
    pub fn update_drag_position(&mut self, tab_id: TabId, offset_x: f32, offset_y: f32) {
        if let Some(animation) = self.tab_animations.get_mut(&tab_id) {
            if let TabAnimationData::Drag { offset_x: ref mut ox, offset_y: ref mut oy, .. } =
                animation.data
            {
                *ox = offset_x;
                *oy = offset_y;
            }
        }
    }
}

impl Default for WorkspaceAnimationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance information for animations
#[derive(Debug)]
pub struct AnimationPerformanceInfo {
    pub active_animations: usize,
    pub frame_count: u64,
    pub reduce_motion_enabled: bool,
}

/// Helper functions for common animation calculations
pub mod animation_helpers {

    /// Calculate smooth drag offset with momentum
    pub fn calculate_drag_offset(
        start_pos: (f32, f32),
        current_pos: (f32, f32),
        momentum_factor: f32,
    ) -> (f32, f32) {
        let base_offset_x = current_pos.0 - start_pos.0;
        let base_offset_y = current_pos.1 - start_pos.1;

        // Apply momentum for smoother movement
        let offset_x = base_offset_x * momentum_factor;
        let offset_y = base_offset_y * momentum_factor;

        (offset_x, offset_y)
    }

    /// Calculate drop zone visual feedback
    pub fn calculate_drop_zone_feedback(
        mouse_x: f32,
        tab_positions: &[(f32, f32)], // (start_x, width) for each tab
        threshold: f32,
    ) -> Option<usize> {
        for (index, &(start_x, width)) in tab_positions.iter().enumerate() {
            let center_x = start_x + width / 2.0;
            if (mouse_x - center_x).abs() < threshold {
                return Some(index);
            }
        }
        None
    }

    /// Smooth step function for natural-feeling animations
    pub fn smooth_step(t: f32) -> f32 {
        t * t * (3.0 - 2.0 * t)
    }

    /// Bounce easing for playful animations
    pub fn ease_out_bounce(t: f32) -> f32 {
        if t < 1.0 / 2.75 {
            7.5625 * t * t
        } else if t < 2.0 / 2.75 {
            let t_adj = t - 1.5 / 2.75;
            7.5625 * t_adj * t_adj + 0.75
        } else if t < 2.5 / 2.75 {
            let t_adj = t - 2.25 / 2.75;
            7.5625 * t_adj * t_adj + 0.9375
        } else {
            let t_adj = t - 2.625 / 2.75;
            7.5625 * t_adj * t_adj + 0.984375
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_manager_creation() {
        let manager = WorkspaceAnimationManager::new();
        assert_eq!(manager.active_animations().count(), 0);
        assert!(!manager.has_active_animations());
    }

    #[test]
    fn test_start_tab_animation() {
        let mut manager = WorkspaceAnimationManager::new();
        let tab_id = TabId(1);

        manager.start_tab_animation(tab_id, TabAnimationType::Open, None);
        assert!(manager.has_active_animations());
        assert!(manager.get_tab_animation(tab_id).is_some());
    }

    #[test]
    fn test_reduce_motion() {
        let mut manager = WorkspaceAnimationManager::new();
        let tab_id = TabId(1);

        manager.start_tab_animation(tab_id, TabAnimationType::Open, None);
        manager.set_reduce_motion(true);

        // Animation should be completed immediately
        if let Some(animation) = manager.get_tab_animation(tab_id) {
            assert!(animation.is_complete);
        }
    }

    #[test]
    fn test_animation_helpers() {
        use animation_helpers::*;

        let offset = calculate_drag_offset((0.0, 0.0), (10.0, 5.0), 1.0);
        assert_eq!(offset, (10.0, 5.0));

        let smooth = smooth_step(0.5);
        assert!(smooth > 0.0 && smooth < 1.0);
    }
}
