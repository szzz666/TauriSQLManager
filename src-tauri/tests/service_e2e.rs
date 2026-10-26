//! 端到端服务层测试
//! 用临时数据目录 + 真实 SQLite 文件，覆盖「连接 → 表结构 → 数据 CRUD → SQL → 导出」全链路

// 这些测试会改进程级环境变量与全局 store，必须串行执行，
// 唯一的办法就是让 SEQ 这个 MutexGuard 跨越 await 一直持有到测试结束。
#![allow(clippy::await_holding_lock)]

use serde_json::{json, Value};
use std::path::PathBuf;
use tauri_sql_manager_lib::db::PoolManager;
use tauri_sql_manager_lib::model::{
    ColumnDefinition, ConnectionInput, DbType, TableDataParams,
};
use tauri_sql_manager_lib::service;
use tauri_sql_manager_lib::store;

/// 建立隔离的测试环境：独立数据目录 + 已建表的 SQLite 文件 + 一条连接配置
/// 串行化：这些测试会修改进程级环境变量与全局 store，不能并行
static SEQ: std::sync::Mutex<()> = std::sync::Mutex::new(());

async fn setup() -> (PoolManager, String, PathBuf, std::sync::MutexGuard<'static, ()>) {
    let guard = SEQ.lock().unwrap_or_else(|e| e.into_inner());
    let mut tmp = std::env::temp_dir();
    let salt = format!("tsm_e2e_{}_{}", std::process::id(), rand_suffix());
    tmp.push(salt);
    std::fs::create_dir_all(&tmp).expect("创建临时目录");

    // 通过环境变量把配置目录指向临时目录
    std::env::set_var("TSM_DATA_DIR", &tmp);
    store::reload_for_test();

    let db_path = tmp.join("demo.db");
    let url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = sqlx::sqlite::SqlitePool::connect(&url).await.expect("创建数据库");
    sqlx::query(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            email TEXT,
            age INTEGER DEFAULT 0
        )",
    )
    .execute(&pool)
    .await
    .expect("建表");
    for i in 1..=25 {
        sqlx::query("INSERT INTO users (name, email, age) VALUES (?, ?, ?)")
            .bind(format!("user{i}"))
            .bind(format!("u{i}@example.com"))
            .bind(18 + i)
            .execute(&pool)
            .await
            .expect("插入");
    }
    pool.close().await;

    let created = service::create_connection(&ConnectionInput {
        name: "测试连接".to_string(),
        db_type: DbType::Sqlite,
        host: String::new(),
        port: 0,
        database: db_path.to_string_lossy().to_string(),
        username: String::new(),
        password: Some(String::new()),
        auto_reconnect: Some(true),
    })
    .expect("创建连接");

    (PoolManager::new(), created.id, tmp, guard)
}

fn rand_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[tokio::test]
async fn full_flow_connection_table_data_sql_export() {
    let (pm, conn_id, dir, _guard) = setup().await;

    // ---------- 1. 连接被持久化且密码脱敏 ----------
    let list = service::list_connections();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "测试连接");
    // 配置文件确实写到了临时目录
    assert!(dir.join("connections.json").exists());

    // ---------- 2. 测试连接 ----------
    let test = service::test_connection(&ConnectionInput {
        name: "tmp".into(),
        db_type: DbType::Sqlite,
        host: String::new(),
        port: 0,
        database: dir.join("demo.db").to_string_lossy().to_string(),
        username: String::new(),
        password: None,
        auto_reconnect: None,
    })
    .await;
    assert!(test.success, "连接应成功: {}", test.message);
    assert!(test.version.unwrap_or_default().contains("SQLite"));

    // ---------- 3. 列出表 ----------
    let tables = service::list_tables(&pm, &conn_id).await.expect("列表");
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "users");

    // ---------- 4. 表结构 ----------
    let info = service::get_table_info(&pm, &conn_id, "users").await.expect("结构");
    assert_eq!(info.columns.len(), 4);
    assert_eq!(info.primary_keys, vec!["id".to_string()]);
    assert_eq!(info.row_count, 25);
    let name_col = info.columns.iter().find(|c| c.name == "name").unwrap();
    assert!(!name_col.nullable, "name 应为 NOT NULL");
    let email_col = info.columns.iter().find(|c| c.name == "email").unwrap();
    assert!(email_col.nullable, "email 应可为空");

    // ---------- 5. 分页读取 ----------
    let page1 = service::query_table_data(
        &pm,
        &conn_id,
        "users",
        &TableDataParams {
            page: 1,
            page_size: 10,
            sort_column: None,
            sort_order: None,
            r#where: None,
        },
    )
    .await
    .expect("分页");
    assert_eq!(page1.rows.len(), 10);
    assert_eq!(page1.total, 25);
    assert_eq!(page1.total_pages, 3);
    assert_eq!(page1.columns, vec!["id", "name", "email", "age"]);

    let page3 = service::query_table_data(
        &pm,
        &conn_id,
        "users",
        &TableDataParams {
            page: 3,
            page_size: 10,
            sort_column: None,
            sort_order: None,
            r#where: None,
        },
    )
    .await
    .expect("第 3 页");
    assert_eq!(page3.rows.len(), 5, "最后一页应只剩 5 行");

    // ---------- 6. 条件过滤 ----------
    let filtered = service::query_table_data(
        &pm,
        &conn_id,
        "users",
        &TableDataParams {
            page: 1,
            page_size: 50,
            sort_column: None,
            sort_order: None,
            r#where: Some("age > 30".to_string()),
        },
    )
    .await
    .expect("条件查询");
    assert_eq!(filtered.total, 13, "age > 30 共 13 行");
    for row in &filtered.rows {
        assert!(row["age"].as_i64().unwrap() > 30);
    }

    // ---------- 7. 插入 ----------
    let mut new_row = serde_json::Map::new();
    new_row.insert("name".to_string(), json!("新用户"));
    new_row.insert("email".to_string(), json!("new@example.com"));
    new_row.insert("age".to_string(), json!(99));
    let inserted = service::insert_row(&pm, &conn_id, "users", &new_row)
        .await
        .expect("插入");
    assert_eq!(inserted.affected, 1);

    // ---------- 8. 按主键更新 ----------
    let mut values = serde_json::Map::new();
    values.insert("age".to_string(), json!(100));
    let mut pk = serde_json::Map::new();
    pk.insert("id".to_string(), json!(26));
    let updated = service::update_row_by_pk(&pm, &conn_id, "users", &values, &pk)
        .await
        .expect("更新");
    assert_eq!(updated.affected, 1);

    // 校验更新生效，且未把主键写进去
    let after = service::query_table_data(
        &pm,
        &conn_id,
        "users",
        &TableDataParams {
            page: 1,
            page_size: 10,
            sort_column: Some("id".to_string()),
            sort_order: Some("DESC".to_string()),
            r#where: None,
        },
    )
    .await
    .expect("复查");
    assert_eq!(after.rows[0]["age"], json!(100));
    assert_eq!(after.rows[0]["id"], json!(26));

    // ---------- 9. SQL 执行 ----------
    let q = service::execute_sql(&pm, &conn_id, "SELECT COUNT(*) AS c FROM users")
        .await
        .expect("执行查询");
    assert!(q.query);
    assert_eq!(q.rows[0][0], json!(26));
    assert_eq!(q.columns, vec!["c"]);

    let upd = service::execute_sql(&pm, &conn_id, "UPDATE users SET age = age + 1")
        .await
        .expect("执行更新");
    assert!(!upd.query);
    assert_eq!(upd.affected_rows, 26);

    // ---------- 10. 批量执行 ----------
    let batch = service::execute_batch(
        &pm,
        &conn_id,
        "INSERT INTO users (name) VALUES ('a'); INSERT INTO users (name) VALUES ('b');",
    )
    .await
    .expect("批量执行");
    assert_eq!(batch.len(), 2);

    // ---------- 11. 导出 ----------
    let csv = service::export_query_content(&pm, &conn_id, "SELECT id, name FROM users WHERE id <= 2", "csv", None)
        .await
        .expect("导出 CSV");
    assert!(csv.starts_with('\u{feff}'), "CSV 应带 BOM");
    assert!(csv.contains("id,name"));
    assert!(csv.contains("user1"));

    let json_out = service::export_query_content(&pm, &conn_id, "SELECT id FROM users WHERE id = 1", "json", None)
        .await
        .expect("导出 JSON");
    let parsed: Value = serde_json::from_str(&json_out).expect("JSON 可解析");
    assert_eq!(parsed[0]["id"], json!(1));

    let sql_out = service::export_query_content(&pm, &conn_id, "SELECT id FROM users WHERE id = 1", "sql", Some("users"))
        .await
        .expect("导出 SQL");
    assert!(sql_out.contains("INSERT INTO \"users\""));

    // ---------- 12. 按主键删除 ----------
    let deleted = service::delete_row_by_pk(&pm, &conn_id, "users", &pk)
        .await
        .expect("删除");
    assert_eq!(deleted.affected, 1);

    let after_del = service::get_table_info(&pm, &conn_id, "users").await.expect("结构");
    assert_eq!(after_del.row_count, 27);
}

#[tokio::test]
async fn create_and_alter_table() {
    let (pm, conn_id, _dir, _guard) = setup().await;

    let columns = vec![
        ColumnDefinition {
            name: "id".into(),
            col_type: "INTEGER".into(),
            length: 0,
            scale: 0,
            nullable: false,
            auto_increment: true,
            primary_key: true,
            unique: false,
            default_value: None,
            comment: None,
            action: None,
            old_name: None,
        },
        ColumnDefinition {
            name: "title".into(),
            col_type: "TEXT".into(),
            length: 0,
            scale: 0,
            nullable: false,
            auto_increment: false,
            primary_key: false,
            unique: false,
            default_value: None,
            comment: None,
            action: None,
            old_name: None,
        },
    ];

    // 预览 SQL 不应执行
    let preview = service::preview_create_sql(&pm, &conn_id, "notes", &columns, None).expect("预览");
    assert!(preview.starts_with("CREATE TABLE"));

    service::create_table(&pm, &conn_id, "notes", columns.clone(), None)
        .await
        .expect("建表");

    let info = service::get_table_info(&pm, &conn_id, "notes").await.expect("结构");
    assert_eq!(info.columns.len(), 2);
    assert_eq!(info.primary_keys, vec!["id".to_string()]);

    // 加列
    service::add_column(
        &pm,
        &conn_id,
        "notes",
        &ColumnDefinition {
            name: "created_at".into(),
            col_type: "DATETIME".into(),
            nullable: true,
            ..Default::default()
        },
    )
    .await
    .expect("加列");
    let info = service::get_table_info(&pm, &conn_id, "notes").await.expect("结构");
    assert_eq!(info.columns.len(), 3);

    // 删列
    service::drop_column(&pm, &conn_id, "notes", "created_at")
        .await
        .expect("删列");
    let info = service::get_table_info(&pm, &conn_id, "notes").await.expect("结构");
    assert_eq!(info.columns.len(), 2);

    // 重命名表
    service::rename_table(&pm, &conn_id, "notes", "notes2")
        .await
        .expect("重命名");
    let tables = service::list_tables(&pm, &conn_id).await.expect("列表");
    assert!(tables.iter().any(|t| t.name == "notes2"));

    // 清空表
    service::truncate_table(&pm, &conn_id, "notes2").await.expect("清空");

    // 删表
    service::drop_table(&pm, &conn_id, "notes2").await.expect("删表");
    let tables = service::list_tables(&pm, &conn_id).await.expect("列表");
    assert!(!tables.iter().any(|t| t.name == "notes2"));
}

#[tokio::test]
async fn import_csv_and_json() {
    let (pm, conn_id, _dir, _guard) = setup().await;

    // CSV 导入（ignore 模式）
    let csv = "name,email,age\n导入A,a@example.com,20\n导入B,b@example.com,30\n";
    let result = service::import_data(&pm, &conn_id, "users", "csv", csv, "ignore")
        .await
        .expect("CSV 导入");
    assert_eq!(result.total_rows, 2);
    assert_eq!(result.affected_rows, 2);
    assert_eq!(result.format, "csv");

    // JSON 导入
    let json_rows = r#"[{"name":"导入C","email":"c@example.com","age":40}]"#;
    let result = service::import_data(&pm, &conn_id, "users", "json", json_rows, "insert")
        .await
        .expect("JSON 导入");
    assert_eq!(result.affected_rows, 1);

    let info = service::get_table_info(&pm, &conn_id, "users").await.expect("结构");
    assert_eq!(info.row_count, 28);
}

#[tokio::test]
async fn rejects_invalid_connections() {
    let _guard = SEQ.lock().unwrap_or_else(|e| e.into_inner());
    let mut dir = std::env::temp_dir();
    dir.push(format!("tsm_e2e_invalid_{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok();
    std::env::set_var("TSM_DATA_DIR", &dir);
    store::reload_for_test();

    // 名称不能为空
    let err = service::create_connection(&ConnectionInput {
        name: "   ".into(),
        db_type: DbType::Sqlite,
        host: String::new(),
        port: 0,
        database: "/tmp/x.db".into(),
        username: String::new(),
        password: None,
        auto_reconnect: None,
    });
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("名称"));

    // SQLite 必须给出文件
    let err = service::create_connection(&ConnectionInput {
        name: "ok".into(),
        db_type: DbType::Sqlite,
        host: String::new(),
        port: 0,
        database: String::new(),
        username: String::new(),
        password: None,
        auto_reconnect: None,
    });
    assert!(err.is_err());

    // 远程数据库必须有主机与库名
    let err = service::create_connection(&ConnectionInput {
        name: "mysql".into(),
        db_type: DbType::Mysql,
        host: String::new(),
        port: 3306,
        database: "db".into(),
        username: "root".into(),
        password: None,
        auto_reconnect: None,
    });
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("主机"));
}