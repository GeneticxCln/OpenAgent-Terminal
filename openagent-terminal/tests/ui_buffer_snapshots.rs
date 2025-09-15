use openagent_terminal_core::grid::{Dimensions, Grid};
use openagent_terminal_core::index::{Column, Line, Point};
use openagent_terminal_core::term::cell::Cell;

fn grid_to_ascii(grid: &Grid<Cell>) -> String {
    let mut out = String::new();
    for i in 0..grid.screen_lines() {
        let line = Line(i as i32);
        for j in 0..grid.columns() {
            let ch = grid[line][Column(j)].c;
            out.push(if ch.is_control() { ' ' } else { ch });
        }
        // trim trailing spaces for stable snapshot
        while out.ends_with(' ') {
            out.pop();
        }
        out.push('\n');
    }
    out
}

#[test]
fn ui_buffer_basic_snapshot() {
    // Build a tiny grid and write a few characters
    let mut grid: Grid<Cell> = Grid::new(5, 10, 100);
    // Place some text on first line
    grid[Line(0)][Column(0)].c = 'H';
    grid[Line(0)][Column(1)].c = 'e';
    grid[Line(0)][Column(2)].c = 'l';
    grid[Line(0)][Column(3)].c = 'l';
    grid[Line(0)][Column(4)].c = 'o';
    // And second line
    grid[Line(1)][Column(0)].c = 'T';
    grid[Line(1)][Column(1)].c = 'U';
    grid[Line(1)][Column(2)].c = 'I';

    let ascii = grid_to_ascii(&grid);
    insta::assert_snapshot!(ascii);
}

#[test]
fn ui_buffer_wrapping_snapshot() {
    // Ensure wrapped content stabilizes
    let mut grid: Grid<Cell> = Grid::new(3, 6, 100);
    let text = "The quick brown fox";
    let mut col = 0;
    let mut row = 0;
    for ch in text.chars() {
        grid[Line(row)][Column(col)].c = ch;
        col += 1;
        if col >= grid.columns() {
            col = 0;
            row += 1;
            if row >= grid.screen_lines() { break; }
        }
    }

    let ascii = grid_to_ascii(&grid);
    insta::assert_snapshot!(ascii);
}
