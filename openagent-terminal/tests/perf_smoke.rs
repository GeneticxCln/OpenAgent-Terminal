use std::process::Command;
use std::time::{Duration, Instant};

use openagent_terminal::config::UiConfig;
use openagent_terminal_core::event::EventListener;
use openagent_terminal_core::term::Term;
use openagent_terminal_core::vi_mode::ViMotion;

struct MockEventProxy;
impl EventListener for MockEventProxy {}

fn example_bin() -> String {
    // Cargo sets this env var for example binaries during tests.
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_render_smoke") {
        return p;
    }
    // Fallback to common path if env var is not present (shouldn't happen under `cargo test`).
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // leave openagent-terminal/
    p.push("target");
    p.push("debug");
    p.push("examples");
    p.push(if cfg!(windows) { "render_smoke.exe" } else { "render_smoke" });
    p.to_string_lossy().into_owned()
}

#[cfg(target_os = "linux")]
#[test]
fn idle_memory_usage_under_targets() {
    // Measure resident set size (RSS) from /proc/self/status and assert under target.
    // We do minimal work to approximate idle state of core components without full window init.
    use std::fs;

    // Parse VmRSS in kB
    let status = fs::read_to_string("/proc/self/status").expect("/proc/self/status");
    let mut rss_kb: u64 = 0;
    for line in status.lines() {
        if line.starts_with("VmRSS:") {
            // Format: VmRSS:     12345 kB
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                rss_kb = parts[1].parse::<u64>().unwrap_or(0);
            }
            break;
        }
    }

    assert!(rss_kb > 0, "Failed to parse VmRSS from /proc/self/status");

    // Targets (approximate):
    // - Idle core (no full GUI): < 50 MB
    // Give some headroom for CI variance.
    let rss_mb = rss_kb as f64 / 1024.0;
    assert!(rss_mb < 150.0, "Idle RSS exceeded target (<150MB): {:.1}MB", rss_mb);
}

#[test]
fn startup_time_under_100ms_core_term() {
    // Measure constructing a core Term with a tiny size; should be very fast.
    let cfg = UiConfig::default();
    let size = openagent_terminal::display::SizeInfo::new(21.0, 51.0, 3.0, 3.0, 0., 0., false);

    let start = Instant::now();
    let mut term = Term::new(cfg.term_options(), &size, MockEventProxy);
    // Exercise a small op to avoid optimizing away
    term.vi_motion(ViMotion::FirstOccupied);
    let elapsed = start.elapsed();

    // Be generous for CI variance, but keep the target tight
    assert!(
        elapsed <= Duration::from_millis(100),
        "Core Term startup exceeded 100ms: {:?}",
        elapsed
    );
}

#[test]
fn render_smoke_runs_quickly() {
    // Runs the example headless smoke renderer; ensure it returns quickly
    let bin = example_bin();
    let start = Instant::now();
    let output = Command::new(&bin)
        .arg("--backend=gl")
        .output()
        .expect("failed to run render_smoke example");
    let elapsed = start.elapsed();

    assert!(output.status.success(), "render_smoke(gl) exited with failure: {:?}", output);

    // Budget a modest runtime ceiling to catch severe regressions while avoiding flakiness.
    // On CI, account for cold cache and VM variance.
    assert!(
        elapsed <= Duration::from_millis(500),
        "render_smoke example exceeded 500ms: {:?}",
        elapsed
    );
}
