use std::path::PathBuf;

use crate::types::AiProvider;

pub fn system_prompt() -> String {
    "You are an expert terminal assistant. Provide concise, accurate, step-by-step reasoning and terminal-safe guidance. When you propose commands, briefly justify them and mention any risks. Avoid unsafe patterns (e.g., piping remote scripts into shell).".to_string()
}

pub fn select_model(provider: AiProvider) -> String {
    match provider {
        AiProvider::OpenAI => std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string()),
        AiProvider::Anthropic => std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-3-5-sonnet-latest".to_string()),
        AiProvider::Ollama => std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.1:8b-instruct".to_string()),
        AiProvider::OpenRouter => std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| "openrouter/auto".to_string()),
    }
}

pub fn error_prompt(cmd: &str, error: &str, exit_code: i32, cwd: Option<&PathBuf>) -> String {
    format!(
        "A command failed in the terminal.\nCommand: {}\nExit code: {}\nError output: {}\nWorking directory: {}\n\nPlease: \n1) Diagnose the root cause succinctly.\n2) Provide exact commands to fix it.\n3) If multiple possibilities exist, rank most likely first.\n4) Mention any prerequisites or environment setup.",
        cmd,
        exit_code,
        error,
        cwd.map(|p| p.display().to_string()).unwrap_or_else(|| "unknown".to_string())
    )
}

pub fn success_prompt(cmd: &str, exit_code: i32, stdout: &str, stderr: &str, cwd: Option<&PathBuf>) -> String {
    let out = truncate(stdout);
    let err = truncate(stderr);
    format!(
        "A command executed successfully.\nCommand: {}\nExit code: {}\nWorking directory: {}\n\nStdout (truncated):\n{}\n\nStderr (truncated):\n{}\n\nPlease provide: \n1) A brief summary of what happened.\n2) Optional next commands to continue the workflow.\n3) Any tips for best practices.",
        cmd,
        exit_code,
        cwd.map(|p| p.display().to_string()).unwrap_or_else(|| "unknown".to_string()),
        out,
        err
    )
}

fn truncate(s: &str) -> String {
    const MAX: usize = 2000;
    if s.len() <= MAX { s.to_string() } else { format!("{}...<truncated>", &s[..MAX]) }
}
