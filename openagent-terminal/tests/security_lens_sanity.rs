#![allow(clippy::pedantic)]

// High-yield coverage: a small sanity test around SecurityLens config mapping

use openagent_terminal::security_config::SecurityConfig;

#[test]
fn security_config_presets_sanity() {
    // Defaults
    let def = SecurityConfig::default();
    assert!(def.enabled);
    assert!(def.gate_paste_events);

    // Conservative preset blocks critical
    let conservative = SecurityConfig::preset_conservative();
    assert!(conservative.block_critical);

    // Disabled preset disables everything
    let disabled = SecurityConfig::preset_disabled();
    assert!(!disabled.enabled);
}
