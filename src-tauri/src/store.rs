//! 本地配置持久化
//! 连接配置与设置保存在系统应用数据目录，替代原项目的 connections.json / config.yml

use crate::model::{AppSettings, ConnectionConfig};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// 启动时由 Tauri 解析出的应用数据目录。
/// 移动端（Android/iOS）没有 HOME 环境变量，`dirs::data_dir()` 会返回 None，
/// 回退到 "." 是只读的安装目录，因此必须由 Tauri 的 path API 提供正确路径。
static DATA_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// 设置应用数据目录（在 tauri setup 中调用）
pub fn set_data_dir(dir: PathBuf) {
    let _ = DATA_DIR_OVERRIDE.set(dir);
}

/// 应用数据目录
/// 优先级：TSM_DATA_DIR 环境变量（测试用） > Tauri 注入目录 > dirs 推断
pub fn data_dir() -> PathBuf {
    let dir = match std::env::var("TSM_DATA_DIR") {
        Ok(custom) if !custom.trim().is_empty() => PathBuf::from(custom),
        _ => match DATA_DIR_OVERRIDE.get() {
            Some(d) => d.clone(),
            None => dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("tauri-sql-manager"),
        },
    };
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

fn connections_path() -> PathBuf {
    data_dir().join("connections.json")
}

fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

/// 全局配置存储；用 Mutex 串行化文件读写，避免并发写入损坏文件
pub struct Store {
    pub connections: Vec<ConnectionConfig>,
    pub settings: AppSettings,
}

static STORE: OnceLock<Mutex<Store>> = OnceLock::new();

/// 测试辅助：按当前 TSM_DATA_DIR 重新加载配置（仅测试使用）
#[doc(hidden)]
pub fn reload_for_test() {
    let connections = load_connections();
    let settings = load_settings();
    if let Some(store) = STORE.get() {
        if let Ok(mut guard) = store.lock() {
            guard.connections = connections;
            guard.settings = settings;
        }
        return;
    }
    let _ = init();
}

/// 初始化全局存储（在应用启动时调用一次）
pub fn init() -> &'static Mutex<Store> {
    STORE.get_or_init(|| {
        let connections = load_connections();
        let settings = load_settings();
        Mutex::new(Store {
            connections,
            settings,
        })
    })
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    serde_json::from_str(&text).ok()
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| format!("序列化配置失败: {e}"))?;
    // 先写临时文件再重命名，避免写入中途崩溃导致配置损坏
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|e| format!("写入配置失败: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("保存配置失败: {e}"))?;
    Ok(())
}

fn load_connections() -> Vec<ConnectionConfig> {
    read_json::<Vec<ConnectionConfig>>(&connections_path()).unwrap_or_default()
}

fn load_settings() -> AppSettings {
    read_json::<AppSettings>(&settings_path()).unwrap_or_default()
}

/// 保存连接列表
pub fn save_connections(connections: &[ConnectionConfig]) -> Result<(), String> {
    write_json(&connections_path(), &connections.to_vec())
}

/// 保存应用设置
pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    write_json(&settings_path(), settings)
}

/// 仅更新会话字段（最后连接 / 最后页面），不影响其它设置
pub fn save_session(last_connection_id: Option<String>, last_menu: Option<String>) -> Result<(), String> {
    let updated = {
        let handle = init();
        let mut guard = handle.lock().map_err(|_| "配置存储锁定失败".to_string())?;
        guard.settings.last_connection_id = last_connection_id;
        guard.settings.last_menu = last_menu;
        guard.settings.clone()
    };
    save_settings(&updated)
}

/// 读取设置副本
pub fn settings() -> AppSettings {
    init().lock().map(|s| s.settings.clone()).unwrap_or_default()
}

/// 读取连接列表副本
pub fn connections() -> Vec<ConnectionConfig> {
    init()
        .lock()
        .map(|s| s.connections.clone())
        .unwrap_or_default()
}

/// 按 id 查找连接配置（含明文密码，仅供内部建立连接使用）
pub fn find_connection(id: &str) -> Option<ConnectionConfig> {
    init()
        .lock()
        .ok()
        .and_then(|s| s.connections.iter().find(|c| c.id == id).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_defaults_are_sane() {
        let s = AppSettings::default();
        assert_eq!(s.max_query_rows, 10_000);
        assert_eq!(s.query_timeout, 60);
        assert_eq!(s.page_size, 20);
    }

    #[test]
    fn data_dir_is_created() {
        let dir = data_dir();
        assert!(dir.exists());
    }
}
