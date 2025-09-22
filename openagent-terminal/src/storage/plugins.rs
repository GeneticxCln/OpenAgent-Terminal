use sqlx::SqlitePool;
use sqlx::{Pool, Row, Sqlite};

#[derive(Clone)]
pub struct PluginStorage {
    pool: SqlitePool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use tempfile::TempDir;

    #[tokio::test]
    async fn migrations_and_tables_exist() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("terminal.db");
        let storage = Storage::new(&db_path).await.expect("storage");
        let pool = storage.pool.clone();
        // Tables should exist
        for t in ["plugin_kv", "plugin_docs", "plugin_quotas", "ai_conversations", "ai_turns"] {
            let exists: (i64,) = sqlx::query_as(
                "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name = ?",
            )
            .bind(t)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(exists.0, 1, "missing table {t}");
        }
    }

    #[tokio::test]
    async fn quota_and_namespace_isolation() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("terminal.db");
        let storage = Storage::new(&db_path).await.expect("storage");
        let ps = storage.plugins();
        let plugin = "plugin.alpha";
        let ns = "main";
        // Set tight quotas for test
        ps.set_quota(
            plugin,
            Some(ns),
            QuotaConfig {
                max_total_bytes: Some(1024),
                max_value_bytes: Some(256),
                max_keys: Some(2),
                max_docs: Some(1),
            },
        )
        .await
        .unwrap();

        // Two small keys OK
        ps.put_kv(plugin, ns, "k1", &[0u8; 200]).await.unwrap();
        ps.put_kv(plugin, ns, "k2", &[0u8; 200]).await.unwrap();
        // Third should fail due to max_keys
        assert!(ps.put_kv(plugin, ns, "k3", &[0u8; 100]).await.is_err());
        // Oversized value should fail
        assert!(ps.put_kv(plugin, ns, "k2", &[0u8; 300]).await.is_err());

        // Docs
        ps.put_doc(plugin, ns, "doc1", &"{}".repeat(50)).await.unwrap();
        assert!(ps.put_doc(plugin, ns, "doc2", "{}").await.is_err());

        // Namespace isolation across plugins
        let other = "plugin.beta";
        assert!(ps.get_kv(other, ns, "k1").await.unwrap().is_none());
    }
}

#[derive(Debug, Clone)]
pub struct QuotaConfig {
    pub max_total_bytes: Option<i64>,
    pub max_value_bytes: Option<i64>,
    pub max_keys: Option<i64>,
    pub max_docs: Option<i64>,
}

impl PluginStorage {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    fn default_quota() -> QuotaConfig {
        QuotaConfig {
            max_total_bytes: Some(50 * 1024 * 1024), // 50 MiB
            max_value_bytes: Some(1024 * 1024),      // 1 MiB per entry/doc
            max_keys: Some(5000),
            max_docs: Some(5000),
        }
    }

    pub async fn set_quota(
        &self,
        plugin_id: &str,
        namespace: Option<&str>,
        quota: QuotaConfig,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO plugin_quotas (plugin_id, namespace, max_total_bytes, max_value_bytes, max_keys, max_docs)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(plugin_id, namespace) DO UPDATE SET
                max_total_bytes = excluded.max_total_bytes,
                max_value_bytes = excluded.max_value_bytes,
                max_keys = excluded.max_keys,
                max_docs = excluded.max_docs
            "#,
        )
        .bind(plugin_id)
        .bind(namespace)
        .bind(quota.max_total_bytes)
        .bind(quota.max_value_bytes)
        .bind(quota.max_keys)
        .bind(quota.max_docs)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_quota(
        &self,
        plugin_id: &str,
        namespace: Option<&str>,
    ) -> Result<QuotaConfig, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT max_total_bytes, max_value_bytes, max_keys, max_docs
            FROM plugin_quotas
            WHERE plugin_id = ? AND (namespace IS ? OR namespace = ?)
            ORDER BY namespace IS NOT NULL DESC
            LIMIT 1
            "#,
        )
        .bind(plugin_id)
        .bind(namespace)
        .bind(namespace.unwrap_or(""))
        .fetch_optional(&self.pool)
        .await?;

        Ok(match row {
            Some(row) => QuotaConfig {
                max_total_bytes: row.get::<Option<i64>, _>("max_total_bytes"),
                max_value_bytes: row.get::<Option<i64>, _>("max_value_bytes"),
                max_keys: row.get::<Option<i64>, _>("max_keys"),
                max_docs: row.get::<Option<i64>, _>("max_docs"),
            },
            None => QuotaConfig {
                max_total_bytes: None,
                max_value_bytes: None,
                max_keys: None,
                max_docs: None,
            },
        })
    }

    /// Put a namespaced key/value; enforces quotas.
    pub async fn put_kv(
        &self,
        plugin_id: &str,
        namespace: &str,
        key: &str,
        value: &[u8],
    ) -> Result<(), sqlx::Error> {
        let quota = {
            let q = self.get_quota(plugin_id, Some(namespace)).await?;
            // Merge with defaults: default acts as upper bound unless explicit quota provided
            let d = Self::default_quota();
            QuotaConfig {
                max_total_bytes: Some(q.max_total_bytes.unwrap_or(d.max_total_bytes.unwrap())),
                max_value_bytes: Some(q.max_value_bytes.unwrap_or(d.max_value_bytes.unwrap())),
                max_keys: Some(q.max_keys.unwrap_or(d.max_keys.unwrap())),
                max_docs: Some(q.max_docs.unwrap_or(d.max_docs.unwrap())),
            }
        };
        if let Some(max_val) = quota.max_value_bytes {
            if (value.len() as i64) > max_val {
                return Err(sqlx::Error::Protocol("value exceeds max_value_bytes".into()));
            }
        }
        let mut tx = self.pool.begin().await?;

        // Count keys and total bytes for namespace
        let row = sqlx::query(
            r#"SELECT COUNT(*) as keys, COALESCE(SUM(LENGTH(value)),0) as total FROM plugin_kv WHERE plugin_id=? AND namespace=?"#,
        )
        .bind(plugin_id)
        .bind(namespace)
        .fetch_one(&mut *tx)
        .await?;
        let keys: i64 = row.get::<i64, _>("keys");
        let total: i64 = row.get::<i64, _>("total");

        // Existing value size (if any) to adjust total
        let existing_len = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT LENGTH(value) FROM plugin_kv WHERE plugin_id=? AND namespace=? AND key=?",
        )
        .bind(plugin_id)
        .bind(namespace)
        .bind(key)
        .fetch_optional(&mut *tx)
        .await?
        .flatten()
        .unwrap_or(0);

        let new_total = total - existing_len + (value.len() as i64);
        if let Some(max_total) = quota.max_total_bytes {
            if new_total > max_total {
                return Err(sqlx::Error::Protocol("exceeds max_total_bytes".into()));
            }
        }
        if let Some(max_keys) = quota.max_keys {
            // If inserting a new key, enforce count
            let is_new = existing_len == 0;
            if is_new && keys + 1 > max_keys {
                return Err(sqlx::Error::Protocol("exceeds max_keys".into()));
            }
        }

        sqlx::query(
            r#"
            INSERT INTO plugin_kv (plugin_id, namespace, key, value)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(plugin_id, namespace, key) DO UPDATE SET value=excluded.value, updated_at=strftime('%s','now')*1000
            "#,
        )
        .bind(plugin_id)
        .bind(namespace)
        .bind(key)
        .bind(value)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_kv(
        &self,
        plugin_id: &str,
        namespace: &str,
        key: &str,
    ) -> Result<Option<Vec<u8>>, sqlx::Error> {
        let row = sqlx::query_scalar::<_, Option<Vec<u8>>>(
            "SELECT value FROM plugin_kv WHERE plugin_id=? AND namespace=? AND key=?",
        )
        .bind(plugin_id)
        .bind(namespace)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?
        .flatten();
        Ok(row)
    }

    /// Store a JSON document identified by doc_id within namespace; enforces doc count and total size quotas
    pub async fn put_doc(
        &self,
        plugin_id: &str,
        namespace: &str,
        doc_id: &str,
        doc_json: &str,
    ) -> Result<(), sqlx::Error> {
        let quota = {
            let q = self.get_quota(plugin_id, Some(namespace)).await?;
            let d = Self::default_quota();
            QuotaConfig {
                max_total_bytes: Some(q.max_total_bytes.unwrap_or(d.max_total_bytes.unwrap())),
                max_value_bytes: Some(q.max_value_bytes.unwrap_or(d.max_value_bytes.unwrap())),
                max_keys: Some(q.max_keys.unwrap_or(d.max_keys.unwrap())),
                max_docs: Some(q.max_docs.unwrap_or(d.max_docs.unwrap())),
            }
        };
        if let Some(max_val) = quota.max_value_bytes {
            if (doc_json.len() as i64) > max_val {
                return Err(sqlx::Error::Protocol("document exceeds max_value_bytes".into()));
            }
        }
        let mut tx = self.pool.begin().await?;

        // Count docs and total bytes
        let row = sqlx::query(
            r#"SELECT COUNT(*) as docs, COALESCE(SUM(LENGTH(doc_json)),0) as total FROM plugin_docs WHERE plugin_id=? AND namespace=?"#,
        )
        .bind(plugin_id)
        .bind(namespace)
        .fetch_one(&mut *tx)
        .await?;
        let docs: i64 = row.get::<i64, _>("docs");
        let total: i64 = row.get::<i64, _>("total");

        let existing_len = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT LENGTH(doc_json) FROM plugin_docs WHERE plugin_id=? AND namespace=? AND doc_id=?",
        )
        .bind(plugin_id)
        .bind(namespace)
        .bind(doc_id)
        .fetch_optional(&mut *tx)
        .await?
        .flatten()
        .unwrap_or(0);

        let new_total = total - existing_len + (doc_json.len() as i64);
        if let Some(max_total) = quota.max_total_bytes {
            if new_total > max_total {
                return Err(sqlx::Error::Protocol("exceeds max_total_bytes".into()));
            }
        }
        if let Some(max_docs) = quota.max_docs {
            let is_new = existing_len == 0;
            if is_new && docs + 1 > max_docs {
                return Err(sqlx::Error::Protocol("exceeds max_docs".into()));
            }
        }

        sqlx::query(
            r#"
            INSERT INTO plugin_docs (plugin_id, namespace, doc_id, doc_json)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(plugin_id, namespace, doc_id) DO UPDATE SET doc_json=excluded.doc_json, updated_at=strftime('%s','now')*1000
            "#,
        )
        .bind(plugin_id)
        .bind(namespace)
        .bind(doc_id)
        .bind(doc_json)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_doc(
        &self,
        plugin_id: &str,
        namespace: &str,
        doc_id: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let row = sqlx::query_scalar::<_, Option<String>>(
            "SELECT doc_json FROM plugin_docs WHERE plugin_id=? AND namespace=? AND doc_id=?",
        )
        .bind(plugin_id)
        .bind(namespace)
        .bind(doc_id)
        .fetch_optional(&self.pool)
        .await?
        .flatten();
        Ok(row)
    }
}
