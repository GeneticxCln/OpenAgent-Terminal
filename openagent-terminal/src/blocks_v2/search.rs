// Search engine for blocks

use super::{Block, SearchQuery};
use crate::blocks_v2::storage::BlockStorage;
use anyhow::Result;
use std::sync::Arc;

/// Search engine for blocks
#[allow(dead_code)]
pub struct SearchEngine {
    storage: Arc<BlockStorage>,
}

impl SearchEngine {
    pub async fn new(storage: Arc<BlockStorage>) -> Result<Self> {
        Ok(Self { storage })
    }

    pub async fn index_block(&self, _block: &Arc<Block>) -> Result<()> {
        // Indexing happens automatically via SQLite triggers
        Ok(())
    }

    pub async fn update_block(&self, _block: &Block) -> Result<()> {
        // Updates happen automatically via SQLite triggers
        Ok(())
    }

    pub async fn search(&self, query: SearchQuery) -> Result<Vec<Arc<Block>>> {
        // Delegate to storage to leverage SQL/FTS
        self.storage.search(&query).await
    }
}
