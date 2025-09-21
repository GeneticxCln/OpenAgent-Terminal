#![allow(clippy::pedantic, clippy::cast_precision_loss)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use openagent_terminal::config::UiConfig;
use openagent_terminal::display::SizeInfo;
use openagent_terminal_core::event::EventListener;
use openagent_terminal_core::term::Term;
use openagent_terminal_core::vi_mode::ViMotion;

struct MockEventProxy;
impl EventListener for MockEventProxy {}

fn benchmark_terminal_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("terminal_creation");
    group.measurement_time(Duration::from_secs(10));

    // Test terminal creation with different sizes
    let sizes = vec![("small", 40, 12), ("standard", 80, 24), ("large", 120, 40), ("xl", 200, 60)];

    for (name, cols, rows) in sizes {
        let size = SizeInfo::new(12.0, 24.0, 3.0, 3.0, cols as f32, rows as f32, false);

        group.bench_with_input(BenchmarkId::new("term_creation", name), &size, |b, size| {
            b.iter(|| {
                let config = UiConfig::default();
                let mut term = Term::new(config.term_options(), size, MockEventProxy);
                // Exercise basic operation
                term.vi_motion(ViMotion::FirstOccupied);
            });
        });
    }

    group.finish();
}

fn benchmark_grid_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_operations");
    group.measurement_time(Duration::from_secs(6));

    let config = UiConfig::default();
    let size = SizeInfo::new(12.0, 24.0, 3.0, 3.0, 80.0, 24.0, false);

    // Test grid creation performance
    group.bench_function("grid_creation", |b| {
        b.iter(|| {
            let _term = Term::new(config.term_options(), &size, MockEventProxy);
        });
    });

    // Test VI motions
    group.bench_function("vi_motions", |b| {
        b.iter_with_setup(
            || Term::new(config.term_options(), &size, MockEventProxy),
            |mut term| {
                term.vi_motion(ViMotion::FirstOccupied);
            },
        );
    });

    group.finish();
}

fn benchmark_selection_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("selection_operations");
    group.measurement_time(Duration::from_secs(5));

    let config = UiConfig::default();
    let size = SizeInfo::new(12.0, 24.0, 3.0, 3.0, 80.0, 24.0, false);

    group.bench_function("selection_to_string", |b| {
        b.iter_with_setup(
            || Term::new(config.term_options(), &size, MockEventProxy),
            |term| {
                // Benchmark selection conversion to string
                let _selection = term.selection_to_string();
            },
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_terminal_creation,
    benchmark_grid_operations,
    benchmark_selection_ops
);
criterion_main!(benches);
