//! Tauri SQL Manager 应用入口
pub mod commands;
pub mod db;
pub mod model;
pub mod service;
pub mod store;

use commands::AppState;
use db::PoolManager;
use std::sync::Arc;

/// Android / iOS 需要通过该属性导出原生入口，
/// 桌面端该属性不生效（cfg(mobile) 为 false）。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 注意：配置目录必须在 store::init() 之前确定。
    // 移动端没有 HOME 环境变量，dirs::data_dir() 会失败，
    // 因此先用 Tauri 的 PathResolver 解析并注入。

    let pools = Arc::new(PoolManager::new());
    let pools_for_exit = Arc::clone(&pools);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { pools })
        .setup(|app| {
            use tauri::Manager;
            // 用 Tauri 解析应用数据目录（移动端沙箱内可写）
            if let Ok(dir) = app.path().app_data_dir() {
                store::set_data_dir(dir);
            }
            // 此时路径已确定，再加载配置
            store::init();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_settings,
            commands::save_settings,
            commands::data_dir,
            commands::save_session,
            commands::list_connections,
            commands::get_connection,
            commands::create_connection,
            commands::update_connection,
            commands::delete_connection,
            commands::test_connection,
            commands::test_saved_connection,
            commands::connection_status,
            commands::disconnect,
            commands::pick_sqlite_file,
            commands::list_tables,
            commands::get_table_info,
            commands::list_indexes,
            commands::preview_create_sql,
            commands::create_table,
            commands::drop_table,
            commands::rename_table,
            commands::truncate_table,
            commands::alter_table,
            commands::add_column,
            commands::modify_column,
            commands::drop_column,
            commands::query_table_data,
            commands::insert_row,
            commands::update_row_by_pk,
            commands::delete_row_by_pk,
            commands::import_data,
            commands::execute_sql,
            commands::execute_batch,
            commands::execute_query_paged,
            commands::export_query_content,
            commands::export_query_to_file,
            commands::dialect_column_types,
            commands::dialect_default_port,
        ])
        .build(tauri::generate_context!())
        .expect("启动 Tauri 应用失败")
        .run(move |_app_handle, event| {
            // 应用退出时关闭所有数据库连接
            if let tauri::RunEvent::ExitRequested { .. } = event {
                pools_for_exit.close_all();
            }
        });
}