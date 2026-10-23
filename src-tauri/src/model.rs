//! 前后端共享的数据模型
//! 所有结构体统一使用 camelCase 序列化，与 TypeScript 侧类型保持一致。

use serde::{Deserialize, Serialize};

/// 支持的数据库类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    Sqlite,
    Mysql,
    Mariadb,
    Postgresql,
    Mssql,
}

impl DbType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DbType::Sqlite => "sqlite",
            DbType::Mysql => "mysql",
            DbType::Mariadb => "mariadb",
            DbType::Postgresql => "postgresql",
            DbType::Mssql => "mssql",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            DbType::Sqlite => "SQLite",
            DbType::Mysql => "MySQL",
            DbType::Mariadb => "MariaDB",
            DbType::Postgresql => "PostgreSQL",
            DbType::Mssql => "SQL Server",
        }
    }

    /// 默认端口，SQLite 无端口
    pub fn default_port(&self) -> u16 {
        match self {
            DbType::Sqlite => 0,
            DbType::Mysql => 3306,
            DbType::Mariadb => 3306,
            DbType::Postgresql => 5432,
            DbType::Mssql => 1433,
        }
    }

    pub fn all() -> Vec<DbType> {
        vec![
            DbType::Sqlite,
            DbType::Mysql,
            DbType::Mariadb,
            DbType::Postgresql,
            DbType::Mssql,
        ]
    }
}

/// 数据库连接配置（持久化结构）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionConfig {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub db_type: DbType,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    /// 数据库名；SQLite 时为文件绝对路径
    #[serde(default)]
    pub database: String,
    #[serde(default)]
    pub username: String,
    /// 密码仅保存在本地配置文件中，返回前端时会被清空
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_true")]
    pub auto_reconnect: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

fn default_true() -> bool {
    true
}

impl ConnectionConfig {
    /// 返回给前端时脱敏：清空密码
    pub fn masked(&self) -> Self {
        let mut copy = self.clone();
        copy.password.clear();
        copy
    }

    /// 端口为空时回退到类型默认端口
    pub fn effective_port(&self) -> u16 {
        if self.port == 0 {
            self.db_type.default_port()
        } else {
            self.port
        }
    }
}

/// 新建/更新连接的载荷
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInput {
    pub name: String,
    #[serde(rename = "type")]
    pub db_type: DbType,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub database: String,
    #[serde(default)]
    pub username: String,
    /// 留空表示“不修改密码”（更新时）
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub auto_reconnect: Option<bool>,
}

/// 连接测试结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub success: bool,
    pub message: String,
    pub elapsed: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// 连接运行状态
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 列信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub col_type: String,
    pub length: i32,
    pub scale: i32,
    pub nullable: bool,
    pub auto_increment: bool,
    pub primary_key: bool,
    pub unique: bool,
    pub default_value: String,
    pub comment: String,
    pub ordinal: i32,
}

impl ColumnInfo {
    /// 含精度的完整类型，例如 VARCHAR(255)
    #[allow(dead_code)]
    pub fn full_type(&self) -> String {
        let mut s = self.col_type.to_uppercase();
        if self.length > 0 {
            s.push('(');
            s.push_str(&self.length.to_string());
            if self.scale > 0 {
                s.push(',');
                s.push_str(&self.scale.to_string());
            }
            s.push(')');
        }
        s
    }
}

/// 表信息
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub name: String,
    pub comment: String,
    pub table_type: String,
    pub row_count: i64,
    pub data_size: i64,
    pub columns: Vec<ColumnInfo>,
    pub primary_keys: Vec<String>,
    pub create_time: String,
    pub update_time: String,
}

/// 索引信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub name: String,
    pub column_name: String,
    pub unique: bool,
    pub primary_key: bool,
    pub asc_or_desc: String,
    pub ordinal: i32,
}

/// 建表/改表的列定义
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnDefinition {
    pub name: String,
    #[serde(rename = "type", default)]
    pub col_type: String,
    #[serde(default)]
    pub length: i32,
    #[serde(default)]
    pub scale: i32,
    #[serde(default = "default_true")]
    pub nullable: bool,
    #[serde(default)]
    pub auto_increment: bool,
    #[serde(default)]
    pub primary_key: bool,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
    /// 改表操作：ADD / MODIFY / DROP / RENAME
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub old_name: Option<String>,
}

/// SQL 执行结果
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub query: bool,
    pub columns: Vec<String>,
    pub column_types: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub affected_rows: i64,
    pub elapsed: i64,
    pub sql: String,
    pub truncated: bool,
    pub returned_rows: i64,
    pub message: String,
}

/// 分页表数据
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableData {
    pub rows: Vec<serde_json::Map<String, serde_json::Value>>,
    pub columns: Vec<String>,
    pub column_types: Vec<String>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

/// 分页查询参数
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDataParams {
    #[serde(default = "one_i64")]
    pub page: i64,
    #[serde(default = "twenty_i64")]
    pub page_size: i64,
    #[serde(default)]
    pub sort_column: Option<String>,
    #[serde(default)]
    pub sort_order: Option<String>,
    /// 原始 WHERE 条件（不含 WHERE 关键字）
    #[serde(default)]
    pub r#where: Option<String>,
}

fn one_i64() -> i64 {
    1
}
fn twenty_i64() -> i64 {
    20
}

/// 导入结果统计
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub total_rows: i64,
    pub affected_rows: i64,
    pub format: String,
    pub table: String,
}

/// 影响行数结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AffectedResult {
    pub affected: i64,
}

/// 应用设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// 单次查询最大返回行数
    pub max_query_rows: i64,
    /// SQL 查询超时（秒）
    pub query_timeout: u64,
    /// 数据浏览默认每页行数
    pub page_size: i64,
    /// 上次使用的连接（用于启动时恢复会话）
    #[serde(default)]
    pub last_connection_id: Option<String>,
    /// 上次停留的页面（用于启动时恢复会话）
    #[serde(default)]
    pub last_menu: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            max_query_rows: 10_000,
            query_timeout: 60,
            page_size: 20,
            last_connection_id: None,
            last_menu: None,
        }
    }
}

/// 应用信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub supported_types: Vec<String>,
    /// 与 supported_types 一一对应的展示名称
    pub supported_type_labels: Vec<String>,
}
