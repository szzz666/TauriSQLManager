//! 数据库访问层

pub mod conn;
pub mod dialect;
pub mod exec;
pub mod mssql;
pub mod meta;

pub use conn::PoolManager;
pub use dialect::Dialect;

/// Base64 编码（二进制列在 JSON 中以 Base64 表示）
pub fn encode_base64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
