// Command Notebooks core module
// Provides: Notebook types, storage, and manager with command execution

pub mod storage;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::sync::RwLock;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::blocks_v2::{BlockId, BlockManager, CreateBlockParams, ExecutionStatus, ShellType};

/// Notebook identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotebookId(Uuid);

impl NotebookId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_string(s: &str) -> Result<Self> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for NotebookId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::str::FromStr for NotebookId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl std::fmt::Display for NotebookId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Notebook cell identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellId(Uuid);

impl CellId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_string(s: &str) -> Result<Self> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for CellId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::str::FromStr for CellId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl std::fmt::Display for CellId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Cell type for notebooks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellType {
    Command,
    Markdown,
}

/// Notebook model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notebook {
    pub id: NotebookId,
    pub name: String,
    pub description: Option<String>,
    pub tags: HashSet<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Default working directory for this notebook
    pub default_directory: Option<PathBuf>,
    /// Environment variable overrides applied to all command cells
    pub env_overrides: std::collections::HashMap<String, String>,
    /// Declared parameters for template substitution ({{name}}) in command cells
    pub params: Vec<NotebookParam>,
}

/// Notebook cell model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookCell {
    pub id: CellId,
    pub notebook_id: NotebookId,
    pub idx: i64,
    pub cell_type: CellType,
    pub content: String, // command string or markdown text
    pub directory: Option<PathBuf>,
    pub shell: Option<ShellType>,
    pub output: Option<String>,
    pub error_output: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub block_id: Option<BlockId>,
    pub status: ExecutionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Manager for notebooks
pub struct NotebookManager {
    storage: storage::NotebookStorage,
    block_manager: Option<Arc<RwLock<BlockManager>>>,
}

/// Notebook parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookParam {
    pub name: String,
    pub description: Option<String>,
    pub default: Option<String>,
}

impl NotebookManager {
    /// Create a new manager. Will create the notebooks database in `data_dir`.
    pub async fn new(
        data_dir: impl AsRef<Path>,
        block_manager: Option<Arc<RwLock<BlockManager>>>,
    ) -> Result<Self> {
        let storage = storage::NotebookStorage::new(data_dir.as_ref()).await?;
        Ok(Self { storage, block_manager })
    }

    pub async fn create_notebook(
        &self,
        name: String,
        description: Option<String>,
        tags: HashSet<String>,
    ) -> Result<Notebook> {
        let nb = Notebook {
            id: NotebookId::new(),
            name,
            description,
            tags,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            default_directory: None,
            env_overrides: std::collections::HashMap::new(),
            params: Vec::new(),
        };
        self.storage.insert_notebook(&nb).await?;
        Ok(nb)
    }

    pub async fn list_notebooks(&self) -> Result<Vec<Notebook>> {
        self.storage.list_notebooks().await
    }

    pub async fn get_notebook(&self, id: NotebookId) -> Result<Notebook> {
        self.storage.get_notebook(id).await
    }

    pub async fn add_markdown_cell(
        &self,
        notebook_id: NotebookId,
        idx: Option<i64>,
        text: String,
    ) -> Result<NotebookCell> {
        let order = match idx {
            Some(i) => i,
            None => self.storage.next_index_for_notebook(notebook_id).await?,
        };
        let cell = NotebookCell {
            id: CellId::new(),
            notebook_id,
            idx: order,
            cell_type: CellType::Markdown,
            content: text,
            directory: None,
            shell: None,
            output: None,
            error_output: None,
            exit_code: None,
            duration_ms: None,
            block_id: None,
            status: ExecutionStatus::Success, // non-executable
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.storage.insert_cell(&cell).await?;
        Ok(cell)
    }

    pub async fn add_command_cell(
        &self,
        notebook_id: NotebookId,
        idx: Option<i64>,
        command: String,
        directory: Option<PathBuf>,
        shell: Option<ShellType>,
    ) -> Result<NotebookCell> {
        let order = match idx {
            Some(i) => i,
            None => self.storage.next_index_for_notebook(notebook_id).await?,
        };
        let cell = NotebookCell {
            id: CellId::new(),
            notebook_id,
            idx: order,
            cell_type: CellType::Command,
            content: command,
            directory,
            shell,
            output: None,
            error_output: None,
            exit_code: None,
            duration_ms: None,
            block_id: None,
            status: ExecutionStatus::Running,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.storage.insert_cell(&cell).await?;
        Ok(cell)
    }

    pub async fn list_cells(&self, notebook_id: NotebookId) -> Result<Vec<NotebookCell>> {
        self.storage.list_cells(notebook_id).await
    }

    pub async fn run_cell(&self, cell_id: CellId) -> Result<NotebookCell> {
        let mut cell = self.storage.get_cell(cell_id).await?;
        if cell.cell_type != CellType::Command {
            return Err(anyhow!("Cell is not a command cell"));
        }

        // Resolve working directory: cell.dir > notebook default > cwd
        let mut cwd = cell
            .directory
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        // Try notebook default
        if let Ok(nb) = self.storage.get_notebook(cell.notebook_id).await {
            if let Some(dir) = nb.default_directory.as_ref() {
                if cell.directory.is_none() {
                    cwd = dir.clone();
                }
            }
        }

        let shell = cell.shell.unwrap_or(ShellType::Bash);
        let (program, arg_flag) = map_shell_to_program(shell);
        let mut cmdline = cell.content.clone();
        // Substitute notebook-level parameters {{name}} with defaults
        if let Ok(nb) = self.storage.get_notebook(cell.notebook_id).await {
            if !nb.params.is_empty() {
                let param_map: std::collections::HashMap<String, String> = nb
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), p.default.clone().unwrap_or_default()))
                    .collect();
                cmdline = substitute_params(&cmdline, &param_map);
            }
        }

        // Create a Block first if we have a block manager
        let mut maybe_block_id: Option<BlockId> = None;
        if let Some(bm) = &self.block_manager {
            let mut bm = bm.write().await;
            let params = CreateBlockParams {
                command: cmdline.clone(),
                directory: Some(cwd.clone()),
                environment: Some(std::env::vars().collect()),
                shell: Some(shell),
                tags: None,
                parent_id: None,
                metadata: None,
            };
            let block = bm.create_block(params).await?;
            maybe_block_id = Some(block.id);
            debug!("Notebook cell {} created block {}", cell.id, block.id);
        }

        // Execute the command
        let started = std::time::Instant::now();
        // Merge env overrides
        let mut envs: std::collections::HashMap<String, String> = std::env::vars().collect();
        if let Ok(nb) = self.storage.get_notebook(cell.notebook_id).await {
            for (k, v) in nb.env_overrides {
                envs.insert(k, v);
            }
        }
        let output = tokio::process::Command::new(program)
            .arg(arg_flag)
            .arg(&cmdline)
            .current_dir(&cwd)
            .envs(&envs)
            .output()
            .await
            .with_context(|| format!("Failed to execute command: {}", &cmdline))?;

        let duration = started.elapsed();
        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Update cell
        cell.output = Some(stdout.clone());
        cell.error_output = Some(stderr.clone());
        cell.exit_code = Some(exit_code);
        cell.duration_ms = Some(duration.as_millis() as u64);
        cell.status =
            if exit_code == 0 { ExecutionStatus::Success } else { ExecutionStatus::Failed };
        cell.block_id = maybe_block_id;
        cell.updated_at = Utc::now();
        self.storage.update_cell(&cell).await?;

        // Update block if present
        if let (Some(bid), Some(bm)) = (maybe_block_id, &self.block_manager) {
            let mut bm = bm.write().await;
            bm.update_block_output(bid, stdout.clone(), exit_code, duration.as_millis() as u64)
                .await?;
        }

        Ok(cell)
    }

    pub async fn run_notebook(&self, notebook_id: NotebookId) -> Result<Vec<NotebookCell>> {
        let mut out = Vec::new();
        let mut cells = self.storage.list_cells(notebook_id).await?;
        // Sort by idx
        cells.sort_by_key(|c| c.idx);
        for c in cells {
            if c.cell_type == CellType::Command {
                let executed = self.run_cell(c.id).await?;
                out.push(executed);
            } else {
                out.push(c);
            }
        }
        Ok(out)
    }
}

fn map_shell_to_program(shell: ShellType) -> (&'static str, &'static str) {
    match shell {
        ShellType::Bash => ("bash", "-c"),
        ShellType::Zsh => ("zsh", "-c"),
        ShellType::Fish => ("fish", "-c"),
        ShellType::PowerShell => ("pwsh", "-c"),
        ShellType::Nushell => ("nu", "-c"),
        ShellType::Custom(_) => ("sh", "-c"),
    }
}

// ===== CLI integration =====
use clap::{Args, Subcommand};

/// CLI options for `openagent-terminal notebook ...`
#[derive(Args, Debug, Clone)]
pub struct NotebookOptions {
    #[clap(subcommand)]
    pub cmd: NotebookCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum NotebookCommand {
    /// Create a new notebook
    Create {
        name: String,
        #[clap(long)]
        description: Option<String>,
        #[clap(long)]
        tag: Vec<String>,
    },
    /// List notebooks
    List {},
    /// Show notebook cells
    Show {
        /// Notebook ID or exact name
        notebook: String,
    },
    /// Add a command cell
    AddCommand {
        notebook: String,
        #[clap(long)]
        idx: Option<i64>,
        command: String,
        #[clap(long)]
        shell: Option<String>,
        #[clap(long, value_hint = clap::ValueHint::DirPath)]
        directory: Option<PathBuf>,
    },
    /// Add a markdown cell
    AddMarkdown {
        notebook: String,
        #[clap(long)]
        idx: Option<i64>,
        #[clap(long)]
        text: Option<String>,
        #[clap(long, value_hint = clap::ValueHint::FilePath)]
        file: Option<PathBuf>,
    },
    /// Run a single command cell by index (0-based) or all if omitted
    Run {
        notebook: String,
        #[clap(long)]
        cell: Option<i64>,
        #[clap(long = "param", value_parser = parse_kv, num_args = 0..)]
        params: Vec<(String, String)>,
    },
    /// Export a notebook to JSON or Markdown
    Export {
        notebook: String,
        #[clap(long, value_enum)]
        format: ExportFmt,
        #[clap(long, value_hint = clap::ValueHint::FilePath)]
        out: Option<PathBuf>,
    },
    /// Import a notebook from a JSON file
    Import {
        #[clap(long, value_hint = clap::ValueHint::FilePath)]
        file: PathBuf,
        #[clap(long)]
        generate_new_ids: bool,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ExportFmt {
    Json,
    Md,
}

/// Entry point to execute notebook-related CLI commands
pub async fn run_cli(opts: &NotebookOptions) -> Result<i32> {
    // Compute data directories similar to components_init
    let base_data_dir =
        dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("openagent-terminal");
    let blocks_dir = base_data_dir.join("blocks");
    let notebooks_dir = base_data_dir.join("notebooks");

    // Initialize block manager (if possible)
    let block_manager = match BlockManager::new(blocks_dir.clone()).await {
        Ok(bm) => Some(Arc::new(RwLock::new(bm))),
        Err(e) => {
            warn!("Blocks not available: {} — continuing without block linkage", e);
            None
        }
    };

    let mgr = NotebookManager::new(&notebooks_dir, block_manager).await?;

    match &opts.cmd {
        NotebookCommand::Create { name, description, tag } => {
            let tags: HashSet<String> = tag.iter().cloned().collect();
            let nb = mgr.create_notebook(name.clone(), description.clone(), tags).await?;
            println!("{}", nb.id);
            Ok(0)
        }
        NotebookCommand::List {} => {
            let list = mgr.list_notebooks().await?;
            for nb in list {
                println!("{}\t{}", nb.id, nb.name);
            }
            Ok(0)
        }
        NotebookCommand::Show { notebook } => {
            let nb = resolve_notebook(&mgr, notebook).await?;
            println!("{}\t{}", nb.id, nb.name);
            let mut cells = mgr.list_cells(nb.id).await?;
            cells.sort_by_key(|c| c.idx);
            for c in cells {
                match c.cell_type {
                    CellType::Markdown => {
                        println!("#{} [md]\n{}", c.idx, c.content);
                    }
                    CellType::Command => {
                        println!("#{} [cmd] {}", c.idx, c.content);
                        if let Some(code) = c.exit_code {
                            println!("-> exit={} time={}ms", code, c.duration_ms.unwrap_or(0));
                        }
                    }
                }
            }
            Ok(0)
        }
        NotebookCommand::AddCommand { notebook, idx, command, shell, directory } => {
            let nb = resolve_notebook(&mgr, notebook).await?;
            let shell_t = shell.as_ref().and_then(|s| s.parse::<ShellType>().ok());
            let cell = mgr
                .add_command_cell(nb.id, *idx, command.clone(), directory.clone(), shell_t)
                .await?;
            println!("{}", cell.id);
            Ok(0)
        }
        NotebookCommand::AddMarkdown { notebook, idx, text, file } => {
            let nb = resolve_notebook(&mgr, notebook).await?;
            let content = if let Some(path) = file {
                let mut f = tokio::fs::File::open(path)
                    .await
                    .with_context(|| format!("Failed reading {:?}", path))?;
                let mut buf = String::new();
                f.read_to_string(&mut buf).await?;
                buf
            } else if let Some(t) = text.clone() {
                t
            } else {
                return Err(anyhow!("Provide either --text or --file"));
            };
            let cell = mgr.add_markdown_cell(nb.id, *idx, content).await?;
            println!("{}", cell.id);
            Ok(0)
        }
        NotebookCommand::Run { notebook, cell, params: _ } => {
            let nb = resolve_notebook(&mgr, notebook).await?;
            if let Some(i) = cell {
                // Run a single cell by index
                let cells = mgr.list_cells(nb.id).await?;
                let target = cells
                    .into_iter()
                    .find(|c| c.idx == *i)
                    .ok_or_else(|| anyhow!("No cell with index {}", i))?;
                let updated = mgr.run_cell(target.id).await?;
                // Print outputs
                if let Some(out) = updated.output {
                    print!("{}", out);
                }
                if let Some(err) = updated.error_output {
                    eprint!("{}", err);
                }
                Ok(updated.exit_code.unwrap_or(0))
            } else {
                // Run entire notebook (current notebook-level defaults are used)
                let results = mgr.run_notebook(nb.id).await?;
                // Print outputs in order
                for c in &results {
                    if c.cell_type == CellType::Command {
                        if let Some(out) = &c.output {
                            print!("{}", out);
                        }
                        if let Some(err) = &c.error_output {
                            eprint!("{}", err);
                        }
                    }
                }
                // Return last exit code or 0
                let code = results
                    .iter()
                    .rev()
                    .find_map(|c| if c.cell_type == CellType::Command { c.exit_code } else { None })
                    .unwrap_or(0);
                Ok(code)
            }
        }
        NotebookCommand::Export { notebook, format, out } => {
            let nb = resolve_notebook(&mgr, notebook).await?;
            let data = match format {
                ExportFmt::Json => export_notebook_json(&mgr, nb.id).await?,
                ExportFmt::Md => export_notebook_markdown(&mgr, nb.id).await?,
            };
            if let Some(path) = out {
                tokio::fs::write(path, &data).await?;
            } else {
                let mut stdout = tokio::io::stdout();
                tokio::io::AsyncWriteExt::write_all(&mut stdout, &data).await?;
            }
            Ok(0)
        }
        NotebookCommand::Import { file, generate_new_ids } => {
            let bytes = tokio::fs::read(file).await?;
            let ids = import_notebook_json(&mgr, &bytes, *generate_new_ids).await?;
            for id in ids {
                println!("{}", id);
            }
            Ok(0)
        }
    }
}

async fn resolve_notebook(mgr: &NotebookManager, arg: &str) -> Result<Notebook> {
    // Try ID first
    if let Ok(id) = arg.parse::<NotebookId>() {
        if let Ok(nb) = mgr.get_notebook(id).await {
            return Ok(nb);
        }
    }
    // Fallback to exact name match
    let list = mgr.list_notebooks().await?;
    list.into_iter().find(|n| n.name == arg).ok_or_else(|| anyhow!("Notebook not found: {}", arg))
}

fn substitute_params(cmd: &str, params: &std::collections::HashMap<String, String>) -> String {
    let mut out = String::with_capacity(cmd.len());
    let mut i = 0;
    let b = cmd.as_bytes();
    while i < b.len() {
        if i + 3 < b.len() && b[i] == b'{' && b[i + 1] == b'{' {
            // find closing }}
            let mut j = i + 2;
            while j + 1 < b.len() && !(b[j] == b'}' && b[j + 1] == b'}') {
                j += 1;
            }
            if j + 1 < b.len() {
                let key = &cmd[i + 2..j].trim();
                if let Some(val) = params.get(&key.to_string()) {
                    out.push_str(val);
                } else {
                    out.push_str("");
                }
                i = j + 2;
                continue;
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

fn parse_kv(s: &str) -> Result<(String, String), String> {
    let (k, v) = s.split_once('=').ok_or_else(|| "expected key=value".to_string())?;
    Ok((k.to_string(), v.to_string()))
}

async fn export_notebook_json(mgr: &NotebookManager, id: NotebookId) -> Result<Vec<u8>> {
    let nb = mgr.storage.get_notebook(id).await?;
    let cells = mgr.storage.list_cells(id).await?;
    let export = serde_json::json!({
        "notebook": nb,
        "cells": cells,
    });
    Ok(serde_json::to_vec_pretty(&export)?)
}

pub(crate) async fn export_notebook_markdown(
    mgr: &NotebookManager,
    id: NotebookId,
) -> Result<Vec<u8>> {
    let nb = mgr.storage.get_notebook(id).await?;
    let mut s = String::new();
    s.push_str(&format!("# {}\n\n", nb.name));
    if let Some(desc) = &nb.description {
        s.push_str(desc);
        s.push_str("\n\n");
    }
    let mut cells = mgr.storage.list_cells(id).await?;
    cells.sort_by_key(|c| c.idx);
    for c in cells {
        match c.cell_type {
            CellType::Markdown => {
                s.push_str(&c.content);
                s.push_str("\n\n");
            }
            CellType::Command => {
                s.push_str("```sh\n");
                s.push_str(&c.content);
                s.push_str("\n```\n\n");
                if let Some(out) = &c.output {
                    s.push_str("<details><summary>output</summary>\n\n");
                    s.push_str("```\n");
                    s.push_str(out);
                    s.push_str("\n```\n\n</details>\n\n");
                }
            }
        }
    }
    Ok(s.into_bytes())
}

async fn import_notebook_json(
    mgr: &NotebookManager,
    data: &[u8],
    generate_new_ids: bool,
) -> Result<Vec<String>> {
    let v: serde_json::Value = serde_json::from_slice(data)?;
    let nb: Notebook = serde_json::from_value(
        v.get("notebook").cloned().ok_or_else(|| anyhow!("missing notebook"))?,
    )?;
    let mut cells: Vec<NotebookCell> =
        serde_json::from_value(v.get("cells").cloned().ok_or_else(|| anyhow!("missing cells"))?)?;
    let mut nb_new = nb.clone();
    if generate_new_ids {
        nb_new.id = NotebookId::new();
    }
    mgr.storage.insert_notebook(&nb_new).await?;
    let mut out_ids = vec![nb_new.id.to_string()];
    for mut c in cells.drain(..) {
        c.notebook_id = nb_new.id;
        if generate_new_ids {
            c.id = CellId::new();
        }
        mgr.storage.insert_cell(&c).await?;
        out_ids.push(c.id.to_string());
    }
    Ok(out_ids)
}

impl NotebookManager {
    pub async fn export_notebook_markdown_bytes(&self, id: NotebookId) -> Result<Vec<u8>> {
        export_notebook_markdown(self, id).await
    }

    pub async fn delete_cell(&self, cell_id: CellId) -> Result<()> {
        self.storage.delete_cell(cell_id).await
    }

    pub async fn update_cell_content(
        &self,
        cell_id: CellId,
        new_content: String,
    ) -> Result<NotebookCell> {
        let mut cell = self.storage.get_cell(cell_id).await?;
        cell.content = new_content;
        cell.updated_at = Utc::now();
        self.storage.update_cell(&cell).await?;
        Ok(cell)
    }

    pub async fn convert_cell_type(&self, cell_id: CellId, to: CellType) -> Result<NotebookCell> {
        let mut cell = self.storage.get_cell(cell_id).await?;
        cell.cell_type = to;
        // Non-executable markdown cells cannot have exit status; clear outputs
        if matches!(to, CellType::Markdown) {
            cell.output = None;
            cell.error_output = None;
            cell.exit_code = None;
            cell.duration_ms = None;
            cell.status = ExecutionStatus::Success;
            cell.shell = None;
        }
        cell.updated_at = Utc::now();
        self.storage.update_cell(&cell).await?;
        Ok(cell)
    }
}
