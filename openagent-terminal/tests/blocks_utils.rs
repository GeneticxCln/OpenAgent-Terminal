// High-yield coverage tests for Blocks v2 utilities

use openagent_terminal::blocks_v2::{BlockId, ShellType};

#[test]
fn shell_type_from_str_mappings() {
    // Common shells
    assert_eq!("bash".parse::<ShellType>().unwrap(), ShellType::Bash);
    assert_eq!("zsh".parse::<ShellType>().unwrap(), ShellType::Zsh);
    assert_eq!("fish".parse::<ShellType>().unwrap(), ShellType::Fish);
    assert_eq!("pwsh".parse::<ShellType>().unwrap(), ShellType::PowerShell);
    assert_eq!("powershell".parse::<ShellType>().unwrap(), ShellType::PowerShell);
    assert_eq!("nu".parse::<ShellType>().unwrap(), ShellType::Nushell);
    assert_eq!("nushell".parse::<ShellType>().unwrap(), ShellType::Nushell);

    // Unknown custom shell maps to Custom variant
    match "my-custom-shell".parse::<ShellType>().unwrap() {
        ShellType::Custom(_) => {}
        other => panic!("Expected Custom, got {:?}", other),
    }
}

#[test]
fn block_id_roundtrip() {
    let id = BlockId::new();
    let s = id.to_string();
    let parsed = BlockId::from_string(&s).expect("parse");
    assert_eq!(format!("{}", id), format!("{}", parsed));
}
