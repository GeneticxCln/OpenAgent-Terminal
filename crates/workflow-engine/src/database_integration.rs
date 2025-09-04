use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConnection {
    pub id: Uuid,
    pub name: String,
    pub database_type: DatabaseType,
    pub connection_string: String,
    pub host: String,
    pub port: u16,
    pub database_name: String,
    pub username: String,
    pub ssl_mode: SslMode,
    pub pool_config: PoolConfig,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
    SQLite,
    MongoDB,
    Redis,
    MariaDB,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SslMode {
    Disable,
    Allow,
    Prefer,
    Require,
    VerifyCA,
    VerifyFull,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            min_connections: 1,
            connect_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300),
            max_lifetime: Duration::from_secs(3600),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub id: Uuid,
    pub query: String,
    pub connection_name: String,
    pub execution_time: Duration,
    pub rows_affected: Option<u64>,
    pub columns: Vec<String>,
    pub rows: Vec<HashMap<String, serde_json::Value>>,
    pub error: Option<String>,
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSchema {
    pub tables: Vec<TableInfo>,
    pub views: Vec<ViewInfo>,
    pub functions: Vec<FunctionInfo>,
    pub indexes: Vec<IndexInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub row_count: Option<u64>,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub is_primary_key: bool,
    pub is_foreign_key: bool,
    pub foreign_key_table: Option<String>,
    pub foreign_key_column: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewInfo {
    pub name: String,
    pub definition: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub parameters: Vec<String>,
    pub return_type: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub table_name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub index_type: String,
}

pub struct DatabaseIntegration {
    connections: Arc<Mutex<HashMap<Uuid, DatabaseConnection>>>,
    active_pools: Arc<Mutex<HashMap<Uuid, DatabasePool>>>,
    query_history: Arc<Mutex<Vec<QueryResult>>>,
}

enum DatabasePool {
    PostgreSQL(sqlx::PgPool),
    MySQL(sqlx::MySqlPool),
    SQLite(sqlx::SqlitePool),
    // MongoDB and Redis would require different connection types
}

impl DatabaseIntegration {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            active_pools: Arc::new(Mutex::new(HashMap::new())),
            query_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn add_connection(&self, mut connection: DatabaseConnection) -> Result<Uuid> {
        connection.id = Uuid::new_v4();
        connection.created_at = chrono::Utc::now();

        // Validate connection
        self.test_connection(&connection).await?;

        let id = connection.id;
        let mut connections = self.connections.lock().await;
        connections.insert(id, connection);

        Ok(id)
    }

    pub async fn remove_connection(&self, id: Uuid) -> Result<()> {
        let mut connections = self.connections.lock().await;
        let mut pools = self.active_pools.lock().await;

        if connections.remove(&id).is_none() {
            return Err(anyhow!("Connection not found"));
        }

        // Close the pool if it exists
        if let Some(pool) = pools.remove(&id) {
            match pool {
                DatabasePool::PostgreSQL(pool) => pool.close().await,
                DatabasePool::MySQL(pool) => pool.close().await,
                DatabasePool::SQLite(pool) => pool.close().await,
            }
        }

        Ok(())
    }

    pub async fn get_connections(&self) -> Result<Vec<DatabaseConnection>> {
        let connections = self.connections.lock().await;
        Ok(connections.values().cloned().collect())
    }

    pub async fn get_connection(&self, id: Uuid) -> Result<DatabaseConnection> {
        let connections = self.connections.lock().await;
        connections
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("Connection not found"))
    }

    async fn test_connection(&self, connection: &DatabaseConnection) -> Result<()> {
        match connection.database_type {
            DatabaseType::PostgreSQL => {
                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(1)
                    .connect_timeout(connection.pool_config.connect_timeout)
                    .connect(&connection.connection_string)
                    .await
                    .map_err(|e| anyhow!("Failed to connect to PostgreSQL: {}", e))?;
                
                sqlx::query("SELECT 1").execute(&pool).await
                    .map_err(|e| anyhow!("Failed to test PostgreSQL connection: {}", e))?;
                
                pool.close().await;
            }
            DatabaseType::MySQL | DatabaseType::MariaDB => {
                let pool = sqlx::mysql::MySqlPoolOptions::new()
                    .max_connections(1)
                    .connect_timeout(connection.pool_config.connect_timeout)
                    .connect(&connection.connection_string)
                    .await
                    .map_err(|e| anyhow!("Failed to connect to MySQL/MariaDB: {}", e))?;
                
                sqlx::query("SELECT 1").execute(&pool).await
                    .map_err(|e| anyhow!("Failed to test MySQL/MariaDB connection: {}", e))?;
                
                pool.close().await;
            }
            DatabaseType::SQLite => {
                let pool = sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_timeout(connection.pool_config.connect_timeout)
                    .connect(&connection.connection_string)
                    .await
                    .map_err(|e| anyhow!("Failed to connect to SQLite: {}", e))?;
                
                sqlx::query("SELECT 1").execute(&pool).await
                    .map_err(|e| anyhow!("Failed to test SQLite connection: {}", e))?;
                
                pool.close().await;
            }
            DatabaseType::MongoDB => {
                // MongoDB testing would require mongodb crate
                return Err(anyhow!("MongoDB support not yet implemented"));
            }
            DatabaseType::Redis => {
                // Redis testing would require redis crate
                return Err(anyhow!("Redis support not yet implemented"));
            }
        }

        Ok(())
    }

    async fn get_or_create_pool(&self, connection_id: Uuid) -> Result<()> {
        let mut pools = self.active_pools.lock().await;
        
        if pools.contains_key(&connection_id) {
            return Ok(());
        }

        let connections = self.connections.lock().await;
        let connection = connections
            .get(&connection_id)
            .ok_or_else(|| anyhow!("Connection not found"))?;

        let pool = match connection.database_type {
            DatabaseType::PostgreSQL => {
                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(connection.pool_config.max_connections)
                    .min_connections(connection.pool_config.min_connections)
                    .connect_timeout(connection.pool_config.connect_timeout)
                    .idle_timeout(Some(connection.pool_config.idle_timeout))
                    .max_lifetime(Some(connection.pool_config.max_lifetime))
                    .connect(&connection.connection_string)
                    .await
                    .map_err(|e| anyhow!("Failed to create PostgreSQL pool: {}", e))?;
                
                DatabasePool::PostgreSQL(pool)
            }
            DatabaseType::MySQL | DatabaseType::MariaDB => {
                let pool = sqlx::mysql::MySqlPoolOptions::new()
                    .max_connections(connection.pool_config.max_connections)
                    .min_connections(connection.pool_config.min_connections)
                    .connect_timeout(connection.pool_config.connect_timeout)
                    .idle_timeout(Some(connection.pool_config.idle_timeout))
                    .max_lifetime(Some(connection.pool_config.max_lifetime))
                    .connect(&connection.connection_string)
                    .await
                    .map_err(|e| anyhow!("Failed to create MySQL/MariaDB pool: {}", e))?;
                
                DatabasePool::MySQL(pool)
            }
            DatabaseType::SQLite => {
                let pool = sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(connection.pool_config.max_connections)
                    .min_connections(connection.pool_config.min_connections)
                    .connect_timeout(connection.pool_config.connect_timeout)
                    .idle_timeout(Some(connection.pool_config.idle_timeout))
                    .max_lifetime(Some(connection.pool_config.max_lifetime))
                    .connect(&connection.connection_string)
                    .await
                    .map_err(|e| anyhow!("Failed to create SQLite pool: {}", e))?;
                
                DatabasePool::SQLite(pool)
            }
            DatabaseType::MongoDB | DatabaseType::Redis => {
                return Err(anyhow!("Unsupported database type"));
            }
        };

        pools.insert(connection_id, pool);
        Ok(())
    }

    pub async fn execute_query(&self, connection_id: Uuid, query: &str) -> Result<QueryResult> {
        let start_time = std::time::Instant::now();
        let query_id = Uuid::new_v4();

        self.get_or_create_pool(connection_id).await?;

        let connection_name = {
            let connections = self.connections.lock().await;
            connections
                .get(&connection_id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Unknown".to_string())
        };

        let pools = self.active_pools.lock().await;
        let pool = pools
            .get(&connection_id)
            .ok_or_else(|| anyhow!("Pool not found"))?;

        let mut result = QueryResult {
            id: query_id,
            query: query.to_string(),
            connection_name,
            execution_time: Duration::from_millis(0),
            rows_affected: None,
            columns: Vec::new(),
            rows: Vec::new(),
            error: None,
            executed_at: chrono::Utc::now(),
        };

        let query_result = match pool {
            DatabasePool::PostgreSQL(pool) => {
                self.execute_postgres_query(pool, query).await
            }
            DatabasePool::MySQL(pool) => {
                self.execute_mysql_query(pool, query).await
            }
            DatabasePool::SQLite(pool) => {
                self.execute_sqlite_query(pool, query).await
            }
        };

        result.execution_time = start_time.elapsed();

        match query_result {
            Ok((columns, rows, rows_affected)) => {
                result.columns = columns;
                result.rows = rows;
                result.rows_affected = rows_affected;
            }
            Err(e) => {
                result.error = Some(e.to_string());
            }
        }

        // Update connection last used
        {
            let mut connections = self.connections.lock().await;
            if let Some(connection) = connections.get_mut(&connection_id) {
                connection.last_used = Some(chrono::Utc::now());
            }
        }

        // Store in query history
        {
            let mut history = self.query_history.lock().await;
            history.push(result.clone());
            
            // Keep only last 1000 queries
            if history.len() > 1000 {
                history.remove(0);
            }
        }

        Ok(result)
    }

    async fn execute_postgres_query(
        &self,
        pool: &sqlx::PgPool,
        query: &str,
    ) -> Result<(Vec<String>, Vec<HashMap<String, serde_json::Value>>, Option<u64>)> {
        use sqlx::Row;

        if query.trim().to_lowercase().starts_with("select") {
            let rows = sqlx::query(query).fetch_all(pool).await
                .map_err(|e| anyhow!("Query execution failed: {}", e))?;

            let mut columns = Vec::new();
            let mut result_rows = Vec::new();

            if let Some(first_row) = rows.first() {
                for column in first_row.columns() {
                    columns.push(column.name().to_string());
                }
            }

            for row in rows {
                let mut row_map = HashMap::new();
                for (i, column_name) in columns.iter().enumerate() {
                    let value = self.extract_postgres_value(&row, i)?;
                    row_map.insert(column_name.clone(), value);
                }
                result_rows.push(row_map);
            }

            Ok((columns, result_rows, None))
        } else {
            let result = sqlx::query(query).execute(pool).await
                .map_err(|e| anyhow!("Query execution failed: {}", e))?;

            Ok((Vec::new(), Vec::new(), Some(result.rows_affected())))
        }
    }

    async fn execute_mysql_query(
        &self,
        pool: &sqlx::MySqlPool,
        query: &str,
    ) -> Result<(Vec<String>, Vec<HashMap<String, serde_json::Value>>, Option<u64>)> {
        use sqlx::Row;

        if query.trim().to_lowercase().starts_with("select") {
            let rows = sqlx::query(query).fetch_all(pool).await
                .map_err(|e| anyhow!("Query execution failed: {}", e))?;

            let mut columns = Vec::new();
            let mut result_rows = Vec::new();

            if let Some(first_row) = rows.first() {
                for column in first_row.columns() {
                    columns.push(column.name().to_string());
                }
            }

            for row in rows {
                let mut row_map = HashMap::new();
                for (i, column_name) in columns.iter().enumerate() {
                    let value = self.extract_mysql_value(&row, i)?;
                    row_map.insert(column_name.clone(), value);
                }
                result_rows.push(row_map);
            }

            Ok((columns, result_rows, None))
        } else {
            let result = sqlx::query(query).execute(pool).await
                .map_err(|e| anyhow!("Query execution failed: {}", e))?;

            Ok((Vec::new(), Vec::new(), Some(result.rows_affected())))
        }
    }

    async fn execute_sqlite_query(
        &self,
        pool: &sqlx::SqlitePool,
        query: &str,
    ) -> Result<(Vec<String>, Vec<HashMap<String, serde_json::Value>>, Option<u64>)> {
        use sqlx::Row;

        if query.trim().to_lowercase().starts_with("select") {
            let rows = sqlx::query(query).fetch_all(pool).await
                .map_err(|e| anyhow!("Query execution failed: {}", e))?;

            let mut columns = Vec::new();
            let mut result_rows = Vec::new();

            if let Some(first_row) = rows.first() {
                for column in first_row.columns() {
                    columns.push(column.name().to_string());
                }
            }

            for row in rows {
                let mut row_map = HashMap::new();
                for (i, column_name) in columns.iter().enumerate() {
                    let value = self.extract_sqlite_value(&row, i)?;
                    row_map.insert(column_name.clone(), value);
                }
                result_rows.push(row_map);
            }

            Ok((columns, result_rows, None))
        } else {
            let result = sqlx::query(query).execute(pool).await
                .map_err(|e| anyhow!("Query execution failed: {}", e))?;

            Ok((Vec::new(), Vec::new(), Some(result.rows_affected())))
        }
    }

    fn extract_postgres_value(&self, row: &sqlx::postgres::PgRow, index: usize) -> Result<serde_json::Value> {
        use sqlx::postgres::PgValueRef;
        use sqlx::ValueRef;

        let value_ref = row.try_get_raw(index)?;
        
        if value_ref.is_null() {
            return Ok(serde_json::Value::Null);
        }

        // Try different types
        if let Ok(val) = row.try_get::<String, _>(index) {
            return Ok(serde_json::Value::String(val));
        }
        if let Ok(val) = row.try_get::<i32, _>(index) {
            return Ok(serde_json::Value::Number(val.into()));
        }
        if let Ok(val) = row.try_get::<i64, _>(index) {
            return Ok(serde_json::Value::Number(val.into()));
        }
        if let Ok(val) = row.try_get::<f64, _>(index) {
            if let Some(num) = serde_json::Number::from_f64(val) {
                return Ok(serde_json::Value::Number(num));
            }
        }
        if let Ok(val) = row.try_get::<bool, _>(index) {
            return Ok(serde_json::Value::Bool(val));
        }

        // Fallback to string representation
        Ok(serde_json::Value::String(format!("{:?}", value_ref)))
    }

    fn extract_mysql_value(&self, row: &sqlx::mysql::MySqlRow, index: usize) -> Result<serde_json::Value> {
        use sqlx::ValueRef;

        let value_ref = row.try_get_raw(index)?;
        
        if value_ref.is_null() {
            return Ok(serde_json::Value::Null);
        }

        // Try different types
        if let Ok(val) = row.try_get::<String, _>(index) {
            return Ok(serde_json::Value::String(val));
        }
        if let Ok(val) = row.try_get::<i32, _>(index) {
            return Ok(serde_json::Value::Number(val.into()));
        }
        if let Ok(val) = row.try_get::<i64, _>(index) {
            return Ok(serde_json::Value::Number(val.into()));
        }
        if let Ok(val) = row.try_get::<f64, _>(index) {
            if let Some(num) = serde_json::Number::from_f64(val) {
                return Ok(serde_json::Value::Number(num));
            }
        }
        if let Ok(val) = row.try_get::<bool, _>(index) {
            return Ok(serde_json::Value::Bool(val));
        }

        // Fallback to string representation
        Ok(serde_json::Value::String(format!("{:?}", value_ref)))
    }

    fn extract_sqlite_value(&self, row: &sqlx::sqlite::SqliteRow, index: usize) -> Result<serde_json::Value> {
        use sqlx::ValueRef;

        let value_ref = row.try_get_raw(index)?;
        
        if value_ref.is_null() {
            return Ok(serde_json::Value::Null);
        }

        // Try different types
        if let Ok(val) = row.try_get::<String, _>(index) {
            return Ok(serde_json::Value::String(val));
        }
        if let Ok(val) = row.try_get::<i32, _>(index) {
            return Ok(serde_json::Value::Number(val.into()));
        }
        if let Ok(val) = row.try_get::<i64, _>(index) {
            return Ok(serde_json::Value::Number(val.into()));
        }
        if let Ok(val) = row.try_get::<f64, _>(index) {
            if let Some(num) = serde_json::Number::from_f64(val) {
                return Ok(serde_json::Value::Number(num));
            }
        }
        if let Ok(val) = row.try_get::<bool, _>(index) {
            return Ok(serde_json::Value::Bool(val));
        }

        // Fallback to string representation
        Ok(serde_json::Value::String(format!("{:?}", value_ref)))
    }

    pub async fn get_database_schema(&self, connection_id: Uuid) -> Result<DatabaseSchema> {
        self.get_or_create_pool(connection_id).await?;

        let connections = self.connections.lock().await;
        let connection = connections
            .get(&connection_id)
            .ok_or_else(|| anyhow!("Connection not found"))?;

        match connection.database_type {
            DatabaseType::PostgreSQL => self.get_postgres_schema(connection_id).await,
            DatabaseType::MySQL | DatabaseType::MariaDB => self.get_mysql_schema(connection_id).await,
            DatabaseType::SQLite => self.get_sqlite_schema(connection_id).await,
            _ => Err(anyhow!("Schema introspection not supported for this database type")),
        }
    }

    async fn get_postgres_schema(&self, connection_id: Uuid) -> Result<DatabaseSchema> {
        let tables_query = r#"
            SELECT 
                t.table_name,
                c.column_name,
                c.data_type,
                c.is_nullable,
                c.column_default,
                CASE WHEN pk.column_name IS NOT NULL THEN true ELSE false END as is_primary_key
            FROM information_schema.tables t
            LEFT JOIN information_schema.columns c ON t.table_name = c.table_name
            LEFT JOIN information_schema.key_column_usage pk ON c.table_name = pk.table_name 
                AND c.column_name = pk.column_name
            WHERE t.table_schema = 'public' AND t.table_type = 'BASE TABLE'
            ORDER BY t.table_name, c.ordinal_position
        "#;

        let result = self.execute_query(connection_id, tables_query).await?;
        let tables = self.parse_table_info_from_query_result(&result)?;

        Ok(DatabaseSchema {
            tables,
            views: Vec::new(),
            functions: Vec::new(),
            indexes: Vec::new(),
        })
    }

    async fn get_mysql_schema(&self, connection_id: Uuid) -> Result<DatabaseSchema> {
        let tables_query = r#"
            SELECT 
                t.TABLE_NAME as table_name,
                c.COLUMN_NAME as column_name,
                c.DATA_TYPE as data_type,
                c.IS_NULLABLE as is_nullable,
                c.COLUMN_DEFAULT as column_default,
                CASE WHEN c.COLUMN_KEY = 'PRI' THEN 1 ELSE 0 END as is_primary_key
            FROM information_schema.TABLES t
            LEFT JOIN information_schema.COLUMNS c ON t.TABLE_NAME = c.TABLE_NAME
            WHERE t.TABLE_SCHEMA = DATABASE() AND t.TABLE_TYPE = 'BASE TABLE'
            ORDER BY t.TABLE_NAME, c.ORDINAL_POSITION
        "#;

        let result = self.execute_query(connection_id, tables_query).await?;
        let tables = self.parse_table_info_from_query_result(&result)?;

        Ok(DatabaseSchema {
            tables,
            views: Vec::new(),
            functions: Vec::new(),
            indexes: Vec::new(),
        })
    }

    async fn get_sqlite_schema(&self, connection_id: Uuid) -> Result<DatabaseSchema> {
        let tables_query = r#"
            SELECT 
                m.name as table_name,
                p.name as column_name,
                p.type as data_type,
                CASE WHEN p."notnull" = 0 THEN 'YES' ELSE 'NO' END as is_nullable,
                p.dflt_value as column_default,
                p.pk as is_primary_key
            FROM sqlite_master m
            LEFT JOIN pragma_table_info(m.name) p
            WHERE m.type = 'table' AND m.name NOT LIKE 'sqlite_%'
            ORDER BY m.name, p.cid
        "#;

        let result = self.execute_query(connection_id, tables_query).await?;
        let tables = self.parse_table_info_from_query_result(&result)?;

        Ok(DatabaseSchema {
            tables,
            views: Vec::new(),
            functions: Vec::new(),
            indexes: Vec::new(),
        })
    }

    fn parse_table_info_from_query_result(&self, result: &QueryResult) -> Result<Vec<TableInfo>> {
        let mut tables: HashMap<String, TableInfo> = HashMap::new();

        for row in &result.rows {
            let table_name = row.get("table_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing table_name"))?
                .to_string();

            let column_name = row.get("column_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let data_type = row.get("data_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let nullable = row.get("is_nullable")
                .and_then(|v| v.as_str())
                .map(|s| s.eq_ignore_ascii_case("yes") || s == "1")
                .unwrap_or(true);

            let default_value = row.get("column_default")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let is_primary_key = row.get("is_primary_key")
                .and_then(|v| {
                    if let Some(b) = v.as_bool() {
                        Some(b)
                    } else if let Some(n) = v.as_i64() {
                        Some(n != 0)
                    } else {
                        None
                    }
                })
                .unwrap_or(false);

            let column = ColumnInfo {
                name: column_name,
                data_type,
                nullable,
                default_value,
                is_primary_key,
                is_foreign_key: false,
                foreign_key_table: None,
                foreign_key_column: None,
            };

            tables.entry(table_name.clone())
                .or_insert_with(|| TableInfo {
                    name: table_name,
                    columns: Vec::new(),
                    row_count: None,
                    size_bytes: None,
                })
                .columns
                .push(column);
        }

        Ok(tables.into_values().collect())
    }

    pub async fn get_query_history(&self) -> Result<Vec<QueryResult>> {
        let history = self.query_history.lock().await;
        Ok(history.clone())
    }

    pub async fn clear_query_history(&self) -> Result<()> {
        let mut history = self.query_history.lock().await;
        history.clear();
        Ok(())
    }

    pub fn build_connection_string(
        database_type: &DatabaseType,
        host: &str,
        port: u16,
        database_name: &str,
        username: &str,
        password: &str,
        ssl_mode: &SslMode,
    ) -> String {
        match database_type {
            DatabaseType::PostgreSQL => {
                let ssl_mode_str = match ssl_mode {
                    SslMode::Disable => "disable",
                    SslMode::Allow => "allow",
                    SslMode::Prefer => "prefer",
                    SslMode::Require => "require",
                    SslMode::VerifyCA => "verify-ca",
                    SslMode::VerifyFull => "verify-full",
                };

                format!(
                    "postgresql://{}:{}@{}:{}/{}?sslmode={}",
                    username, password, host, port, database_name, ssl_mode_str
                )
            }
            DatabaseType::MySQL | DatabaseType::MariaDB => {
                format!(
                    "mysql://{}:{}@{}:{}/{}",
                    username, password, host, port, database_name
                )
            }
            DatabaseType::SQLite => {
                format!("sqlite:{}", database_name)
            }
            DatabaseType::MongoDB => {
                format!(
                    "mongodb://{}:{}@{}:{}/{}",
                    username, password, host, port, database_name
                )
            }
            DatabaseType::Redis => {
                format!("redis://{}:{}@{}:{}", username, password, host, port)
            }
        }
    }

    pub async fn discover_local_databases(&self) -> Result<Vec<DatabaseConnection>> {
        let mut discovered = Vec::new();

        // Check for common local database setups
        if let Ok(_) = std::process::Command::new("pg_isready").output() {
            discovered.push(DatabaseConnection {
                id: Uuid::new_v4(),
                name: "Local PostgreSQL".to_string(),
                database_type: DatabaseType::PostgreSQL,
                connection_string: "postgresql://localhost:5432/postgres".to_string(),
                host: "localhost".to_string(),
                port: 5432,
                database_name: "postgres".to_string(),
                username: "postgres".to_string(),
                ssl_mode: SslMode::Prefer,
                pool_config: PoolConfig::default(),
                created_at: chrono::Utc::now(),
                last_used: None,
            });
        }

        // Check for MySQL/MariaDB
        if let Ok(_) = std::process::Command::new("mysql").arg("--version").output() {
            discovered.push(DatabaseConnection {
                id: Uuid::new_v4(),
                name: "Local MySQL".to_string(),
                database_type: DatabaseType::MySQL,
                connection_string: "mysql://localhost:3306/mysql".to_string(),
                host: "localhost".to_string(),
                port: 3306,
                database_name: "mysql".to_string(),
                username: "root".to_string(),
                ssl_mode: SslMode::Prefer,
                pool_config: PoolConfig::default(),
                created_at: chrono::Utc::now(),
                last_used: None,
            });
        }

        // Check for SQLite databases in current directory
        if let Ok(entries) = std::fs::read_dir(".") {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "db" || extension == "sqlite" || extension == "sqlite3" {
                        let path = entry.path();
                        discovered.push(DatabaseConnection {
                            id: Uuid::new_v4(),
                            name: format!("SQLite: {}", path.file_name().unwrap().to_string_lossy()),
                            database_type: DatabaseType::SQLite,
                            connection_string: format!("sqlite:{}", path.to_string_lossy()),
                            host: "localhost".to_string(),
                            port: 0,
                            database_name: path.to_string_lossy().to_string(),
                            username: "".to_string(),
                            ssl_mode: SslMode::Disable,
                            pool_config: PoolConfig::default(),
                            created_at: chrono::Utc::now(),
                            last_used: None,
                        });
                    }
                }
            }
        }

        Ok(discovered)
    }

    pub async fn close_all_connections(&self) -> Result<()> {
        let mut pools = self.active_pools.lock().await;
        
        for (_, pool) in pools.drain() {
            match pool {
                DatabasePool::PostgreSQL(pool) => pool.close().await,
                DatabasePool::MySQL(pool) => pool.close().await,
                DatabasePool::SQLite(pool) => pool.close().await,
            }
        }

        Ok(())
    }

    pub async fn export_query_results(&self, result: &QueryResult, format: ExportFormat) -> Result<String> {
        match format {
            ExportFormat::JSON => {
                Ok(serde_json::to_string_pretty(&result.rows)?)
            }
            ExportFormat::CSV => {
                let mut csv = String::new();
                
                // Add header
                csv.push_str(&result.columns.join(","));
                csv.push('\n');
                
                // Add rows
                for row in &result.rows {
                    let row_values: Vec<String> = result.columns
                        .iter()
                        .map(|col| {
                            row.get(col)
                                .map(|v| match v {
                                    serde_json::Value::String(s) => format!("\"{}\"", s.replace("\"", "\"\"")),
                                    serde_json::Value::Number(n) => n.to_string(),
                                    serde_json::Value::Bool(b) => b.to_string(),
                                    serde_json::Value::Null => "".to_string(),
                                    _ => format!("\"{}\"", v.to_string()),
                                })
                                .unwrap_or_else(|| "".to_string())
                        })
                        .collect();
                    
                    csv.push_str(&row_values.join(","));
                    csv.push('\n');
                }
                
                Ok(csv)
            }
            ExportFormat::Markdown => {
                let mut md = String::new();
                
                // Add table header
                md.push_str(&format!("| {} |\n", result.columns.join(" | ")));
                md.push_str(&format!("| {} |\n", result.columns.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")));
                
                // Add rows
                for row in &result.rows {
                    let row_values: Vec<String> = result.columns
                        .iter()
                        .map(|col| {
                            row.get(col)
                                .map(|v| match v {
                                    serde_json::Value::String(s) => s.clone(),
                                    serde_json::Value::Number(n) => n.to_string(),
                                    serde_json::Value::Bool(b) => b.to_string(),
                                    serde_json::Value::Null => "NULL".to_string(),
                                    _ => v.to_string(),
                                })
                                .unwrap_or_else(|| "".to_string())
                        })
                        .collect();
                    
                    md.push_str(&format!("| {} |\n", row_values.join(" | ")));
                }
                
                Ok(md)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    JSON,
    CSV,
    Markdown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_integration_creation() {
        let db_integration = DatabaseIntegration::new();
        assert!(true); // Basic creation should work
    }

    #[test]
    fn test_build_connection_string() {
        let conn_str = DatabaseIntegration::build_connection_string(
            &DatabaseType::PostgreSQL,
            "localhost",
            5432,
            "testdb",
            "user",
            "pass",
            &SslMode::Prefer,
        );

        assert_eq!(conn_str, "postgresql://user:pass@localhost:5432/testdb?sslmode=prefer");
    }

    #[test]
    fn test_sqlite_connection_string() {
        let conn_str = DatabaseIntegration::build_connection_string(
            &DatabaseType::SQLite,
            "",
            0,
            "./test.db",
            "",
            "",
            &SslMode::Disable,
        );

        assert_eq!(conn_str, "sqlite:./test.db");
    }
}
