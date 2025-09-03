use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use openagent_terminal::security_lens::{SecurityLens, SecurityPolicy};

fn benchmark_security_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("security_analysis");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(200);

    // Test command scenarios with different risk levels
    let long_cmd = format!("echo {}", "A".repeat(500));
    let command_scenarios = vec![
        ("safe", "ls -la"),
        ("safe_complex", "find . -name '*.rs' -type f | head -10"),
        ("caution", "curl -s https://api.github.com/user"),
        ("warning", "sudo systemctl restart nginx"),
        ("critical", "rm -rf /tmp/*"),
        ("critical_extreme", "dd if=/dev/zero of=/dev/sda bs=1M"),
        ("obfuscated", "$(echo 'cm0gLXJm' | base64 -d) /tmp/*"),
        ("long_command", long_cmd.as_str()),
    ];

    for (risk_type, command) in command_scenarios {
        group.bench_with_input(
            BenchmarkId::new("command_analysis", risk_type),
            &command,
            |b, cmd| {
                b.iter_with_setup(
                    || SecurityLens::new(SecurityPolicy::default()),
                    |mut lens| {
                        let _risk = lens.analyze_command(cmd);
                    },
                );
            },
        );
    }

    group.finish();
}

fn benchmark_pattern_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_matching");
    group.measurement_time(Duration::from_secs(8));

    // Test bulk command analysis (simulating rapid-fire AI suggestions)
    let bulk_commands: Vec<&str> = vec![
        "git status",
        "cargo build",
        "npm install",
        "docker run -it ubuntu",
        "ssh user@server",
        "curl -X POST https://api.example.com",
        "python3 script.py",
        "sudo apt update",
        "rm file.txt",
        "cp src dest",
    ];

    group.bench_function("bulk_analysis", |b| {
        b.iter(|| {
            let mut lens = SecurityLens::new(SecurityPolicy::default());
            for cmd in &bulk_commands {
                let _risk = lens.analyze_command(cmd);
            }
        });
    });

    group.finish();
}

fn benchmark_policy_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy_creation");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("security_policy_default", |b| {
        b.iter(|| SecurityPolicy::default());
    });

    group.bench_function("security_lens_init", |b| {
        b.iter(|| SecurityLens::new(SecurityPolicy::default()));
    });

    group.finish();
}

fn benchmark_rate_limiting(c: &mut Criterion) {
    let mut group = c.benchmark_group("rate_limiting");
    group.measurement_time(Duration::from_secs(6));

    group.bench_function("rapid_analysis", |b| {
        b.iter_with_setup(
            || SecurityLens::new(SecurityPolicy::default()),
            |mut lens| {
                // Simulate rapid command analysis to test rate limiting overhead
                for i in 0..100 {
                    let cmd = format!("echo 'test command {}'", i);
                    let _risk = lens.analyze_command(&cmd);
                }
            },
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_security_analysis,
    benchmark_pattern_matching,
    benchmark_policy_creation,
    benchmark_rate_limiting
);
criterion_main!(benches);
