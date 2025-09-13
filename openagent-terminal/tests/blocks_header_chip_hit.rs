// Integration-style tests for header action chip hit-testing under constrained widths
// These tests exercise the Blocks helper used by input path hit detection.

use openagent_terminal::display::blocks::Blocks;

#[test]
fn header_chip_hit_offscreen_is_ignored() {
    // With a long header, the first chip start equals or exceeds the columns width.
    // Clicks near the right edge must not register a hit.
    let header = "a".repeat(40); // width 40 => first chip starts at 42
    let columns = 42usize;
    assert_eq!(Blocks::chip_hit_at(&header, columns - 1, columns), None);
    assert_eq!(Blocks::chip_hit_at(&header, columns, columns), None);
}

#[test]
fn header_chip_hit_partial_visibility_hits_visible() {
    // Header such that first chip is partially visible in the last two columns.
    // Only clicks within the visible part should count.
    let header = "a".repeat(38); // width 38 => first chip starts at 40
    let columns = 41usize; // visible columns: [0, 41)
                           // [Copy] has width 6; only col 40 is visible from that range [40, 46)
    assert_eq!(Blocks::chip_hit_at(&header, 39, columns), None); // last header col
    assert_eq!(Blocks::chip_hit_at(&header, 40, columns), Some(0)); // visible part of [Copy]
    assert_eq!(Blocks::chip_hit_at(&header, 41, columns), None); // out of bounds
}

#[test]
fn header_chip_hit_unicode_header() {
    // Use a header with double-width characters and emoji to ensure width math works.
    let header = "▶ 你好🌟abc"; // unicode width = 2(你好) + 2(emoji approximated as 2) + 3 = ~7 plus glyphs
                                // Compute a safe columns width just before first chip starts
    let ranges = Blocks::compute_header_chip_ranges(header);
    // The first chip must start after header.width() + 2
    let first_start = ranges[0].0;
    let columns = first_start; // chips start offscreen
                               // Clicking at the last visible column shouldn't hit
    assert_eq!(Blocks::chip_hit_at(header, columns - 1, columns), None);
}
