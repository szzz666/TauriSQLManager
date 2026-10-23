//! 验证 SQLite 表列表能返回真实行数（回归：此前恒为 0）

use std::path::PathBuf;
use tauri_sql_manager_lib::db::{conn, meta};
use tauri_sql_manager_lib::model::{ConnectionConfig, DbType};

fn temp_config(name: &str) -> (ConnectionConfig, PathBuf) {
    let mut path = std::env::temp_dir();
    path.push(format!("tsm_rowcount_{}_{}.db", name, std::process::id()));
    let _ = std::fs::remove_file(&path);
    let config = ConnectionConfig {
        id: format!("test-{name}"),
        name: name.to_string(),
        db_type: DbType::Sqlite,
        host: String::new(),
        port: 0,
        database: path.to_string_lossy().to_string(),
        username: String::new(),
        password: String::new(),
        auto_reconnect: true,
        created_at: 0,
        updated_at: 0,
    };
    (config, path)
}

#[tokio::test]
async fn sqlite_table_row_counts_are_real() {
    let (config, path) = temp_config("counts");

    // 建库并写入已知数量的数据
    {
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let pool = sqlx::sqlite::SqlitePool::connect(&url).await.expect("建库");
        sqlx::query("CREATE TABLE empty_one (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("建表1");
        sqlx::query("CREATE TABLE filled (id INTEGER PRIMARY KEY, v TEXT)")
            .execute(&pool)
            .await
            .expect("建表2");
        for i in 0..7 {
            sqlx::query("INSERT INTO filled (v) VALUES (?)")
                .bind(format!("v{i}"))
                .execute(&pool)
                .await
                .expect("插入");
        }
        pool.close().await;
    }

    let db = conn::open(&config).await.expect("打开数据库");
    let tables = meta::list_tables(&db).await.expect("列出表");

    let empty = tables.iter().find(|t| t.name == "empty_one").expect("应有 empty_one");
    let filled = tables.iter().find(|t| t.name == "filled").expect("应有 filled");

    assert_eq!(empty.row_count, 0, "空表行数应为 0");
    assert_eq!(filled.row_count, 7, "filled 应有 7 行");

    // sqlite_sequence 等内部表不应出现
    assert!(!tables.iter().any(|t| t.name.starts_with("sqlite_")), "不应包含内部表");

    db.close().await;
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn sqlite_autoindex_is_not_listed_as_user_index() {
    let (config, path) = temp_config("idx");
    {
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let pool = sqlx::sqlite::SqlitePool::connect(&url).await.expect("建库");
        sqlx::query("CREATE TABLE t (id INTEGER PRIMARY KEY, email TEXT UNIQUE, name TEXT)")
            .execute(&pool)
            .await
            .expect("建表");
        sqlx::query("CREATE INDEX idx_name ON t(name)")
            .execute(&pool)
            .await
            .expect("建索引");
        pool.close().await;
    }

    let db = conn::open(&config).await.expect("打开数据库");
    let indexes = meta::list_indexes(&db, "t").await.expect("列出索引");
    // 只应看到显式创建的 idx_name，不应看到 sqlite_autoindex_*
    assert!(
        indexes.iter().any(|i| i.name == "idx_name"),
        "应包含 idx_name，实际: {:?}",
        indexes.iter().map(|i| &i.name).collect::<Vec<_>>()
    );
    assert!(
        !indexes.iter().any(|i| i.name.starts_with("sqlite_autoindex")),
        "不应包含自动索引"
    );

    db.close().await;
    let _ = std::fs::remove_file(&path);
}
