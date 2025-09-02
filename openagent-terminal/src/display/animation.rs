#![allow(dead_code)]
use std::time::Instant;

/// Easing function: ease-out cubic.
pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Compute animation progress in [0,1] with easing, given optional start instant,
/// duration in ms, whether opening, and whether the panel is logically active.
/// Returns eased progress (already inverted when closing).
pub fn compute_progress(
    start: Option<Instant>,
    duration_ms: u32,
    opening: bool,
    active: bool,
) -> f32 {
    // If no animation or duration is zero, return 1 for active, 0 otherwise.
    if duration_ms == 0 {
        return if active { 1.0 } else { 0.0 };
    }

    match start {
        Some(s) => {
            let elapsed_ms = s.elapsed().as_millis() as u32;
            let t = (elapsed_ms as f32 / duration_ms as f32).clamp(0.0, 1.0);
            let eased = ease_out_cubic(t);
            if opening {
                eased
            } else {
                1.0 - eased
            }
        },
        None => {
            if active {
                1.0
            } else {
                0.0
            }
        },
    }
}
