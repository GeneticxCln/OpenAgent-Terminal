#![allow(clippy::pedantic)]

// Offscreen tab bar snapshot example (software-rendered)
// Renders simplified tab bar visuals for scenarios:
//   normal, hover, overflow, reduced
// Writes PNGs into tests/snapshot_output/tab_bar_{scenario}.png
// Prints a simple JSON so CI can parse outputs.

use image::{ImageBuffer, Rgba};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Copy)]
enum Scenario {
    Normal,
    Hover,
    Overflow,
    Reduced,
}

fn parse_scenario() -> Scenario {
    let arg = env::args()
        .find(|a| a.starts_with("--scenario="))
        .unwrap_or_else(|| "--scenario=normal".to_string());
    match arg.split('=').nth(1).unwrap_or("normal") {
        "hover" => Scenario::Hover,
        "overflow" => Scenario::Overflow,
        "reduced" => Scenario::Reduced,
        _ => Scenario::Normal,
    }
}

fn out_path_for(s: Scenario) -> PathBuf {
    let name = match s {
        Scenario::Normal => "tab_bar_normal.png",
        Scenario::Hover => "tab_bar_hover.png",
        Scenario::Overflow => "tab_bar_overflow.png",
        Scenario::Reduced => "tab_bar_reduced.png",
    };
    PathBuf::from("tests/snapshot_output").join(name)
}

#[inline]
fn fill_rect(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: u32, y: u32, w: u32, h: u32, color: [u8; 4]) {
    let (width, height) = img.dimensions();
    let x0 = x.min(width);
    let y0 = y.min(height);
    let x1 = (x + w).min(width);
    let y1 = (y + h).min(height);
    for yy in y0..y1 {
        for xx in x0..x1 {
            img.put_pixel(xx, yy, Rgba(color));
        }
    }
}

fn draw_tab_bar(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, scenario: Scenario) {
    let (w, h) = img.dimensions();
    let bar_h = 36u32; // tab bar height

    // Colors (approximate theme)
    let surface = [24, 24, 24, 255];
    let surface_muted = [36, 36, 36, 255];
    let accent = [100, 150, 250, 255];
    let text_inactive = [180, 180, 180, 255];

    // Background
    fill_rect(img, 0, 0, w, bar_h, surface);
    // Top highlight
    fill_rect(img, 0, 0, w, 2, [44, 44, 44, 255]);

    // Tabs
    let mut tabs = 5usize;
    if let Scenario::Overflow = scenario {
        tabs = 12; // overflow
    }

    let padding_x = 12u32;
    let gap = 8u32;
    let available_w = w.saturating_sub(padding_x * 2);
    let mut tab_w = if tabs > 0 {
        available_w / tabs as u32
    } else {
        available_w
    };
    tab_w = tab_w.clamp(80, 200);

    let mut x = padding_x;
    for i in 0..tabs {
        if x + tab_w > w {
            break; // clipped/overflow
        }
        // Active tab is #1
        let is_active = i == 1;
        // Hover only: highlight tab #2
        let is_hover = matches!(scenario, Scenario::Hover) && i == 2;
        let col = if is_active {
            surface_muted
        } else if is_hover {
            [52, 52, 52, 255]
        } else {
            [32, 32, 32, 255]
        };
        fill_rect(img, x, 0, tab_w, bar_h, col);
        // Active indicator
        if is_active {
            // Reduced motion: use shorter indicator to validate change
            let ind_w = if matches!(scenario, Scenario::Reduced) { tab_w / 2 } else { tab_w };
            fill_rect(img, x, bar_h - 3, ind_w, 3, accent);
        }
        // Close button square at right side
        let cb_x = x + tab_w.saturating_sub(20);
        let cb_y = bar_h / 2 - 8;
        fill_rect(img, cb_x, cb_y, 16, 16, [220, 220, 220, 200]);
        x += tab_w + gap;
    }

    // New tab "+" button at end
    let btn_size = (bar_h as f32 * 0.6) as u32;
    let btn_x = x;
    let btn_y = (bar_h - btn_size) / 2;
    fill_rect(img, btn_x, btn_y, btn_size, btn_size, [48, 48, 48, 255]);
    // Plus sign
    let line_thickness = 2u32;
    let plus_len = (btn_size as f32 * 0.5) as u32;
    let px = btn_x + (btn_size - plus_len) / 2;
    let py = btn_y + (btn_size - line_thickness) / 2;
    fill_rect(img, px, py, plus_len, line_thickness, text_inactive);
    let px2 = btn_x + (btn_size - line_thickness) / 2;
    let py2 = btn_y + (btn_size - plus_len) / 2;
    fill_rect(img, px2, py2, line_thickness, plus_len, text_inactive);
}

fn main() {
    let scenario = parse_scenario();
    let out_path = out_path_for(scenario);

    let width = 800u32;
    let height = 200u32; // include some space below the bar for stability

    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(width, height, Rgba([18, 18, 18, 255]));
    draw_tab_bar(&mut img, scenario);

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    img.save(&out_path).expect("failed to save offscreen tab bar snapshot");

    // Print JSON
    let name = out_path.file_name().unwrap().to_string_lossy();
    println!(
        "{{\"output\":\"{}\",\"width\":{},\"height\":{},\"scenario\":\"{}\"}}",
        out_path.display(),
        width,
        height,
        name
    );
}
