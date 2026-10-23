//! Tauri SQL Manager 应用入口
pub mod commands;
pub mod db;
pub mod model;
pub mod service;
pub mod store;

use commands::AppState;
use db::PoolManager;
use std::sync::Arc;

pub fn run() {
    // 初始化配置存储（读取本地连接与设置）
    store::init();

    let pools = Arc::new(PoolManager::new());
    let pools_for_exit = Arc::clone(&pools);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { pools })
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