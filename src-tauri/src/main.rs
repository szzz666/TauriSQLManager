// 阻止 Windows 下弹出控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri_sql_manager_lib::run()
}
