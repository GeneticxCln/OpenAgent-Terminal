use std::time::Instant;

use openagent_terminal::config::UiConfig;
use openagent_terminal::display::SizeInfo;
use openagent_terminal_core::event::EventListener;
use openagent_terminal_core::term::Term;
use openagent_terminal_core::vi_mode::ViMotion;

struct MockEventProxy;
impl EventListener for MockEventProxy {}

fn parse_max_ms() -> u64 {
    // Accept "--max-ms 800" or "--max-ms=800"; default 800
    let mut args = std::env::args().skip(1);
    let mut max_ms: u64 = 800;
    while let Some(arg) = args.next() {
        if arg == "--max-ms" {
            if let Some(v) = args.next() {
                if let Ok(n) = v.parse::<u64>() { max_ms = n; }
            }
        } else if let Some(rest) = arg.strip_prefix("--max-ms=") {
            if let Ok(n) = rest.parse::<u64>() { max_ms = n; }
        }
    }
    max_ms
}

fn main() {
    let max_ms = parse_max_ms();

    // Measure a representative cold-start path without requiring a GPU context.
    // We time minimal config + Term initialization work only.
    let t0 = Instant::now();

    let config = UiConfig::default();
    // A small, fixed size to keep benchmark stable across CI machines.
    let size = SizeInfo::new(12.0, 24.0, 3.0, 3.0, 80.0, 24.0, false);
    let mut term = Term::new(config.term_options(), &size, MockEventProxy);
    // Exercise a tiny operation to ensure initialization completed paths are touched.
    term.vi_motion(ViMotion::FirstOccupied);

    let elapsed_ms = t0.elapsed().as_millis() as u64;

    // Emit JSON suitable for jq parsing in CI.
    println!("{{\"cold_start_ms\": {}}}", elapsed_ms);

    // Respect threshold by exiting non-zero if exceeded (CI uses set -e)
    if elapsed_ms > max_ms {
        std::process::exit(1);
    }
}
