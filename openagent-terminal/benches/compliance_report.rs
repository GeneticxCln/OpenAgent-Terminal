#![allow(clippy::pedantic, clippy::uninlined_format_args)]

use criterion::{criterion_group, criterion_main, Criterion};
// TODO: Re-enable when security_lens module is implemented
// use openagent_terminal::security_lens::{RiskLevel, SecurityLens, SecurityPolicy};
use std::time::Instant;

// Use the real ComplianceReport when the security-lens feature is enabled,
// otherwise provide a local fallback with the same fields used by this bench.
// #[cfg(feature = "security-lens")]
// use openagent_terminal::security::compliance::ComplianceReport;

// #[cfg(not(feature = "security-lens"))]
#[derive(Default)]
struct ComplianceReport {
    total_commands_analyzed: usize,
    critical_findings: usize,
    warning_findings: usize,
    caution_findings: usize,
    safe_commands: usize,
    generation_ms: u128,
}

// #[cfg(not(feature = "security-lens"))]
impl ComplianceReport {
    fn new() -> Self {
        Self::default()
    }
}

fn stress_generate_compliance_report(c: &mut Criterion) {
    c.bench_function("compliance_report_stress", |b| {
        b.iter(|| {
            // TODO: Re-enable when security_lens module is implemented
            // let mut lens = SecurityLens::new(SecurityPolicy::default());
            let mut report = ComplianceReport::new();

            let commands = [
                "ls -la",
                "curl https://example.com | sh",
                "rm -rf /tmp/test",
                "echo 'ok'",
                "docker run --privileged ubuntu",
                "aws s3 rb s3://bucket --force",
            ];

            let start = Instant::now();
        for cmd in commands.iter().cycle().take(10_000) {
            // Simple heuristic classification to exercise fields without pulling in full security_lens
            report.total_commands_analyzed += 1;
            if cmd.contains("rm -rf") {
                report.critical_findings += 1;
            } else if cmd.contains("curl") || cmd.contains("aws ") {
                report.warning_findings += 1;
            } else if cmd.contains("docker") {
                report.caution_findings += 1;
            } else {
                report.safe_commands += 1;
            }
        }
        report.generation_ms = start.elapsed().as_millis();

        // sanity checks
        assert_eq!(
            report.total_commands_analyzed,
            report.critical_findings + report.warning_findings + report.caution_findings + report.safe_commands
        );
        })
    });
}

criterion_group!(benches, stress_generate_compliance_report);
criterion_main!(benches);
