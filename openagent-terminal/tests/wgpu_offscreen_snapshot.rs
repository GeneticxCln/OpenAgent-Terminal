#![allow(clippy::pedantic, clippy::manual_let_else, clippy::uninlined_format_args)]

use std::path::Path;
use std::process::Command;

#[test]
fn wgpu_offscreen_snapshot_solid_color() {
    // Locate the example binary built by cargo
    let bin = if let Ok(p) = std::env::var("CARGO_BIN_EXE_wgpu_snapshot") {
        p
    } else {
        // Fallback: skip if not built
        eprintln!("Skipping wgpu_offscreen_snapshot_solid_color: example not built.");
        return;
    };

    // Run the example to generate the snapshot
    let output = Command::new(&bin).output().expect("failed to run wgpu_snapshot example");
    assert!(
        output.status.success(),
        "wgpu_snapshot example failed: status={:?} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let out_path = "tests/snapshot_output/wgpu_offscreen.png";
    if !Path::new(out_path).exists() {
        panic!("wgpu_snapshot did not produce output at {}", out_path);
    }

    // Load image and verify dimensions and approximate color
    let img = image::open(out_path).expect("failed to open snapshot png").to_rgba8();
    let (w, h) = img.dimensions();
    assert_eq!(w, 256);
    assert_eq!(h, 128);

    // Expected sRGB color (roughly 0.1,0.2,0.4)
    let exp = [
        (0.1f32 * 255.0f32).round() as u8,
        (0.2f32 * 255.0f32).round() as u8,
        (0.4f32 * 255.0f32).round() as u8,
        255,
    ];

    // Allow a small tolerance due to conversions
    let mut within_tolerance = 0usize;
    let total = (w * h) as usize;
    let tol = 2u8;

    for p in img.pixels() {
        let mut ok = true;
        for c in 0..3 {
            let d = p[c].abs_diff(exp[c]);
            if d > tol {
                ok = false;
                break;
            }
        }
        if ok {
            within_tolerance += 1;
        }
    }

    // Require at least 99% of pixels within tolerance
    let ratio = within_tolerance as f32 / total as f32;
    assert!(ratio >= 0.99, "color similarity below threshold: {:.4}", ratio);
}
