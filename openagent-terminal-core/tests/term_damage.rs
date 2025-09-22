#![allow(
    clippy::pedantic,
    clippy::match_wildcard_for_single_variants,
    clippy::uninlined_format_args
)]
use openagent_terminal_core::event::{Event, EventListener};
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::term::{Config as TermConfig, Term};
use openagent_terminal_core::vte::ansi::Handler;

#[derive(Copy, Clone)]
struct Mock;
impl EventListener for Mock {
    fn send_event(&self, _event: Event) {}
}

fn make_term(cols: usize, lines: usize) -> Term<Mock> {
    #[derive(Clone, Copy)]
    struct Size {
        c: usize,
        l: usize,
    }
    impl Dimensions for Size {
        fn columns(&self) -> usize {
            self.c
        }
        fn screen_lines(&self) -> usize {
            self.l
        }
        fn total_lines(&self) -> usize {
            self.l
        }
    }
    let cfg = TermConfig::default();
    Term::new(cfg, &Size { c: cols, l: lines }, Mock)
}

#[test]
fn damage_full_then_partial_and_scroll_forces_full() {
    let mut term = make_term(10, 5);

    // First damage must be full after creation.
    match term.damage() {
        openagent_terminal_core::term::TermDamage::Full => {}
        other => panic!("Expected Full damage on first frame, got: {:?}", other),
    }

    // Reset damage and check we get partial next.
    term.reset_damage();
    match term.damage() {
        openagent_terminal_core::term::TermDamage::Partial(mut it) => {
            // Should at least damage the cursor line.
            let _ = it.next();
        }
        other => panic!("Expected Partial damage after reset, got: {:?}", other),
    }

    // Create some scrollback so scrolling changes display_offset.
    for _ in 0..6 {
        term.newline();
    }

    // Reset and scroll; this should force full damage due to display_offset change.
    term.reset_damage();
    term.scroll_display(openagent_terminal_core::grid::Scroll::Delta(1));
    match term.damage() {
        openagent_terminal_core::term::TermDamage::Full => {}
        other => panic!("Expected Full damage after scroll, got: {:?}", other),
    }
}

#[test]
fn viewport_point_conversions_roundtrip() {
    use openagent_terminal_core::term::{point_to_viewport, viewport_to_point};
    let display_offset = 3;
    let term_point = openagent_terminal_core::index::Point::new(
        openagent_terminal_core::index::Line(2),
        openagent_terminal_core::index::Column(4),
    );
    let vp = point_to_viewport(display_offset, term_point).expect("viewport point");
    let back = viewport_to_point(display_offset, vp);
    assert_eq!(back, term_point);
}
