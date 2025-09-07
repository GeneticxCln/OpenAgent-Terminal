//! Project indexer and file tree structures

use anyhow::{anyhow, Result};
use ignore::WalkBuilder;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectFile {
    pub path: PathBuf,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: BTreeMap<String, FileNode>,
}

impl FileNode {
    pub fn new(name: String, path: PathBuf, is_dir: bool) -> Self {
        Self { name, path, is_dir, children: BTreeMap::new() }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectIndexConfig {
    pub root: PathBuf,
    pub follow_symlinks: bool,
    pub max_depth: Option<usize>,
}

impl ProjectIndexConfig {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), follow_symlinks: false, max_depth: None }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectIndex {
    pub root: PathBuf,
    pub files: Arc<RwLock<Vec<ProjectFile>>>,
    pub tree: Arc<RwLock<FileNode>>, // root node
}

impl ProjectIndex {
    pub fn build(config: &ProjectIndexConfig) -> Result<Self> {
        let root = config.root.canonicalize().unwrap_or_else(|_| config.root.clone());
        let mut files = Vec::new();
        let mut root_node = FileNode::new(
            root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "/".into()),
            root.clone(),
            true,
        );

        // Use ignore (gitignore aware) walker
        let mut builder = WalkBuilder::new(&root);
        builder.hidden(false).follow_links(config.follow_symlinks);
        if let Some(d) = config.max_depth {
            builder.max_depth(Some(d));
        }
        let walker = builder.build();

        for entry in walker.flatten() {
            if entry.depth() == 0 {
                continue;
            }
            let path = entry.path().to_path_buf();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            files.push(ProjectFile { path: path.clone(), is_dir });
            insert_into_tree(&mut root_node, &root, &path, is_dir)?;
        }

        Ok(Self {
            root,
            files: Arc::new(RwLock::new(files)),
            tree: Arc::new(RwLock::new(root_node)),
        })
    }

    pub fn snapshot_files(&self) -> Vec<ProjectFile> {
        self.files.read().clone()
    }

    pub fn snapshot_tree(&self) -> FileNode {
        self.tree.read().clone()
    }

    /// Start a watcher which updates the index on FS changes. Returns a shutdown handle.
    pub fn start_watcher(&self) -> Result<WatcherHandle> {
        let root = self.root.clone();
        let files = self.files.clone();
        let tree = self.tree.clone();

        let (tx, mut rx) = mpsc::unbounded_channel::<notify::Result<notify::Event>>();

        // Spawn a thread to watch and forward events to async channel
        let mut watcher: RecommendedWatcher = Watcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            notify::Config::default(),
        )?;
        watcher.watch(&root, RecursiveMode::Recursive)?;

        // Spawn async task to process events
        tokio::spawn(async move {
            while let Some(res) = rx.recv().await {
                if let Ok(event) = res {
                    let mut needs_rebuild = false;
                    match event.kind {
                        notify::EventKind::Create(_)
                        | notify::EventKind::Modify(_)
                        | notify::EventKind::Remove(_) => {
                            needs_rebuild = true;
                        },
                        _ => {},
                    }
                    if needs_rebuild {
                        if let Ok(new_index) = ProjectIndex::build(&ProjectIndexConfig {
                            root: root.clone(),
                            follow_symlinks: false,
                            max_depth: None,
                        }) {
                            *files.write() = new_index.snapshot_files();
                            *tree.write() = new_index.snapshot_tree();
                        }
                    }
                }
            }
        });

        Ok(WatcherHandle { watcher: Some(watcher) })
    }
}

pub struct WatcherHandle {
    watcher: Option<RecommendedWatcher>,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        // RecommendedWatcher drops itself; nothing extra to do.
        let _ = self.watcher.take();
    }
}

fn insert_into_tree(root: &mut FileNode, base: &Path, path: &Path, is_dir: bool) -> Result<()> {
    let rel = path.strip_prefix(base).map_err(|_| anyhow!("failed to strip prefix"))?;
    let mut curr = root;
    let mut accum = base.to_path_buf();

    for comp in rel.components() {
        let name = comp.as_os_str().to_string_lossy().to_string();
        accum = accum.join(&name);
        let is_last = accum == path;
        let is_node_dir = if is_last { is_dir } else { true };

        curr = curr
            .children
            .entry(name.clone())
            .or_insert_with(|| FileNode::new(name.clone(), accum.clone(), is_node_dir));
    }

    Ok(())
}
