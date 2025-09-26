#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AiProvider {
    Ollama,
    OpenAI,
    Anthropic,
    OpenRouter,
}