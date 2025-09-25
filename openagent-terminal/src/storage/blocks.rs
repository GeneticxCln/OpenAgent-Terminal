use sqlx::{Pool, Sqlite};

#[derive(Debug, Clone, Default)]
pub struct BlockFilter {}

#[derive(Debug, Clone, Default)]
pub struct BlockSort {}

#[derive(Debug, Clone, Default)]
pub struct StoredBlock {
    pub id: i64,
    pub command: String,
    pub output_preview: String,
}

#[derive(Clone)]
pub struct BlockStorage {
    pool: Pool<Sqlite>,
}

impl BlockStorage {
    pub fn new(pool: Pool<Sqlite>) -> Self { Self { pool } }

    pub async fn search_blocks(
        &self,
        _filter: &BlockFilter,
        _sort: &BlockSort,
    ) -> Result<Vec<StoredBlock>, sqlx::Error> {
        // Minimal stub: just prove the interface compiles
        let _ = &self.pool;
        Ok(Vec::new())
    }

    pub async fn get_session_blocks(
        &self,
        _session: &str,
    ) -> Result<Vec<StoredBlock>, sqlx::Error> {
        let _ = &self.pool;
        Ok(Vec::new())
    }

    pub async fn update_block_tags(
        &self,
        _id: i64,
        _tags: Vec<String>,
    ) -> Result<(), sqlx::Error> {
        let _ = &self.pool;
        Ok(())
    }
}
