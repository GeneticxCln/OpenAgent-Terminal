#![allow(clippy::pedantic, clippy::uninlined_format_args)]

// Linux-only example: report idle memory (VmRSS) for KPI gating.
#[cfg(target_os = "linux")]
fn main() {
    use std::fs;

    // Parse VmRSS from /proc/self/status
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
    println!("{{\"idle_rss_kb\": {}}}", rss_kb);

    // Optional threshold via --max-mb or env KPI_MAX_IDLE_MB
    let mut max_mb: u64 = std::env::var("KPI_MAX_IDLE_MB").ok().and_then(|v| v.parse().ok()).unwrap_or(150);

    // CLI override: --max-mb <NUM> or --max-mb=<NUM>
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--max-mb" {
            if let Some(v) = args.next() { if let Ok(n) = v.parse::<u64>() { max_mb = n; } }
        } else if let Some(rest) = arg.strip_prefix("--max-mb=") {
            if let Ok(n) = rest.parse::<u64>() { max_mb = n; }
        }
    }

    let rss_mb = (rss_kb as f64) / 1024.0;
    if rss_mb as u64 > max_mb { std::process::exit(1); }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    // Not applicable on non-Linux targets (no /proc/self/status). Exit success.
}
