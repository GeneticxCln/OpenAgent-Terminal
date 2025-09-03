use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use openagent_terminal::config::UiConfig;
use openagent_terminal::display::SizeInfo;
use openagent_terminal_core::event::EventListener;
use openagent_terminal_core::term::Term;
use openagent_terminal_core::vi_mode::ViMotion;

struct MockEventProxy;
impl EventListener for MockEventProxy {}

fn benchmark_terminal_startup(c: &mut Criterion) {
    let mut group = c.benchmark_group("terminal_startup");

    // Configure measurement parameters
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);

    // Test different terminal sizes to understand scaling
    let sizes = vec![(80, 24, "standard"), (120, 40, "large"), (200, 60, "xl")];

    for (cols, rows, name) in sizes {
        let size = SizeInfo::new(
            12.0, // cell_width
            24.0, // cell_height
            3.0,  // padding_x
            3.0,  // padding_y
            cols as f32,
            rows as f32,
            false, // dynamic_title
        );

        group.bench_with_input(BenchmarkId::new("core_term_creation", name), &size, |b, size| {
            b.iter(|| {
                let config = UiConfig::default();
                let mut term = Term::new(config.term_options(), size, MockEventProxy);
                // Exercise a small operation to ensure initialization is complete
                term.vi_motion(ViMotion::FirstOccupied);
            });
        });
    }

    group.finish();
}

fn benchmark_config_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_loading");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("ui_config_default", |b| {
        b.iter(|| UiConfig::default());
    });

    group.finish();
}

fn benchmark_vi_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("vi_operations");
    group.measurement_time(Duration::from_secs(5));

    let config = UiConfig::default();
    let size = SizeInfo::new(12.0, 24.0, 3.0, 3.0, 80.0, 24.0, false);

    // Benchmark VI motion operations
    group.bench_function("vi_motion", |b| {
        b.iter_with_setup(
            || Term::new(config.term_options(), &size, MockEventProxy),
            |mut term| {
                term.vi_motion(ViMotion::FirstOccupied);
                term.vi_motion(ViMotion::FirstOccupied); // Use valid motion
            },
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_terminal_startup,
    benchmark_config_loading,
    benchmark_vi_operations
);
criterion_main!(benches);
