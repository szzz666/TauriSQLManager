//! 连接池管理与连接串构造
//! 替代原项目 database/ConnectionManager（JDBC 连接管理）

use crate::model::{ConnectionConfig, DbType};
use sqlx::{MySqlPool, PgPool, SqlitePool};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// 一个数据库连接所对应的句柄
#[derive(Clone)]
pub enum Db {
    Sqlite(SqlitePool),
    MySql(MySqlPool),
    Postgres(PgPool),
    /// SQL Server 使用 tiberius，每次操作新建连接
    Mssql(ConnectionConfig),
}

impl Db {
    pub fn db_type(&self) -> DbType {
        match self {
            Db::Sqlite(_) => DbType::Sqlite,
            Db::MySql(_) => DbType::Mysql,
            Db::Postgres(_) => DbType::Postgresql,
            Db::Mssql(_) => DbType::Mssql,
        }
    }

    /// 关闭连接
    pub async fn close(self) {
        match self {
            Db::Sqlite(p) => p.close().await,
            Db::MySql(p) => p.close().await,
            Db::Postgres(p) => p.close().await,
            Db::Mssql(_) => {}
        }
    }
}

/// 连接池管理器：connId -> Db
#[derive(Default)]
pub struct PoolManager {
    pools: Mutex<HashMap<String, Db>>,
}

impl PoolManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 取已缓存的连接，不存在则建立
    pub async fn get(&self, config: &ConnectionConfig) -> Result<Db, String> {
        if let Some(db) = self.cached(&config.id) {
            return Ok(db);
        }
        let db = open(config).await?;
        if let Ok(mut map) = self.pools.lock() {
            map.insert(config.id.clone(), db.clone());
        }
        Ok(db)
    }

    /// 仅取已缓存的连接，不新建
    pub fn cached(&self, conn_id: &str) -> Option<Db> {
        self.pools.lock().ok().and_then(|m| m.get(conn_id).cloned())
    }

    pub fn is_active(&self, conn_id: &str) -> bool {
        self.cached(conn_id).is_some()
    }

    /// 关闭并移除指定连接的池
    pub fn close(&self, conn_id: &str) {
        let removed = self.pools.lock().ok().and_then(|mut m| m.remove(conn_id));
        if let Some(db) = removed {
            tokio::spawn(async move { db.close().await });
        }
    }

    /// 关闭所有连接（应用退出时调用）
    pub fn close_all(&self) {
        let all = self
            .pools
            .lock()
            .map(|mut m| std::mem::take(&mut *m))
            .unwrap_or_default();
        tokio::spawn(async move {
            for (_, db) in all {
                db.close().await;
            }
        });
    }
}

/// 依据配置建立连接
pub async fn open(config: &ConnectionConfig) -> Result<Db, String> {
    match config.db_type {
        DbType::Sqlite => {
            if config.database.trim().is_empty() {
                return Err("请选择 SQLite 数据库文件".to_string());
            }
            let opts = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&config.database)
                .create_if_missing(false)
                .foreign_keys(true);
            let pool = SqlitePool::connect_with(opts)
                .await
                .map_err(|e| format!("打开 SQLite 数据库失败: {e}"))?;
            Ok(Db::Sqlite(pool))
        }
        DbType::Mysql | DbType::Mariadb => {
            let pool = MySqlPool::connect(&mysql_url(config))
                .await
                .map_err(|e| format!("连接 MySQL 失败: {e}"))?;
            Ok(Db::MySql(pool))
        }
        DbType::Postgresql => {
            let pool = PgPool::connect(&postgres_url(config))
                .await
                .map_err(|e| format!("连接 PostgreSQL 失败: {e}"))?;
            Ok(Db::Postgres(pool))
        }
        DbType::Mssql => Ok(Db::Mssql(config.clone())),
    }
}

/// 构造 MySQL 连接串（用户名密码做百分号编码，避免特殊字符破坏 URL）
pub fn mysql_url(config: &ConnectionConfig) -> String {
    format!(
        "mysql://{}:{}@{}:{}/{}",
        encode(&config.username),
        encode(&config.password),
        config.host,
        config.effective_port(),
        encode(&config.database)
    )
}

/// 构造 PostgreSQL 连接串
pub fn postgres_url(config: &ConnectionConfig) -> String {
    format!(
        "postgres://{}:{}@{}:{}/{}",
        encode(&config.username),
        encode(&config.password),
        config.host,
        config.effective_port(),
        encode(&config.database)
    )
}

/// URL 组件百分号编码（保留 RFC 3986 unreserved 字符）
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// 测试连接，返回数据库版本信息
pub async fn test_connect(config: &ConnectionConfig) -> Result<String, String> {
    match config.db_type {
        DbType::Sqlite => {
            if !std::path::Path::new(&config.database).exists() {
                return Err(format!("SQLite 文件不存在: {}", config.database));
            }
            let opts = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&config.database)
                .read_only(true);
            let pool = SqlitePool::connect_with(opts)
                .await
                .map_err(|e| format!("打开 SQLite 数据库失败: {e}"))?;
            let version: String = sqlx::query_scalar("SELECT sqlite_version()")
                .fetch_one(&pool)
                .await
                .map_err(|e| format!("执行测试查询失败: {e}"))?;
            pool.close().await;
            Ok(format!("SQLite {version}"))
        }
        DbType::Mysql | DbType::Mariadb => {
            let pool = sqlx::mysql::MySqlPoolOptions::new()
                .acquire_timeout(Duration::from_secs(10))
                .max_connections(1)
                .connect(&mysql_url(config))
                .await
                .map_err(|e| format!("连接 MySQL 失败: {e}"))?;
            let version: String = sqlx::query_scalar("SELECT VERSION()")
                .fetch_one(&pool)
                .await
                .map_err(|e| format!("执行测试查询失败: {e}"))?;
            pool.close().await;
            Ok(format!("MySQL {version}"))
        }
        DbType::Postgresql => {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .acquire_timeout(Duration::from_secs(10))
                .max_connections(1)
                .connect(&postgres_url(config))
                .await
                .map_err(|e| format!("连接 PostgreSQL 失败: {e}"))?;
            let version: String = sqlx::query_scalar("SELECT version()")
                .fetch_one(&pool)
                .await
                .map_err(|e| format!("执行测试查询失败: {e}"))?;
            pool.close().await;
            Ok(version)
        }
        DbType::Mssql => crate::db::mssql::test_connect(config).await,
    }
}

/// 读取数据库版本（用于连接状态展示）
pub async fn version(db: &Db) -> Result<String, String> {
    match db {
        Db::Sqlite(p) => {
            let v: String = sqlx::query_scalar("SELECT sqlite_version()")
                .fetch_one(p)
                .await
                .map_err(|e| e.to_string())?;
            Ok(format!("SQLite {v}"))
        }
        Db::MySql(p) => {
            let v: String = sqlx::query_scalar("SELECT VERSION()")
                .fetch_one(p)
                .await
                .map_err(|e| e.to_string())?;
            Ok(format!("MySQL {v}"))
        }
        Db::Postgres(p) => {
            let v: String = sqlx::query_scalar("SELECT version()")
                .fetch_one(p)
                .await
                .map_err(|e| e.to_string())?;
            Ok(v)
        }
        Db::Mssql(config) => crate::db::mssql::test_connect(config).await,
    }
}
