use anyhow::Result;
use dirs;

use crate::blocks_v2::{BlockManager, BlockRecord, SearchQuery, BlockId};

#[derive(Debug, Clone, Default)]
pub struct BlockFilter {
    pub text: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct BlockSort {
    pub field: Option<String>,
    pub order: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct StoredBlock {
    pub id: i64,
    pub command: String,
    pub output_preview: String,
}

#[derive(Clone)]
pub struct BlockStorage {
    manager: std::sync::Arc<tokio::sync::RwLock<BlockManager>>,
}

impl BlockStorage {
    /// Create storage with default on-disk location under XDG data dir
    pub async fn new_default() -> Result<Self> {
        let root = dirs::data_dir()
            .unwrap_or(std::path::PathBuf::from("."))
            .join("openagent-terminal")
            .join("blocks");
        let manager = BlockManager::new(root).await?;
        Ok(Self { manager: std::sync::Arc::new(tokio::sync::RwLock::new(manager)) })
    }

    /// Create storage with an explicit root directory
    pub async fn new_with_root(root: std::path::PathBuf) -> Result<Self> {
        let manager = BlockManager::new(root).await?;
        Ok(Self { manager: std::sync::Arc::new(tokio::sync::RwLock::new(manager)) })
    }

    pub async fn search_blocks(&self, filter: &BlockFilter, sort: &BlockSort) -> Result<Vec<StoredBlock>> {
        let q = SearchQuery {
            text: filter.text.as_deref(),
            directory: None,
            shell: None,
            tags: None,
            starred: None,
            from_date: None,
            to_date: None,
            limit: filter.limit.or(Some(100)),
            offset: filter.offset.or(Some(0)),
            sort_by: sort.field.as_deref(),
            sort_order: sort.order.as_deref(),
        };
        let mgr = self.manager.read().await;
        let results: Vec<BlockRecord> = mgr.search(q).await?;
        Ok(results
            .into_iter()
            .map(|r| StoredBlock {
                id: r.id.0 as i64,
                command: r.command,
                output_preview: r.output.chars().take(200).collect(),
            })
            .collect())
    }

    pub async fn get_session_blocks(&self, _session: &str) -> Result<Vec<StoredBlock>> {
        // For now, reuse a search with default query; when session support is added, add WHERE clause.
        self.search_blocks(&BlockFilter::default(), &BlockSort::default()).await
    }

    pub async fn update_block_tags(&self, id: i64, tags: Vec<String>) -> Result<()> {
        let mut mgr = self.manager.write().await;
        mgr.update_block_tags(BlockId(id as u64), tags).await?;
        Ok(())
    }

    /// Toggle starred flag for a block and return the new value
    pub async fn toggle_starred(&self, id: i64) -> Result<bool> {
        let mut mgr = self.manager.write().await;
        let new_starred = mgr.toggle_starred(BlockId(id as u64)).await?;
        Ok(new_starred)
    }
}
