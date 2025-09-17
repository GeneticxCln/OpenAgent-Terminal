use openagent_terminal_core::event::{Event, EventListener};
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Direction, Point, Side};
use openagent_terminal_core::term::search::RegexSearch;
use openagent_terminal_core::term::{Config as TermConfig, Term};
use openagent_terminal_core::vte::ansi;

#[derive(Copy, Clone)]
struct Mock;
impl EventListener for Mock { fn send_event(&self, _event: Event) {} }

fn make_term_with_text(text: &str, cols: usize, lines: usize) -> Term<Mock> {
    #[derive(Clone, Copy)]
    struct Size { c: usize, l: usize }
    impl Dimensions for Size {
        fn columns(&self) -> usize { self.c }
        fn screen_lines(&self) -> usize { self.l }
        fn total_lines(&self) -> usize { self.l }
    }
    let cfg = TermConfig::default();
    let mut term = Term::new(cfg, &Size { c: cols, l: lines }, Mock);
    let mut parser: ansi::Processor = ansi::Processor::new();
    parser.advance(&mut term, text.as_bytes());
    term
}

#[test]
fn search_right_and_left_with_max_lines_and_sides() {
    // Place text on multiple lines and search with bounds and sides
    let text = "alpha beta\nBeta gamma\nALPHA BETA";
    let term = make_term_with_text(text, 20, 5);

    // Case sensitivity: upper-case in pattern -> case sensitive
    let mut re_cs = RegexSearch::new("BETA").expect("regex");
    // Case insensitive: no uppercase in pattern
    let mut re_ci = RegexSearch::new("beta").expect("regex");

    // Origin: pick top-left
    let origin = Point::new(openagent_terminal_core::index::Line(0), Column(0));

    // To the right, case-insensitive should hit first line 'beta'
    let m1 = term.search_next(&mut re_ci, origin, Direction::Right, Side::Left, Some(2)).expect("m1");
    assert_eq!(*m1.start(), Point::new(openagent_terminal_core::index::Line(0), Column(6)));

    // Case-sensitive should skip 'beta' line 0 and find 'BETA' line 1
    let m2 = term.search_next(&mut re_cs, origin, Direction::Right, Side::Left, Some(2)).expect("m2");
    assert_eq!(m2.start().line, openagent_terminal_core::index::Line(1));

    // Leftward search from below should find previous occurrence
    let origin2 = Point::new(openagent_terminal_core::index::Line(2), Column(19));
    let m3 = term.search_next(&mut re_ci, origin2, Direction::Left, Side::Right, Some(3)).expect("m3");
    // Should match the 'BETA' at end of last line depending on Side::Right
    assert_eq!(m3.end().line, openagent_terminal_core::index::Line(2));
}
