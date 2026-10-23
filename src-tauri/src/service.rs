//! 业务服务层
//! 对应原项目 service/ConnectionService、TableService、DataService、SqlService

use crate::db::conn::{self, Db};
use crate::db::exec::{self, Bind, RawResult};
use crate::db::meta;
use crate::db::Dialect;
use crate::model::*;
use crate::store;
use serde_json::{Map, Value as Json};
use std::time::Instant;

// ==================== 连接管理 ====================

/// 列出所有连接（密码脱敏）
pub fn list_connections() -> Vec<ConnectionConfig> {
    store::connections()
        .into_iter()
        .map(|c| c.masked())
        .collect()
}

/// 取单个连接（密码脱敏）
pub fn get_connection(id: &str) -> Result<ConnectionConfig, String> {
    store::find_connection(id)
        .map(|c| c.masked())
        .ok_or_else(|| format!("连接不存在: {id}"))
}

/// 把输入转换为配置；更新时保留未提供的密码
pub fn build_config(input: &ConnectionInput, existing: Option<&ConnectionConfig>) -> ConnectionConfig {
    let now = chrono::Utc::now().timestamp_millis();
    let (id, created_at) = match existing {
        Some(e) => (e.id.clone(), e.created_at),
        None => (uuid::Uuid::new_v4().simple().to_string(), now),
    };
    // 密码留空表示沿用旧密码
    let password = match &input.password {
        Some(p) if !p.is_empty() => p.clone(),
        _ => existing.map(|e| e.password.clone()).unwrap_or_default(),
    };
    let port = if input.port == 0 {
        input.db_type.default_port()
    } else {
        input.port
    };
    ConnectionConfig {
        id,
        name: input.name.trim().to_string(),
        db_type: input.db_type,
        host: input.host.trim().to_string(),
        port,
        database: input.database.trim().to_string(),
        username: input.username.clone(),
        password,
        auto_reconnect: input.auto_reconnect.unwrap_or(true),
        created_at,
        updated_at: now,
    }
}

/// 校验连接配置
pub fn validate(config: &ConnectionConfig) -> Result<(), String> {
    if config.name.trim().is_empty() {
        return Err("连接名称不能为空".to_string());
    }
    match config.db_type {
        DbType::Sqlite => {
            if config.database.trim().is_empty() {
                return Err("请选择 SQLite 数据库文件".to_string());
            }
        }
        _ => {
            if config.host.trim().is_empty() {
                return Err("主机地址不能为空".to_string());
            }
            if config.database.trim().is_empty() {
                return Err("数据库名不能为空".to_string());
            }
        }
    }
    Ok(())
}

/// 新增连接
pub fn create_connection(input: &ConnectionInput) -> Result<ConnectionConfig, String> {
    let config = build_config(input, None);
    validate(&config)?;
    {
        let store = store::init();
        let mut guard = store.lock().map_err(|_| "配置存储锁定失败".to_string())?;
        guard.connections.push(config.clone());
        store::save_connections(&guard.connections)?;
    }
    Ok(config.masked())
}

/// 更新连接
pub fn update_connection(id: &str, input: &ConnectionInput) -> Result<ConnectionConfig, String> {
    let existing = store::find_connection(id).ok_or_else(|| format!("连接不存在: {id}"))?;
    let config = build_config(input, Some(&existing));
    validate(&config)?;
    {
        let store = store::init();
        let mut guard = store.lock().map_err(|_| "配置存储锁定失败".to_string())?;
        match guard.connections.iter_mut().find(|c| c.id == id) {
            Some(slot) => *slot = config.clone(),
            None => return Err(format!("连接不存在: {id}")),
        }
        store::save_connections(&guard.connections)?;
    }
    Ok(config.masked())
}

/// 删除连接
pub fn delete_connection(id: &str) -> Result<(), String> {
    {
        let store = store::init();
        let mut guard = store.lock().map_err(|_| "配置存储锁定失败".to_string())?;
        let before = guard.connections.len();
        guard.connections.retain(|c| c.id != id);
        if guard.connections.len() == before {
            return Err(format!("连接不存在: {id}"));
        }
        store::save_connections(&guard.connections)?;
    }
    Ok(())
}

/// 测试连接（未保存的配置）
pub async fn test_connection(input: &ConnectionInput) -> TestResult {
    let config = build_config(input, None);
    run_test(&config).await
}

/// 测试已保存的连接
pub async fn test_saved_connection(id: &str) -> Result<TestResult, String> {
    let config = store::find_connection(id).ok_or_else(|| format!("连接不存在: {id}"))?;
    Ok(run_test(&config).await)
}

async fn run_test(config: &ConnectionConfig) -> TestResult {
    let start = Instant::now();
    let result = conn::test_connect(config).await;
    let elapsed = start.elapsed().as_millis() as i64;
    match result {
        Ok(version) => TestResult {
            success: true,
            message: "连接成功".to_string(),
            elapsed,
            version: Some(version),
        },
        Err(e) => TestResult {
            success: false,
            message: e,
            elapsed,
            version: None,
        },
    }
}

// ==================== 表结构管理 ====================

/// 打开连接（取缓存或新建）
pub async fn open_db(pm: &crate::db::PoolManager, conn_id: &str) -> Result<Db, String> {
    let config = store::find_connection(conn_id).ok_or_else(|| format!("连接不存在: {conn_id}"))?;
    pm.get(&config).await
}

pub async fn list_tables(pm: &crate::db::PoolManager, conn_id: &str) -> Result<Vec<TableInfo>, String> {
    let db = open_db(pm, conn_id).await?;
    meta::list_tables(&db).await
}

pub async fn get_table_info(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    table: &str,
) -> Result<TableInfo, String> {
    let db = open_db(pm, conn_id).await?;
    meta::get_table_info(&db, table).await
}

pub async fn list_indexes(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    table: &str,
) -> Result<Vec<IndexInfo>, String> {
    let db = open_db(pm, conn_id).await?;
    meta::list_indexes(&db, table).await
}

/// 生成建表 SQL（供预览）
pub fn preview_create_sql(
    _pm: &crate::db::PoolManager,
    conn_id: &str,
    name: &str,
    columns: &[ColumnDefinition],
    comment: Option<&str>,
) -> Result<String, String> {
    let config = store::find_connection(conn_id).ok_or_else(|| format!("连接不存在: {conn_id}"))?;
    let dialect = Dialect::of(config.db_type);
    if columns.is_empty() {
        return Err("至少需要定义一列".to_string());
    }
    Ok(dialect.build_create_table_sql(name, columns, comment))
}

pub async fn create_table(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    name: &str,
    columns: Vec<ColumnDefinition>,
    comment: Option<String>,
) -> Result<(), String> {
    if columns.is_empty() {
        return Err("至少需要定义一列".to_string());
    }
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());
    let sql = dialect.build_create_table_sql(name, &columns, comment.as_deref());
    exec::execute_update(&db, &sql, &[]).await?;
    Ok(())
}

pub async fn drop_table(pm: &crate::db::PoolManager, conn_id: &str, table: &str) -> Result<(), String> {
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());
    exec::execute_update(&db, &dialect.build_drop_table_sql(table), &[]).await?;
    Ok(())
}

pub async fn rename_table(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    table: &str,
    new_name: &str,
) -> Result<(), String> {
    if new_name.trim().is_empty() {
        return Err("新表名不能为空".to_string());
    }
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());
    exec::execute_update(&db, &dialect.build_rename_table_sql(table, new_name), &[]).await?;
    Ok(())
}

pub async fn truncate_table(pm: &crate::db::PoolManager, conn_id: &str, table: &str) -> Result<(), String> {
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());
    exec::execute_update(&db, &dialect.build_truncate_table_sql(table), &[]).await?;
    Ok(())
}

pub async fn add_column(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    table: &str,
    col: &ColumnDefinition,
) -> Result<(), String> {
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());
    exec::execute_update(&db, &dialect.build_add_column_sql(table, col), &[]).await?;
    Ok(())
}

pub async fn modify_column(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    table: &str,
    col: &ColumnDefinition,
) -> Result<(), String> {
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());
    let sql = dialect.build_modify_column_sql(table, col)?;
    // PostgreSQL 返回的是用分号分隔的多条语句
    for stmt in sql.split(';') {
        let stmt = stmt.trim();
        if !stmt.is_empty() {
            exec::execute_update(&db, stmt, &[]).await?;
        }
    }
    Ok(())
}

pub async fn drop_column(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    table: &str,
    column: &str,
) -> Result<(), String> {
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());
    exec::execute_update(&db, &dialect.build_drop_column_sql(table, column), &[]).await?;
    Ok(())
}

/// 批量变更表结构，返回执行过的 SQL
pub async fn alter_table(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    table: &str,
    changes: Vec<ColumnDefinition>,
) -> Result<Vec<String>, String> {
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());
    let mut executed = Vec::new();
    for col in &changes {
        let action = col.action.as_deref().unwrap_or("ADD").to_uppercase();
        let sql = match action.as_str() {
            "ADD" => dialect.build_add_column_sql(table, col),
            "MODIFY" | "ALTER" => dialect.build_modify_column_sql(table, col)?,
            "DROP" => dialect.build_drop_column_sql(table, &col.name),
            "RENAME" => dialect.build_rename_column_sql(
                table,
                col.old_name.as_deref().unwrap_or(&col.name),
                &col.name,
            ),
            other => return Err(format!("不支持的操作: {other}")),
        };
        for stmt in sql.split(';') {
            let stmt = stmt.trim();
            if !stmt.is_empty() {
                exec::execute_update(&db, stmt, &[]).await?;
            }
        }
        executed.push(sql);
    }
    Ok(executed)
}

// ==================== 数据 CRUD ====================

/// 分页查询表数据
pub async fn query_table_data(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    table: &str,
    params: &TableDataParams,
) -> Result<TableData, String> {
    let settings = store::settings();
    let max_rows = settings.max_query_rows.max(1) as usize;
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());

    let page = params.page.max(1);
    let page_size = params.page_size.clamp(1, max_rows as i64);

    // 条件查询通过子句拼接，超出分页时同时统计总数
    let where_clause = params
        .r#where
        .as_ref()
        .map(|w| w.trim().to_string())
        .filter(|w| !w.is_empty());

    let base = format!(
        "SELECT * FROM {}{}",
        dialect.quote_identifier(table),
        where_clause
            .as_ref()
            .map(|w| format!(" WHERE {w}"))
            .unwrap_or_default()
    );

    // 排序列做白名单校验：必须存在于表中，避免注入
    let table_info = meta::get_table_info(&db, table).await?;
    let mut ordered = base.clone();
    if let Some(sort_col) = params.sort_column.as_ref().filter(|c| !c.trim().is_empty()) {
        if table_info.columns.iter().any(|c| &c.name == sort_col) {
            let dir = match params.sort_order.as_deref() {
                Some(o) if o.eq_ignore_ascii_case("asc") => "ASC",
                _ => "DESC",
            };
            ordered = format!("{base} ORDER BY {} {dir}", dialect.quote_identifier(sort_col));
        }
    }

    let offset = (page - 1) * page_size;
    let sql = dialect.build_paged_sql(&ordered, offset, page_size);
    let raw = exec::fetch_all(&db, &sql, &[], page_size as usize).await?;

    // 统计总数
    let count_sql = format!(
        "SELECT COUNT(*) FROM {}{}",
        dialect.quote_identifier(table),
        where_clause
            .as_ref()
            .map(|w| format!(" WHERE {w}"))
            .unwrap_or_default()
    );
    let total = exec::fetch_all(&db, &count_sql, &[], 1)
        .await
        .ok()
        .and_then(|r| r.rows.first().and_then(|row| row.first().cloned()))
        .and_then(|v| match v {
            Json::Number(num) => num.as_i64(),
            Json::String(s) => s.parse().ok(),
            _ => None,
        })
        .unwrap_or(0);

    Ok(raw_to_table_data(raw, page, page_size, total))
}

fn raw_to_table_data(raw: RawResult, page: i64, page_size: i64, total: i64) -> TableData {
    let mut rows = Vec::with_capacity(raw.rows.len());
    for values in raw.rows {
        let mut obj = Map::new();
        for (i, col) in raw.columns.iter().enumerate() {
            obj.insert(col.clone(), values.get(i).cloned().unwrap_or(Json::Null));
        }
        rows.push(obj);
    }
    let total_pages = if page_size > 0 {
        (total as f64 / page_size as f64).ceil() as i64
    } else {
        1
    };
    TableData {
        rows,
        columns: raw.columns,
        column_types: raw.column_types,
        total,
        page,
        page_size,
        total_pages: total_pages.max(1),
    }
}

/// 插入一行
pub async fn insert_row(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    table: &str,
    row: &Map<String, Json>,
) -> Result<AffectedResult, String> {
    if row.is_empty() {
        return Err("插入数据不能为空".to_string());
    }
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());
    let columns: Vec<String> = row.keys().cloned().collect();
    let binds: Vec<Bind> = row.values().map(Bind::from_json).collect();
    let sql = dialect.build_insert_sql(table, &columns, "insert", &[]);
    let affected = exec::execute_update(&db, &sql, &binds).await?;
    Ok(AffectedResult { affected })
}

/// 按主键更新
pub async fn update_row_by_pk(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    table: &str,
    values: &Map<String, Json>,
    pk: &Map<String, Json>,
) -> Result<AffectedResult, String> {
    if values.is_empty() {
        return Err("更新数据不能为空".to_string());
    }
    if pk.is_empty() {
        return Err("主键值不能为空".to_string());
    }
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());

    // 过滤掉主键自身
    let cols: Vec<String> = values
        .keys()
        .filter(|k| !pk.contains_key(*k))
        .cloned()
        .collect();
    if cols.is_empty() {
        return Err("没有可更新的非主键字段".to_string());
    }
    let mut binds: Vec<Bind> = cols
        .iter()
        .map(|c| Bind::from_json(&values[c]))
        .collect();

    let where_parts: Vec<String> = pk
        .keys()
        .map(|k| format!("{} = ?", dialect.quote_identifier(k)))
        .collect();
    let where_clause = where_parts.join(" AND ");
    for v in pk.values() {
        binds.push(Bind::from_json(v));
    }

    let sql = dialect.build_update_sql(table, &cols, Some(&where_clause));
    let affected = exec::execute_update(&db, &sql, &binds).await?;
    Ok(AffectedResult { affected })
}

/// 按主键删除
pub async fn delete_row_by_pk(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    table: &str,
    pk: &Map<String, Json>,
) -> Result<AffectedResult, String> {
    if pk.is_empty() {
        return Err("主键值不能为空".to_string());
    }
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());
    let where_clause = pk
        .keys()
        .map(|k| format!("{} = ?", dialect.quote_identifier(k)))
        .collect::<Vec<_>>()
        .join(" AND ");
    let binds: Vec<Bind> = pk.values().map(Bind::from_json).collect();
    let sql = dialect.build_delete_sql(table, Some(&where_clause));
    let affected = exec::execute_update(&db, &sql, &binds).await?;
    Ok(AffectedResult { affected })
}

/// 导入数据（CSV / JSON）
pub async fn import_data(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    table: &str,
    format: &str,
    content: &str,
    mode: &str,
) -> Result<ImportResult, String> {
    if content.trim().is_empty() {
        return Err("导入数据不能为空".to_string());
    }
    let rows = match format.to_lowercase().as_str() {
        "json" => parse_json_rows(content)?,
        _ => parse_csv_rows(content)?,
    };
    if rows.is_empty() {
        return Err("解析后没有有效数据行".to_string());
    }
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());
    let columns: Vec<String> = rows[0].keys().cloned().collect();

    // PostgreSQL 的 ON CONFLICT 需要明确的冲突键；SQL Server 无等价语法，
    // 因此按行执行普通 INSERT，冲突时按模式决定跳过还是报错
    let pk_cols = meta::get_table_info(&db, table)
        .await
        .map(|t| t.primary_keys)
        .unwrap_or_default();
    let conflict_cols: Vec<String> = pk_cols
        .iter()
        .filter(|k| columns.iter().any(|c| c == *k))
        .cloned()
        .collect();
    let sql = dialect.build_insert_sql(table, &columns, mode, &conflict_cols);

    let mut affected = 0i64;
    let mut skipped = 0i64;
    for (idx, row) in rows.iter().enumerate() {
        let binds: Vec<Bind> = columns
            .iter()
            .map(|c| Bind::from_json(row.get(c).unwrap_or(&Json::Null)))
            .collect();
        match exec::execute_update(&db, &sql, &binds).await {
            Ok(n) => affected += n,
            Err(e) => {
                // ignore / replace 模式下跳过冲突行，insert 模式直接报错
                if mode.eq_ignore_ascii_case("ignore") || mode.eq_ignore_ascii_case("replace") {
                    skipped += 1;
                    continue;
                }
                return Err(format!("导入第 {} 行失败: {e}", idx + 1));
            }
        }
    }

    let _ = skipped;
    Ok(ImportResult {
        total_rows: rows.len() as i64,
        affected_rows: affected,
        format: format.to_string(),
        table: table.to_string(),
    })
}

fn parse_json_rows(content: &str) -> Result<Vec<Map<String, Json>>, String> {
    let value: Json = serde_json::from_str(content).map_err(|e| format!("JSON 解析失败: {e}"))?;
    let arr = value
        .as_array()
        .ok_or_else(|| "JSON 导入需要是对象数组格式".to_string())?;
    let mut rows = Vec::new();
    for item in arr {
        match item {
            Json::Object(map) => rows.push(map.clone()),
            _ => return Err("JSON 数组中的元素必须是对象".to_string()),
        }
    }
    Ok(rows)
}

fn parse_csv_rows(content: &str) -> Result<Vec<Map<String, Json>>, String> {
    // 去掉 BOM
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = content.lines().filter(|l| !l.trim().is_empty());
    let header = match lines.next() {
        Some(h) => parse_csv_line(h),
        None => return Ok(Vec::new()),
    };
    if header.is_empty() {
        return Ok(Vec::new());
    }
    let mut rows = Vec::new();
    for line in lines {
        let values = parse_csv_line(line);
        let mut map = Map::new();
        for (i, key) in header.iter().enumerate() {
            let raw = values.get(i).map(|s| s.as_str()).unwrap_or("");
            map.insert(
                key.clone(),
                if raw.is_empty() {
                    Json::Null
                } else {
                    Json::String(raw.to_string())
                },
            );
        }
        rows.push(map);
    }
    Ok(rows)
}

/// 解析单行 CSV，支持引号包裹与转义
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                current.push(c);
            }
        } else if c == '"' {
            in_quotes = true;
        } else if c == ',' {
            result.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
    }
    result.push(current.trim().to_string());
    result
}

// ==================== SQL 执行 ====================

/// 拆分多条 SQL（忽略字符串与注释中的分号）
pub fn split_sql_statements(sql_text: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut chars = sql_text.chars().peekable();

    while let Some(c) = chars.next() {
        let next = chars.peek().copied().unwrap_or('\0');

        if !in_single && !in_double {
            if !in_block_comment && c == '-' && next == '-' {
                in_line_comment = true;
            }
            if in_line_comment {
                if c == '\n' {
                    in_line_comment = false;
                    current.push(c);
                }
                continue;
            }
            if !in_line_comment && c == '/' && next == '*' {
                in_block_comment = true;
                chars.next();
                continue;
            }
            if in_block_comment {
                if c == '*' && next == '/' {
                    in_block_comment = false;
                    chars.next();
                }
                continue;
            }
        }

        if c == '\'' && !in_double {
            in_single = !in_single;
        } else if c == '"' && !in_single {
            in_double = !in_double;
        }

        if c == ';' && !in_single && !in_double {
            let s = current.trim().to_string();
            if !s.is_empty() {
                statements.push(s);
            }
            current.clear();
            continue;
        }
        current.push(c);
    }
    let last = current.trim().to_string();
    if !last.is_empty() {
        statements.push(last);
    }
    statements
}

/// 执行单条 SQL，自动区分查询与更新
pub async fn execute_sql(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    sql_text: &str,
) -> Result<QueryResult, String> {
    let sql = sql_text.trim();
    if sql.is_empty() {
        return Err("SQL 不能为空".to_string());
    }
    let db = open_db(pm, conn_id).await?;
    run_single(&db, sql).await
}

async fn run_single(db: &Db, sql: &str) -> Result<QueryResult, String> {
    let settings = store::settings();
    let max_rows = settings.max_query_rows.max(1) as usize;
    let start = Instant::now();

    if exec::returns_rows(sql) {
        let raw = exec::fetch_all(db, sql, &[], max_rows).await?;
        let elapsed = start.elapsed().as_millis() as i64;
        let returned = raw.rows.len() as i64;
        Ok(QueryResult {
            query: true,
            columns: raw.columns,
            column_types: raw.column_types,
            rows: raw.rows,
            affected_rows: 0,
            elapsed,
            sql: sql.to_string(),
            truncated: raw.truncated,
            returned_rows: returned,
            message: String::new(),
        })
    } else {
        let affected = exec::execute_update(db, sql, &[]).await?;
        let elapsed = start.elapsed().as_millis() as i64;
        Ok(QueryResult {
            query: false,
            affected_rows: affected,
            elapsed,
            sql: sql.to_string(),
            message: format!("影响行数: {affected}"),
            ..Default::default()
        })
    }
}

/// 批量执行多条 SQL（分号分隔）
pub async fn execute_batch(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    sql_text: &str,
) -> Result<Vec<QueryResult>, String> {
    let statements = split_sql_statements(sql_text);
    if statements.is_empty() {
        return Err("SQL 不能为空".to_string());
    }
    let db = open_db(pm, conn_id).await?;
    let mut results = Vec::new();
    for stmt in statements {
        let trimmed = exec::strip_leading_comments(&stmt).trim();
        if trimmed.is_empty() {
            continue;
        }
        match run_single(&db, &stmt).await {
            Ok(r) => results.push(r),
            Err(e) => return Err(format!("执行失败: {trimmed}\n错误: {e}")),
        }
    }
    Ok(results)
}

/// 分页执行查询
pub async fn execute_query_paged(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    sql_text: &str,
    offset: i64,
    limit: i64,
) -> Result<QueryResult, String> {
    let settings = store::settings();
    let max_rows = settings.max_query_rows.max(1);
    let limit = if limit <= 0 { 100 } else { limit.min(max_rows) };
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());
    let paged = dialect.build_paged_sql(sql_text, offset, limit);
    let start = Instant::now();
    let raw = exec::fetch_all(&db, &paged, &[], limit as usize).await?;
    let elapsed = start.elapsed().as_millis() as i64;
    let returned = raw.rows.len() as i64;
    Ok(QueryResult {
        query: true,
        columns: raw.columns,
        column_types: raw.column_types,
        rows: raw.rows,
        elapsed,
        sql: paged,
        truncated: raw.truncated,
        returned_rows: returned,
        ..Default::default()
    })
}

/// 导出查询结果内容
pub async fn export_query_content(
    pm: &crate::db::PoolManager,
    conn_id: &str,
    sql_text: &str,
    format: &str,
    table_name: Option<&str>,
) -> Result<String, String> {
    let db = open_db(pm, conn_id).await?;
    let dialect = Dialect::of(db.db_type());
    let raw = exec::fetch_all(&db, sql_text, &[], 0).await?;
    Ok(export_raw(&raw, format, table_name, &dialect))
}

/// 按格式序列化结果集
pub fn export_raw(raw: &RawResult, format: &str, table_name: Option<&str>, dialect: &Dialect) -> String {
    match format.to_lowercase().as_str() {
        "json" => export_json(raw),
        "sql" => export_sql(raw, table_name.unwrap_or("export_table"), dialect),
        _ => export_csv(raw),
    }
}

fn export_csv(raw: &RawResult) -> String {
    let mut out = String::new();
    // BOM 头，便于 Excel 正确识别 UTF-8
    out.push('\u{feff}');
    out.push_str(
        &raw.columns
            .iter()
            .map(|c| csv_escape(c))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push_str("\r\n");
    for row in &raw.rows {
        let values: Vec<String> = row.iter().map(json_to_csv).collect();
        out.push_str(&values.join(","));
        out.push_str("\r\n");
    }
    out
}

fn json_to_csv(v: &Json) -> String {
    match v {
        Json::Null => String::new(),
        Json::String(s) => csv_escape(s),
        other => csv_escape(&other.to_string()),
    }
}

fn csv_escape(v: &str) -> String {
    if v.contains(',') || v.contains('"') || v.contains('\n') || v.contains('\r') {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

fn export_json(raw: &RawResult) -> String {
    let mut arr = Vec::with_capacity(raw.rows.len());
    for row in &raw.rows {
        let mut obj = Map::new();
        for (i, col) in raw.columns.iter().enumerate() {
            obj.insert(col.clone(), row.get(i).cloned().unwrap_or(Json::Null));
        }
        arr.push(Json::Object(obj));
    }
    serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".to_string())
}

fn export_sql(raw: &RawResult, table_name: &str, dialect: &Dialect) -> String {
    let table = dialect.quote_identifier(table_name);
    let cols = raw
        .columns
        .iter()
        .map(|c| dialect.quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");
    let mut out = String::new();
    for row in &raw.rows {
        let values = row
            .iter()
            .map(|v| match v {
                Json::Null => "NULL".to_string(),
                Json::Number(n) => n.to_string(),
                Json::Bool(b) => if *b { "1" } else { "0" }.to_string(),
                Json::String(s) => format!("'{}'", s.replace('\'', "''")),
                other => format!("'{}'", other.to_string().replace('\'', "''")),
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("INSERT INTO {table} ({cols}) VALUES ({values});\n"));
    }
    out
}