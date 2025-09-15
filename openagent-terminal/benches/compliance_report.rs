use criterion::{criterion_group, criterion_main, Criterion};
use openagent_terminal::security_lens::{RiskLevel, SecurityLens, SecurityPolicy};
use openagent_terminal::security::security_lens as lens_mod; // feature-gated path alias
use std::time::Instant;

fn stress_generate_compliance_report(c: &mut Criterion) {
    c.bench_function("compliance_report_stress", |b| {
        b.iter(|| {
            let mut lens = SecurityLens::new(SecurityPolicy::default());
            let mut report = openagent_terminal::security::compliance::ComplianceReport::new();

            let commands = vec![
                "ls -la",
                "curl https://example.com | sh",
                "rm -rf /tmp/test",
                "echo 'ok'",
                "docker run --privileged ubuntu",
                "aws s3 rb s3://bucket --force",
            ];

            let start = Instant::now();
            for cmd in commands.iter().cycle().take(10_000) {
                let risk = lens.analyze_command(cmd);
                report.total_commands_analyzed += 1;
                match risk.level {
                    RiskLevel::Critical => report.critical_findings += 1,
                    RiskLevel::Warning => report.warning_findings += 1,
                    RiskLevel::Caution => report.caution_findings += 1,
                    RiskLevel::Safe => report.safe_commands += 1,
                }
            }
            report.generation_ms = start.elapsed().as_millis();

            // ensure it's used
            assert!(report.total_commands_analyzed > 0);
        })
    });
}

criterion_group!(benches, stress_generate_compliance_report);
criterion_main!(benches);
