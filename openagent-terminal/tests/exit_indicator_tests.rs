use std::time::Duration;

// Test the helper in tab_bar for exit indicator alpha fade
#[test]
fn test_exit_indicator_alpha_ease_out() {
    // Base alpha
    let base = 0.85_f32;
    // At t=0, should be near 1.0
    let a0 = openagent_terminal::display::tab_bar::exit_indicator_alpha(Duration::from_millis(0), base);
    assert!(a0 <= 1.0 && a0 >= base);

    // Midway (~400ms): between base and 1.0
    let a_mid = openagent_terminal::display::tab_bar::exit_indicator_alpha(Duration::from_millis(400), base);
    assert!(a_mid > base && a_mid <= 1.0);

    // Beyond duration (>=800ms): settle at base
    let a_end = openagent_terminal::display::tab_bar::exit_indicator_alpha(Duration::from_millis(900), base);
    assert!((a_end - base).abs() < 1e-3);
}
