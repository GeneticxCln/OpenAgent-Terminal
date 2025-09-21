#![allow(clippy::pedantic)]
use openagent_terminal_core::event::{Event, EventListener};
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Direction, Line, Point, Side};
use openagent_terminal_core::term::search::RegexSearch;
use openagent_terminal_core::term::{Config as TermConfig, Term};

#[derive(Clone, Default)]
struct TestListener;
impl EventListener for TestListener {
    fn send_event(&self, _event: Event) {}
}

struct TestDims {
    lines: usize,
    cols: usize,
}
impl Dimensions for TestDims {
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

fn make_term() -> Term<TestListener> {
    let cfg = TermConfig::default();
    let dims = TestDims { lines: 5, cols: 20 };
    Term::new(cfg, &dims, TestListener)
}

#[test]
fn regex_search_basic_right_and_left() {
    let mut term = make_term();

    // Write content into primary grid rows 0..4
    // Row 0: "hello world"
    let row0 = 0i32;
    for (i, ch) in "hello world".chars().enumerate() {
        term.grid_mut()[Line(row0)][Column(i)].c = ch;
    }
    // Row 1: "foo bar baz"
    let row1 = 1i32;
    for (i, ch) in "foo bar baz".chars().enumerate() {
        term.grid_mut()[Line(row1)][Column(i)].c = ch;
    }

    let mut re = RegexSearch::new("bar").expect("regex");

    // Search to the right from start of row1
    let start = Point::new(Line(row1), Column(0));
    let end = Point::new(Line(row1), term.last_column());
    let m = term.regex_search_right(&mut re, start, end).expect("match");
    let s = m.start();
    let e = m.end();
    assert_eq!(s.line, Line(row1));
    // Expect match to start at column 4 in "foo bar baz"
    assert_eq!(s.column.0, 4);
    assert!(e.column.0 >= s.column.0);

    // Now search via search_next API to the right, from origin before the match
    let origin = Point::new(Line(row1), Column(0));
    let next =
        term.search_next(&mut re, origin, Direction::Right, Side::Left, None).expect("search_next");
    assert_eq!(next.start().column.0, 4);

    // Search left from end of line, should find same token
    let origin_left = Point::new(Line(row1), Column(19));
    let prev = term
        .search_next(&mut re, origin_left, Direction::Left, Side::Left, None)
        .expect("search_left");
    assert_eq!(prev.start().column.0, 4);
}
