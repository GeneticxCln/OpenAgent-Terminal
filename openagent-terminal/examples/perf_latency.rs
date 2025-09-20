use std::time::Instant;

use openagent_terminal::config::UiConfig;
use openagent_terminal::display::SizeInfo;
use openagent_terminal_core::event::EventListener;
use openagent_terminal_core::term::Term;
use openagent_terminal_core::vi_mode::ViMotion;

struct MockEventProxy;
impl EventListener for MockEventProxy {}

fn parse_max_ms() -> f64 {
    // Accept "--max-ms 16.0" or "--max-ms=16.0"; default 16.0
    let mut args = std::env::args().skip(1);
    let mut max_ms: f64 = 16.0;
    while let Some(arg) = args.next() {
        if arg == "--max-ms" {
            if let Some(v) = args.next() {
                if let Ok(n) = v.parse::<f64>() {
                    max_ms = n;
                }
            }
        } else if let Some(rest) = arg.strip_prefix("--max-ms=") {
            if let Ok(n) = rest.parse::<f64>() {
                max_ms = n;
            }
        }
    }
    max_ms
}

fn main() {
    let max_ms = parse_max_ms();

    // Simulate a short render-like loop using Term operations only (no GPU/WGPU).
    let config = UiConfig::default();
    let size = SizeInfo::new(12.0, 24.0, 3.0, 3.0, 80.0, 24.0, false);
    let mut term = Term::new(config.term_options(), &size, MockEventProxy);

    // Warmup
    term.vi_motion(ViMotion::FirstOccupied);

    let iters: usize = 200;
    let mut total_ms: f64 = 0.0;

    for _ in 0..iters {
        let t0 = Instant::now();
        // Perform a tiny state touch that would be present in render paths.
        term.vi_motion(ViMotion::FirstOccupied);
        let dt = t0.elapsed();
        total_ms += dt.as_secs_f64() * 1000.0;
    }

    let avg_ms = total_ms / (iters as f64);

    // Emit a simple line parsable by CI: `avg_ms: <value>`
    println!("avg_ms: {:.3}", avg_ms);

    // Respect threshold by exiting non-zero if exceeded
    if avg_ms > max_ms {
        std::process::exit(1);
    }
}
