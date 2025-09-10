#![cfg(feature = "serde")]
use serde::Deserialize;
use serde_json as json;

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use openagent_terminal_core::event::{Event, EventListener};
use openagent_terminal_core::grid::{Dimensions, Grid};
use openagent_terminal_core::index::{Column, Line};
use openagent_terminal_core::term::cell::Cell;
use openagent_terminal_core::term::test::TermSize;
use openagent_terminal_core::term::{Config, Term};
use openagent_terminal_core::vte::ansi;

macro_rules! ref_tests {
    ($($name:ident)*) => {
        $(
            #[test]
            fn $name() {
                let test_dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ref"));
                let test_path = test_dir.join(stringify!($name));
                ref_test(&test_path);
            }
        )*
    };
}

ref_tests! {
    alt_reset
    clear_underline
    colored_reset
    colored_underline
    csi_rep
    decaln_reset
    deccolm_reset
    delete_chars_reset
    delete_lines
    erase_chars_reset
    fish_cc
    grid_reset
    history
    hyperlinks
    indexed_256_colors
    insert_blank_reset
    issue_855
    ll
    newline_with_cursor_beyond_scroll_region
    region_scroll_down
    row_reset
    saved_cursor
    saved_cursor_alt
    scroll_up_reset
    selective_erasure
    sgr
    tab_rendering
    tmux_git_log
    tmux_htop
    underline
    vim_24bitcolors_bce
    vim_large_window_scroll
    vim_simple_edit
    vttest_cursor_movement_1
    vttest_insert
    vttest_origin_mode_1
    vttest_origin_mode_2
    vttest_scroll
    vttest_tab_clear_set
    wrapline_alt_toggle
    zerowidth
    zsh_tab_completion
    erase_in_line
    scroll_in_region_up_preserves_history
    origin_goto
}

fn read_u8<P>(path: P) -> Vec<u8>
where
    P: AsRef<Path>,
{
    let mut res = Vec::new();
    File::open(path.as_ref())
        .unwrap()
        .read_to_end(&mut res)
        .unwrap();

    res
}

#[derive(Deserialize, Default)]
struct RefConfig {
    history_size: u32,
}

#[derive(Copy, Clone)]
struct Mock;

impl EventListener for Mock {
    fn send_event(&self, _event: Event) {}
}

fn ref_test(dir: &Path) {
    let recording = read_u8(dir.join("openagent-terminal.recording"));
    let serialized_size = fs::read_to_string(dir.join("size.json")).unwrap();
    let serialized_grid = fs::read_to_string(dir.join("grid.json")).unwrap();
    let serialized_cfg = fs::read_to_string(dir.join("config.json")).unwrap();

    let size: TermSize = json::from_str(&serialized_size).unwrap();
    let mut grid: Grid<Cell> = json::from_str(&serialized_grid).unwrap();
    grid.initialize_all();
    grid.truncate();
    let ref_config: RefConfig = json::from_str(&serialized_cfg).unwrap();

    let options = Config {
        scrolling_history: ref_config.history_size as usize,
        ..Default::default()
    };

    let mut terminal = Term::new(options, &size, Mock);
    let mut parser: ansi::Processor = ansi::Processor::new();

    parser.advance(&mut terminal, &recording);

    // Truncate invisible lines from the grid.
    let mut term_grid = terminal.grid().clone();
    term_grid.initialize_all();
    term_grid.truncate();

    if grid != term_grid {
        let bless = std::env::var_os("REF_BLESS").is_some() || std::env::var_os("BLESS").is_some();

        if bless {
            // Update the snapshot in-place when REF_BLESS (or BLESS) is set.
            let snapshot_path = dir.join("grid.json");
            let _ = fs::write(&snapshot_path, json::to_string_pretty(&term_grid).unwrap());
            eprintln!(
                "REF_BLESS=1 set: updated snapshot {}",
                snapshot_path.display()
            );

            // Set expected to actual so the assertion below passes in this run.
            grid = term_grid.clone();
        } else {
            for i in 0..grid.total_lines() {
                for j in 0..grid.columns() {
                    let cell = &term_grid[Line(i as i32)][Column(j)];
                    let original_cell = &grid[Line(i as i32)][Column(j)];
                    if original_cell != cell {
                        println!("[{i}][{j}] {original_cell:?} => {cell:?}",);
                    }
                }
            }

            // Dump actual and expected grids to /tmp for inspection.
            let name = dir.file_name().unwrap_or_default().to_string_lossy();
            let actual_path = format!("/tmp/term_grid_{}.json", name);
            let expected_path = format!("/tmp/expected_grid_{}.json", name);
            let _ = fs::write(&actual_path, json::to_string_pretty(&term_grid).unwrap());
            let _ = fs::write(&expected_path, json::to_string_pretty(&grid).unwrap());
            println!(
                "Dumped actual to {} and expected to {}",
                actual_path, expected_path
            );

            panic!("Ref test failed; grid doesn't match");
        }
    }

    assert_eq!(grid, term_grid);
}
