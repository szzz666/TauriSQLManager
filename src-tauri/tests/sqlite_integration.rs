//! SQLite 集成测试：验证方言、元数据查询与数据操作的端到端行为
//! 通过 sqlx 直接连接临时数据库，覆盖 service 层依赖的核心链路

use sqlx::sqlite::SqlitePool;
use sqlx::{Column, Row};

async fn temp_pool() -> SqlitePool {
    let path = std::env::temp_dir().join(format!("tsm_test_{}.db", uuid::Uuid::new_v4()));
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let pool = SqlitePool::connect(&url).await.expect("创建数据库");
    sqlx::query(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            email TEXT,
            age INTEGER DEFAULT 0,
            created_at DATETIME
        )",
    )
    .execute(&pool)
    .await
    .expect("建表");
    sqlx::query("CREATE INDEX idx_users_name ON users(name)")
        .execute(&pool)
        .await
        .expect("建索引");
    pool
}

#[tokio::test]
async fn creates_and_reads_rows() {
    let pool = temp_pool().await;

    sqlx::query("INSERT INTO users (name, email, age) VALUES (?, ?, ?)")
        .bind("Alice")
        .bind("alice@example.com")
        .bind(30i64)
        .execute(&pool)
        .await
        .expect("插入");

    let row = sqlx::query("SELECT id, name, age FROM users WHERE name = ?")
        .bind("Alice")
        .fetch_one(&pool)
        .await
        .expect("查询");

    assert_eq!(row.get::<i64, _>("id"), 1);
    assert_eq!(row.get::<String, _>("name"), "Alice");
    assert_eq!(row.get::<i64, _>("age"), 30);
}

#[tokio::test]
async fn lists_tables_from_master() {
    let pool = temp_pool().await;
    let sql = "SELECT name, type FROM sqlite_master WHERE type IN ('table','view') \
               AND name NOT LIKE 'sqlite_%' ORDER BY name";
    let rows = sqlx::query(sql).fetch_all(&pool).await.expect("读取 sqlite_master");
    let names: Vec<String> = rows.iter().map(|r| r.get::<String, _>("name")).collect();
    assert_eq!(names, vec!["users".to_string()]);
}

#[tokio::test]
async fn reads_column_metadata() {
    let pool = temp_pool().await;
    let rows = sqlx::query("SELECT name, type, \"notnull\", dflt_value, pk FROM pragma_table_info(?)")
        .bind("users")
        .fetch_all(&pool)
        .await
        .expect("读取表结构");

    assert_eq!(rows.len(), 5);
    let id_row = &rows[0];
    assert_eq!(id_row.get::<String, _>("name"), "id");
    assert_eq!(id_row.get::<i64, _>("pk"), 1);

    let email_row = &rows[2];
    assert_eq!(email_row.get::<String, _>("name"), "email");
    // email 可为空 -> notnull = 0
    assert_eq!(email_row.get::<i64, _>("notnull"), 0);
}

#[tokio::test]
async fn paging_is_stable() {
    let pool = temp_pool().await;
    for i in 0..25 {
        sqlx::query("INSERT INTO users (name, age) VALUES (?, ?)")
            .bind(format!("user{i}"))
            .bind(i)
            .execute(&pool)
            .await
            .expect("插入");
    }

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .expect("计数");
    assert_eq!(total, 25);

    // 第二页应当返回 10 条
    let rows = sqlx::query("SELECT * FROM \"users\" ORDER BY id LIMIT 10 OFFSET 10")
        .fetch_all(&pool)
        .await
        .expect("分页查询");
    assert_eq!(rows.len(), 10);
    assert_eq!(rows[0].get::<String, _>("name"), "user10");
}

#[tokio::test]
async fn column_types_are_reported() {
    let pool = temp_pool().await;
    let rows = sqlx::query("SELECT id, name FROM users").fetch_all(&pool).await.expect("查询");
    // sqlite 空表时也能拿到列信息需要查询元数据，这里插入一行确保有返回
    if rows.is_empty() {
        sqlx::query("INSERT INTO users (name) VALUES ('x')")
            .execute(&pool)
            .await
            .expect("插入");
        let rows = sqlx::query("SELECT id, name FROM users").fetch_all(&pool).await.expect("查询");
        let cols = rows[0].columns();
        assert_eq!(cols.len(), 2);
        assert!(!cols[0].name().is_empty());
    }
}

#[tokio::test]
async fn update_and_delete_by_primary_key() {
    let pool = temp_pool().await;
    sqlx::query("INSERT INTO users (name, age) VALUES ('Bob', 40)")
        .execute(&pool)
        .await
        .expect("插入");

    let affected = sqlx::query("UPDATE users SET age = ? WHERE id = ?")
        .bind(41i64)
        .bind(1i64)
        .execute(&pool)
        .await
        .expect("更新")
        .rows_affected();
    assert_eq!(affected, 1);

    let age: i64 = sqlx::query_scalar("SELECT age FROM users WHERE id = 1")
        .fetch_one(&pool)
        .await
        .expect("读取");
    assert_eq!(age, 41);

    let deleted = sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(1i64)
        .execute(&pool)
        .await
        .expect("删除")
        .rows_affected();
    assert_eq!(deleted, 1);
}

#[tokio::test]
async fn inserts_in_replace_and_ignore_modes() {
    let pool = temp_pool().await;
    sqlx::query("INSERT INTO users (id, name) VALUES (1, 'a')")
        .execute(&pool)
        .await
        .expect("插入");

    // REPLACE 覆盖
    sqlx::query("INSERT OR REPLACE INTO users (id, name) VALUES (1, 'b')")
        .execute(&pool)
        .await
        .expect("replace");
    let name: String = sqlx::query_scalar("SELECT name FROM users WHERE id = 1")
        .fetch_one(&pool)
        .await
        .expect("读取");
    assert_eq!(name, "b");

    // IGNORE 跳过冲突
    sqlx::query("INSERT OR IGNORE INTO users (id, name) VALUES (1, 'c')")
        .execute(&pool)
        .await
        .expect("ignore");
    let name: String = sqlx::query_scalar("SELECT name FROM users WHERE id = 1")
        .fetch_one(&pool)
        .await
        .expect("读取");
    assert_eq!(name, "b");
}
