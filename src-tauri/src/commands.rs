//! Tauri 命令：前端通过 invoke 调用
//! 每个命令都是薄封装，业务逻辑位于 service 模块

use crate::db::PoolManager;
use crate::model::*;
use crate::service;
use crate::store;
use serde_json::Value as Json;
use std::sync::Arc;
use tauri::State;

/// 应用状态：连接池管理器
pub struct AppState {
    pub pools: Arc<PoolManager>,
}

// ==================== 应用信息 ====================

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Tauri SQL Manager".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        supported_types: DbType::all().iter().map(|t| t.as_str().to_string()).collect(),
        supported_type_labels: DbType::all()
            .iter()
            .map(|t| t.display_name().to_string())
            .collect(),
    }
}

#[tauri::command]
pub fn get_settings() -> AppSettings {
    store::settings()
}

#[tauri::command]
pub fn save_settings(settings: AppSettings) -> Result<(), String> {
    {
        let s = store::init();
        let mut guard = s.lock().map_err(|_| "配置存储锁定失败".to_string())?;
        guard.settings = settings.clone();
    }
    store::save_settings(&settings)
}

#[tauri::command]
pub fn data_dir() -> String {
    store::data_dir().to_string_lossy().to_string()
}

/// 保存会话（最后使用的连接与页面），用于下次启动恢复
#[tauri::command]
pub fn save_session(last_connection_id: Option<String>, last_menu: Option<String>) -> Result<(), String> {
    store::save_session(last_connection_id, last_menu)
}

// ==================== 连接管理 ====================

#[tauri::command]
pub fn list_connections() -> Vec<ConnectionConfig> {
    service::list_connections()
}

#[tauri::command]
pub fn get_connection(id: String) -> Result<ConnectionConfig, String> {
    service::get_connection(&id)
}

#[tauri::command]
pub fn create_connection(input: ConnectionInput) -> Result<ConnectionConfig, String> {
    service::create_connection(&input)
}

#[tauri::command]
pub fn update_connection(id: String, input: ConnectionInput) -> Result<ConnectionConfig, String> {
    service::update_connection(&id, &input)
}

#[tauri::command]
pub async fn delete_connection(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.pools.close(&id);
    service::delete_connection(&id)
}

#[tauri::command]
pub async fn test_connection(input: ConnectionInput) -> Result<TestResult, String> {
    Ok(service::test_connection(&input).await)
}

#[tauri::command]
pub async fn test_saved_connection(id: String) -> Result<TestResult, String> {
    service::test_saved_connection(&id).await
}

#[tauri::command]
pub async fn connection_status(id: String, state: State<'_, AppState>) -> Result<ConnectionStatus, String> {
    let config = store::find_connection(&id);
    let Some(config) = config else {
        return Ok(ConnectionStatus {
            exists: false,
            name: None,
            r#type: None,
            active: false,
            version: None,
            error: None,
        });
    };
    let active = state.pools.is_active(&id);
    let mut version = None;
    let mut error = None;
    if active {
        if let Some(db) = state.pools.cached(&id) {
            match crate::db::conn::version(&db).await {
                Ok(v) => version = Some(v),
                Err(e) => error = Some(e),
            }
        }
    }
    Ok(ConnectionStatus {
        exists: true,
        name: Some(config.name.clone()),
        r#type: Some(config.db_type.as_str().to_string()),
        active,
        version,
        error,
    })
}

#[tauri::command]
pub async fn disconnect(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.pools.close(&id);
    Ok(())
}

/// 弹出文件选择框选取 SQLite 数据库文件
#[tauri::command]
pub async fn pick_sqlite_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let file = app
        .dialog()
        .file()
        .add_filter("SQLite 数据库", &["db", "sqlite", "sqlite3"])
        .add_filter("所有文件", &["*"])
        .blocking_pick_file();
    Ok(file.and_then(|f| f.into_path().ok().map(|p| p.to_string_lossy().to_string())))
}

// ==================== 表结构管理 ====================

#[tauri::command]
pub async fn list_tables(conn_id: String, state: State<'_, AppState>) -> Result<Vec<TableInfo>, String> {
    service::list_tables(&state.pools, &conn_id).await
}

#[tauri::command]
pub async fn get_table_info(
    conn_id: String,
    table: String,
    state: State<'_, AppState>,
) -> Result<TableInfo, String> {
    service::get_table_info(&state.pools, &conn_id, &table).await
}

#[tauri::command]
pub async fn list_indexes(
    conn_id: String,
    table: String,
    state: State<'_, AppState>,
) -> Result<Vec<IndexInfo>, String> {
    service::list_indexes(&state.pools, &conn_id, &table).await
}

#[tauri::command]
pub fn preview_create_sql(
    conn_id: String,
    name: String,
    columns: Vec<ColumnDefinition>,
    comment: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    service::preview_create_sql(&state.pools, &conn_id, &name, &columns, comment.as_deref())
}

#[tauri::command]
pub async fn create_table(
    conn_id: String,
    name: String,
    columns: Vec<ColumnDefinition>,
    comment: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    service::create_table(&state.pools, &conn_id, &name, columns, comment).await
}

#[tauri::command]
pub async fn drop_table(
    conn_id: String,
    table: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    service::drop_table(&state.pools, &conn_id, &table).await
}

#[tauri::command]
pub async fn rename_table(
    conn_id: String,
    table: String,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    service::rename_table(&state.pools, &conn_id, &table, &new_name).await
}

#[tauri::command]
pub async fn truncate_table(
    conn_id: String,
    table: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    service::truncate_table(&state.pools, &conn_id, &table).await
}

#[tauri::command]
pub async fn alter_table(
    conn_id: String,
    table: String,
    columns: Vec<ColumnDefinition>,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    service::alter_table(&state.pools, &conn_id, &table, columns).await
}

#[tauri::command]
pub async fn add_column(
    conn_id: String,
    table: String,
    col: ColumnDefinition,
    state: State<'_, AppState>,
) -> Result<(), String> {
    service::add_column(&state.pools, &conn_id, &table, &col).await
}

#[tauri::command]
pub async fn modify_column(
    conn_id: String,
    table: String,
    col: ColumnDefinition,
    state: State<'_, AppState>,
) -> Result<(), String> {
    service::modify_column(&state.pools, &conn_id, &table, &col).await
}

#[tauri::command]
pub async fn drop_column(
    conn_id: String,
    table: String,
    column: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    service::drop_column(&state.pools, &conn_id, &table, &column).await
}

// ==================== 数据 CRUD ====================

#[tauri::command]
pub async fn query_table_data(
    conn_id: String,
    table: String,
    params: TableDataParams,
    state: State<'_, AppState>,
) -> Result<TableData, String> {
    service::query_table_data(&state.pools, &conn_id, &table, &params).await
}

#[tauri::command]
pub async fn insert_row(
    conn_id: String,
    table: String,
    row: serde_json::Map<String, Json>,
    state: State<'_, AppState>,
) -> Result<AffectedResult, String> {
    service::insert_row(&state.pools, &conn_id, &table, &row).await
}

#[tauri::command]
pub async fn update_row_by_pk(
    conn_id: String,
    table: String,
    values: serde_json::Map<String, Json>,
    pk: serde_json::Map<String, Json>,
    state: State<'_, AppState>,
) -> Result<AffectedResult, String> {
    service::update_row_by_pk(&state.pools, &conn_id, &table, &values, &pk).await
}

#[tauri::command]
pub async fn delete_row_by_pk(
    conn_id: String,
    table: String,
    pk: serde_json::Map<String, Json>,
    state: State<'_, AppState>,
) -> Result<AffectedResult, String> {
    service::delete_row_by_pk(&state.pools, &conn_id, &table, &pk).await
}

#[tauri::command]
pub async fn import_data(
    conn_id: String,
    table: String,
    format: String,
    content: String,
    mode: String,
    state: State<'_, AppState>,
) -> Result<ImportResult, String> {
    service::import_data(&state.pools, &conn_id, &table, &format, &content, &mode).await
}

// ==================== SQL 执行 ====================

#[tauri::command]
pub async fn execute_sql(
    conn_id: String,
    sql_text: String,
    state: State<'_, AppState>,
) -> Result<QueryResult, String> {
    service::execute_sql(&state.pools, &conn_id, &sql_text).await
}

#[tauri::command]
pub async fn execute_batch(
    conn_id: String,
    sql_text: String,
    state: State<'_, AppState>,
) -> Result<Vec<QueryResult>, String> {
    service::execute_batch(&state.pools, &conn_id, &sql_text).await
}

#[tauri::command]
pub async fn execute_query_paged(
    conn_id: String,
    sql_text: String,
    offset: i64,
    limit: i64,
    state: State<'_, AppState>,
) -> Result<QueryResult, String> {
    service::execute_query_paged(&state.pools, &conn_id, &sql_text, offset, limit).await
}

#[tauri::command]
pub async fn export_query_content(
    conn_id: String,
    sql_text: String,
    format: String,
    table_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    service::export_query_content(
        &state.pools,
        &conn_id,
        &sql_text,
        &format,
        table_name.as_deref(),
    )
    .await
}

/// 导出到文件：弹出保存对话框并写入
#[tauri::command]
pub async fn export_query_to_file(
    app: tauri::AppHandle,
    conn_id: String,
    sql_text: String,
    format: String,
    table_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let content = service::export_query_content(
        &state.pools,
        &conn_id,
        &sql_text,
        &format,
        table_name.as_deref(),
    )
    .await?;

    let ext = match format.to_lowercase().as_str() {
        "json" => "json",
        "sql" => "sql",
        _ => "csv",
    };
    let default_name = format!(
        "{}.{ext}",
        table_name.unwrap_or_else(|| "export".to_string())
    );

    let picked = app
        .dialog()
        .file()
        .set_file_name(&default_name)
        .add_filter(format.to_uppercase(), &[ext])
        .blocking_save_file();

    let Some(path) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    std::fs::write(&path, content).map_err(|e| format!("写入文件失败: {e}"))?;
    Ok(Some(path.to_string_lossy().to_string()))
}

// ==================== 方言元信息 ====================

#[tauri::command]
pub fn dialect_column_types(db_type: DbType) -> Vec<String> {
    crate::db::Dialect::of(db_type)
        .column_types()
        .iter()
        .map(|s| s.to_string())
        .collect()
}

#[tauri::command]
pub fn dialect_default_port(db_type: DbType) -> u16 {
    db_type.default_port()
}
