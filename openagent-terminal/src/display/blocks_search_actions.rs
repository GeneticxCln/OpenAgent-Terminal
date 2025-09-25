// Legacy Blocks Search actions (stubs for feature="never" builds)
#![allow(dead_code, unused_imports, unused_variables)]

/// Minimal enum of actions exposed by the Blocks Search actions menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockAction {
    // AI-assisted
    ExplainError,
    FixError,
    // Clipboard operations
    CopyCommand,
    CopyOutput,
    CopyBoth,
    // Insertions
    InsertCommand,
    InsertAsHereDoc,
    InsertAsHereDocCustom,
    InsertAsJsonHereDoc,
    InsertAsShellHereDoc,
    // Execution / management
    RerunCommand,
    ToggleStar,
    EditTags,
    ExportBlock,
    ShareBlock,
    DeleteBlock,
    ViewFullOutput,
    CreateSnippet,
}

/// Generate a simple HERE doc from output
pub fn generate_heredoc(output: &str) -> String {
    let mut s = String::new();
    s.push_str("cat <<'EOF'\n");
    s.push_str(output);
    if !output.ends_with('\n') { s.push('\n'); }
    s.push_str("EOF\n");
    s
}

/// Generate a HERE doc piped to a command
pub fn generate_heredoc_with_command(output: &str, command: &str) -> String {
    format!("cat <<'EOF' | {}\n{}\nEOF\n", command, output)
}

/// Format output as a JSON HERE doc (no validation)
pub fn format_as_json_heredoc(output: &str) -> String {
    // Best-effort: if it doesn't look like JSON, still wrap it
    generate_heredoc(output)
}

/// Format output as a HERE doc and annotate for shell type (no-op hint)
pub fn format_heredoc_for_shell(output: &str, _shell: &str) -> String {
    generate_heredoc(output)
}
