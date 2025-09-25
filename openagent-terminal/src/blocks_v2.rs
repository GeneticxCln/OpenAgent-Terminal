// Minimal stubs for the legacy Blocks v2 API (feature="never").
#![allow(dead_code)]

use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BlockId(pub u64);
impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.0) }
}

#[derive(Debug, Clone, Copy)]
#[derive(Default)]
pub enum ShellType { #[default]
Bash, Zsh, Fish, PowerShell, Nushell, Custom(u32) }
impl FromStr for ShellType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "bash" => Ok(ShellType::Bash),
            "zsh" => Ok(ShellType::Zsh),
            "fish" => Ok(ShellType::Fish),
            "powershell" | "pwsh" => Ok(ShellType::PowerShell),
            "nushell" | "nu" => Ok(ShellType::Nushell),
            _ => Ok(ShellType::Custom(0)),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CreateBlockParams {
    pub command: String,
    pub directory: Option<PathBuf>,
    pub environment: Option<std::collections::HashMap<String, String>>,
    pub shell: Option<ShellType>,
    pub tags: Option<Vec<String>>,
    pub parent_id: Option<BlockId>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub command_text: Option<String>,
    pub output_text: Option<String>,
    pub sort_by: Option<&'static str>,
    pub sort_order: Option<&'static str>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub starred_only: bool,
    pub tags: Option<Vec<String>>,
    pub directory: Option<String>,
    pub shell: Option<&'static str>,
    pub status: Option<&'static str>,
    pub exit_code: Option<i32>,
    pub duration: Option<u64>,
    pub date_from: Option<&'static str>,
    pub date_to: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct BlockRecord {
    pub id: BlockId,
    pub command: String,
    pub output: String,
    pub directory: PathBuf,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub modified_at: chrono::DateTime<chrono::Utc>,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub starred: bool,
    pub tags: Vec<String>,
    pub shell: ShellType,
    pub status: String,
}

impl Default for BlockRecord {
    fn default() -> Self {
        let now = chrono::Utc::now();
        Self {
            id: BlockId(0),
            command: String::new(),
            output: String::new(),
            directory: PathBuf::new(),
            created_at: now,
            modified_at: now,
            exit_code: 0,
            duration_ms: 0,
            starred: false,
            tags: Vec::new(),
            shell: ShellType::Bash,
            status: String::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct BlockManager {
    _root: PathBuf,
}

impl ShellType {
    pub fn to_str(&self) -> &'static str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::PowerShell => "pwsh",
            ShellType::Nushell => "nu",
            ShellType::Custom(_) => "custom",
        }
    }
}

impl BlockManager {
    pub async fn new(root: PathBuf) -> anyhow::Result<Self> { Ok(Self { _root: root }) }

    pub async fn create_block(&mut self, params: CreateBlockParams) -> anyhow::Result<BlockRecord> {
        let id = BlockId(
            (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64)
                ^ (params.command.len() as u64),
        );
        Ok(BlockRecord { id, command: params.command, ..Default::default() })
    }

    pub async fn search(&self, _query: SearchQuery) -> anyhow::Result<Vec<BlockRecord>> {
        Ok(Vec::new())
    }

    // Stubs used by command_pipeline when feature="never" is enabled
    pub async fn append_output(&mut self, _block_id: BlockId, _content: &str) -> anyhow::Result<()> {
        Ok(())
    }

    pub async fn mark_block_cancelled(&mut self, _block_id: BlockId) -> anyhow::Result<()> { Ok(()) }

    pub async fn update_block_output(
        &mut self,
        _block_id: BlockId,
        _output: String,
        _exit_code: i32,
        _duration_ms: u64,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn set_workspace_pty_collection<T>(&mut self, _handle: T) {}
}
