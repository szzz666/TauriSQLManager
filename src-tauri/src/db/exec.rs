//! 统一查询执行
//! sqlx 的每种驱动有独立的类型系统，这里把执行结果统一成“列 + 行”的原始结构
//! 对应原项目 SqlService.safeGetObject 的取值转换逻辑

use crate::db::conn::Db;
use serde_json::Value as Json;
use sqlx::{Column, Row, TypeInfo, ValueRef};

/// 原始结果集
#[derive(Debug, Default)]
pub struct RawResult {
    pub columns: Vec<String>,
    pub column_types: Vec<String>,
    pub rows: Vec<Vec<Json>>,
    /// 是否还有更多行（被 limit 截断）
    pub truncated: bool,
}

/// 从字符串构造绑定的参数值
#[derive(Debug, Clone, PartialEq)]
pub enum Bind {
    Null,
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl Bind {
    /// 从 JSON 值推断参数类型；数字优先按整数处理
    pub fn from_json(v: &Json) -> Self {
        match v {
            Json::Null => Bind::Null,
            Json::Bool(b) => Bind::Bool(*b),
            Json::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Bind::Int(i)
                } else if let Some(f) = n.as_f64() {
                    Bind::Float(f)
                } else {
                    Bind::Str(n.to_string())
                }
            }
            Json::String(s) => Bind::Str(s.clone()),
            other => Bind::Str(other.to_string()),
        }
    }
}

/// 执行 SQL 并返回结果集；limit 为最大读取行数
pub async fn fetch_all(
    db: &Db,
    sql: &str,
    binds: &[Bind],
    limit: usize,
) -> Result<RawResult, String> {
    match db {
        Db::Sqlite(pool) => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = match b {
                    Bind::Null => q.bind(Option::<String>::None),
                    Bind::Str(s) => q.bind(s.clone()),
                    Bind::Int(i) => q.bind(*i),
                    Bind::Float(f) => q.bind(*f),
                    Bind::Bool(v) => q.bind(*v),
                };
            }
            let rows = q
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
            let mut result = RawResult::default();
            if let Some(first) = rows.first() {
                for col in first.columns() {
                    result.columns.push(col.name().to_string());
                    result.column_types.push(col.type_info().name().to_string());
                }
            }
            for row in rows.iter() {
                let mut values = Vec::with_capacity(result.columns.len());
                for (i, _) in result.columns.iter().enumerate() {
                    values.push(sqlite_value(row, i));
                }
                result.rows.push(values);
            }
            truncate(&mut result, limit);
            Ok(result)
        }
        Db::MySql(pool) => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = match b {
                    Bind::Null => q.bind(Option::<String>::None),
                    Bind::Str(s) => q.bind(s.clone()),
                    Bind::Int(i) => q.bind(*i),
                    Bind::Float(f) => q.bind(*f),
                    Bind::Bool(v) => q.bind(*v),
                };
            }
            let rows = q
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
            let mut result = RawResult::default();
            if let Some(first) = rows.first() {
                for col in first.columns() {
                    result.columns.push(col.name().to_string());
                    result.column_types.push(col.type_info().name().to_string());
                }
            }
            for row in rows.iter() {
                let mut values = Vec::with_capacity(result.columns.len());
                for (i, _) in result.columns.iter().enumerate() {
                    values.push(mysql_value(row, i));
                }
                result.rows.push(values);
            }
            truncate(&mut result, limit);
            Ok(result)
        }
        Db::Postgres(pool) => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = match b {
                    Bind::Null => q.bind(Option::<String>::None),
                    Bind::Str(s) => q.bind(s.clone()),
                    Bind::Int(i) => q.bind(*i),
                    Bind::Float(f) => q.bind(*f),
                    Bind::Bool(v) => q.bind(*v),
                };
            }
            let rows = q
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
            let mut result = RawResult::default();
            if let Some(first) = rows.first() {
                for col in first.columns() {
                    result.columns.push(col.name().to_string());
                    result.column_types.push(col.type_info().name().to_string());
                }
            }
            for row in rows.iter() {
                let mut values = Vec::with_capacity(result.columns.len());
                for (i, _) in result.columns.iter().enumerate() {
                    values.push(pg_value(row, i));
                }
                result.rows.push(values);
            }
            truncate(&mut result, limit);
            Ok(result)
        }
        Db::Mssql(config) => crate::db::mssql::fetch(config, sql, binds, limit).await,
    }
}

fn truncate(result: &mut RawResult, limit: usize) {
    if limit > 0 && result.rows.len() > limit {
        result.rows.truncate(limit);
        result.truncated = true;
    }
}

/// 判断 SQL 是否可能返回结果集（用于决定走查询还是执行更新）
pub fn returns_rows(sql: &str) -> bool {
    let s = strip_leading_comments(sql).trim_start().to_uppercase();
    s.starts_with("SELECT")
        || s.starts_with("WITH")
        || s.starts_with("SHOW")
        || s.starts_with("DESC")
        || s.starts_with("DESCRIBE")
        || s.starts_with("EXPLAIN")
        || s.starts_with("PRAGMA")
        || s.starts_with("VALUES")
        || s.contains(" RETURNING ")
}

/// 去掉 SQL 前导注释与空白，便于判断语句类型
pub fn strip_leading_comments(sql: &str) -> &str {
    let mut rest = sql;
    loop {
        let trimmed = rest.trim_start();
        if let Some(tail) = trimmed.strip_prefix("--") {
            match tail.find('\n') {
                Some(pos) => rest = &tail[pos + 1..],
                None => return "",
            }
        } else if let Some(tail) = trimmed.strip_prefix("/*") {
            match tail.find("*/") {
                Some(pos) => rest = &tail[pos + 2..],
                None => return "",
            }
        } else {
            return trimmed;
        }
    }
}

/// 执行更新类语句，返回影响行数
pub async fn execute_update(
    db: &Db,
    sql: &str,
    binds: &[Bind],
) -> Result<i64, String> {
    match db {
        Db::Sqlite(pool) => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = bind_sqlite(q, b);
            }
            let r = q.execute(pool).await.map_err(|e| e.to_string())?;
            Ok(r.rows_affected() as i64)
        }
        Db::MySql(pool) => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = bind_mysql(q, b);
            }
            let r = q.execute(pool).await.map_err(|e| e.to_string())?;
            Ok(r.rows_affected() as i64)
        }
        Db::Postgres(pool) => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = bind_pg(q, b);
            }
            let r = q.execute(pool).await.map_err(|e| e.to_string())?;
            Ok(r.rows_affected() as i64)
        }
        Db::Mssql(config) => crate::db::mssql::execute(config, sql, binds).await,
    }
}

fn bind_sqlite<'q>(
    q: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    b: &Bind,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    match b {
        Bind::Null => q.bind(Option::<String>::None),
        Bind::Str(s) => q.bind(s.clone()),
        Bind::Int(i) => q.bind(*i),
        Bind::Float(f) => q.bind(*f),
        Bind::Bool(v) => q.bind(*v),
    }
}

fn bind_mysql<'q>(
    q: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    b: &Bind,
) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
    match b {
        Bind::Null => q.bind(Option::<String>::None),
        Bind::Str(s) => q.bind(s.clone()),
        Bind::Int(i) => q.bind(*i),
        Bind::Float(f) => q.bind(*f),
        Bind::Bool(v) => q.bind(*v),
    }
}

fn bind_pg<'q>(
    q: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    b: &Bind,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match b {
        Bind::Null => q.bind(Option::<String>::None),
        Bind::Str(s) => q.bind(s.clone()),
        Bind::Int(i) => q.bind(*i),
        Bind::Float(f) => q.bind(*f),
        Bind::Bool(v) => q.bind(*v),
    }
}

// ===== 取值：把各驱动的原生类型转换为 JSON =====
// 采用“按 Rust 类型依次尝试”的策略，失败则回退为字符串

fn sqlite_value(row: &sqlx::sqlite::SqliteRow, i: usize) -> Json {
    let raw = match row.try_get_raw(i) {
        Ok(v) => v,
        Err(_) => return Json::Null,
    };
    if raw.is_null() {
        return Json::Null;
    }
    let type_name = match row.try_get_raw(i) {
        Ok(v) => v.type_info().name().to_string(),
        Err(_) => String::new(),
    };
    match type_name.as_str() {
        "INTEGER" => row
            .try_get::<i64, _>(i)
            .map(Json::from)
            .or_else(|_| row.try_get::<String, _>(i).map(Json::from))
            .unwrap_or(Json::Null),
        "REAL" => row
            .try_get::<f64, _>(i)
            .map(Json::from)
            .or_else(|_| row.try_get::<String, _>(i).map(Json::from))
            .unwrap_or(Json::Null),
        "BLOB" => match row.try_get::<Vec<u8>, _>(i) {
            Ok(bytes) => Json::String(crate::db::encode_base64(&bytes)),
            Err(_) => row
                .try_get::<String, _>(i)
                .map(Json::from)
                .unwrap_or(Json::Null),
        },
        _ => row
            .try_get::<String, _>(i)
            .map(Json::from)
            .or_else(|_| row.try_get::<i64, _>(i).map(Json::from))
            .or_else(|_| row.try_get::<f64, _>(i).map(Json::from))
            .unwrap_or(Json::Null),
    }
}

fn mysql_value(row: &sqlx::mysql::MySqlRow, i: usize) -> Json {
    let type_name = row
        .try_get_raw(i)
        .map(|v| v.type_info().name().to_string())
        .unwrap_or_default();
    if row
        .try_get_raw(i)
        .map(|v| v.is_null())
        .unwrap_or(true)
    {
        return Json::Null;
    }
    match type_name.as_str() {
        "TINYINT" | "SMALLINT" | "INT" | "MEDIUMINT" | "BIGINT" => row
            .try_get::<i64, _>(i)
            .map(Json::from)
            .or_else(|_| row.try_get::<u64, _>(i).map(Json::from))
            .or_else(|_| row.try_get::<String, _>(i).map(Json::from))
            .unwrap_or(Json::Null),
        "FLOAT" | "DOUBLE" => row
            .try_get::<f64, _>(i)
            .map(Json::from)
            .or_else(|_| row.try_get::<String, _>(i).map(Json::from))
            .unwrap_or(Json::Null),
        "DECIMAL" => row
            .try_get::<sqlx::types::BigDecimal, _>(i)
            .map(|d| Json::String(d.to_string()))
            .or_else(|_| row.try_get::<String, _>(i).map(Json::from))
            .unwrap_or(Json::Null),
        "BLOB" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" | "BINARY" | "VARBINARY" => {
            match row.try_get::<Vec<u8>, _>(i) {
                Ok(bytes) => Json::String(crate::db::encode_base64(&bytes)),
                Err(_) => row
                    .try_get::<String, _>(i)
                    .map(Json::from)
                    .unwrap_or(Json::Null),
            }
        }
        "DATE" => row
            .try_get::<chrono::NaiveDate, _>(i)
            .map(|d| Json::String(d.to_string()))
            .or_else(|_| row.try_get::<String, _>(i).map(Json::from))
            .unwrap_or(Json::Null),
        "DATETIME" | "TIMESTAMP" => row
            .try_get::<chrono::NaiveDateTime, _>(i)
            .map(|d| Json::String(d.to_string()))
            .or_else(|_| row.try_get::<String, _>(i).map(Json::from))
            .unwrap_or(Json::Null),
        "TIME" => row
            .try_get::<chrono::NaiveTime, _>(i)
            .map(|t| Json::String(t.to_string()))
            .or_else(|_| row.try_get::<String, _>(i).map(Json::from))
            .unwrap_or(Json::Null),
        _ => row
            .try_get::<String, _>(i)
            .map(Json::from)
            .unwrap_or(Json::Null),
    }
}

fn pg_value(row: &sqlx::postgres::PgRow, i: usize) -> Json {
    let type_name = row
        .try_get_raw(i)
        .map(|v| v.type_info().name().to_string())
        .unwrap_or_default();
    if row
        .try_get_raw(i)
        .map(|v| v.is_null())
        .unwrap_or(true)
    {
        return Json::Null;
    }
    match type_name.as_str() {
        "BOOL" => row
            .try_get::<bool, _>(i)
            .map(Json::from)
            .unwrap_or(Json::Null),
        "INT2" => row
            .try_get::<i16, _>(i)
            .map(|v| Json::from(v as i64))
            .unwrap_or(Json::Null),
        "INT4" => row
            .try_get::<i32, _>(i)
            .map(|v| Json::from(v as i64))
            .unwrap_or(Json::Null),
        "INT8" => row
            .try_get::<i64, _>(i)
            .map(Json::from)
            .unwrap_or(Json::Null),
        "FLOAT4" => row
            .try_get::<f32, _>(i)
            .map(|v| Json::from(v as f64))
            .unwrap_or(Json::Null),
        "FLOAT8" => row
            .try_get::<f64, _>(i)
            .map(Json::from)
            .unwrap_or(Json::Null),
        "NUMERIC" => row
            .try_get::<sqlx::types::BigDecimal, _>(i)
            .map(|d| Json::String(d.to_string()))
            .or_else(|_| row.try_get::<String, _>(i).map(Json::from))
            .unwrap_or(Json::Null),
        "BYTEA" => match row.try_get::<Vec<u8>, _>(i) {
            Ok(bytes) => Json::String(crate::db::encode_base64(&bytes)),
            Err(_) => Json::Null,
        },
        "JSON" | "JSONB" => row
            .try_get::<Json, _>(i)
            .unwrap_or(Json::Null),
        "UUID" => row
            .try_get::<uuid::Uuid, _>(i)
            .map(|u| Json::String(u.to_string()))
            .or_else(|_| row.try_get::<String, _>(i).map(Json::from))
            .unwrap_or(Json::Null),
        _ => row
            .try_get::<String, _>(i)
            .map(Json::from)
            .or_else(|_| row.try_get::<i64, _>(i).map(Json::from))
            .or_else(|_| row.try_get::<f64, _>(i).map(Json::from))
            .or_else(|_| row.try_get::<chrono::NaiveDateTime, _>(i).map(|d| Json::String(d.to_string())))
            .unwrap_or(Json::Null),
    }
}

/// 取单值字符串结果（预留：用于读取版本、计数等标量查询）
#[allow(dead_code)]
pub async fn scalar_string(db: &Db, sql: &str) -> Result<String, String> {
    let r = fetch_all(db, sql, &[], 1).await?;
    Ok(r.rows
        .first()
        .and_then(|row| row.first())
        .and_then(|v| match v {
            Json::String(s) => Some(s.clone()),
            Json::Null => None,
            other => Some(other.to_string()),
        })
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_returning_statements() {
        assert!(returns_rows("SELECT * FROM t"));
        assert!(returns_rows("  \n select 1"));
        assert!(returns_rows("-- 注释\nSELECT 1"));
        assert!(returns_rows("/* 注释 */ SELECT 1"));
        assert!(returns_rows("WITH x AS (SELECT 1) SELECT * FROM x"));
        assert!(returns_rows("SHOW TABLES"));
        assert!(returns_rows("PRAGMA table_info(t)"));
        assert!(!returns_rows("INSERT INTO t VALUES (1)"));
        assert!(!returns_rows("UPDATE t SET a = 1"));
        assert!(!returns_rows("DELETE FROM t"));
    }

    #[test]
    fn strips_comments() {
        assert_eq!(strip_leading_comments("-- c\nSELECT 1"), "SELECT 1");
        assert_eq!(strip_leading_comments("/* c */ SELECT 1"), "SELECT 1");
        assert_eq!(strip_leading_comments("   SELECT 1"), "SELECT 1");
    }

    #[test]
    fn infers_bind_types() {
        assert_eq!(Bind::from_json(&json!(null)), Bind::Null);
        assert_eq!(Bind::from_json(&json!(42)), Bind::Int(42));
        assert_eq!(Bind::from_json(&json!(1.5)), Bind::Float(1.5));
        assert_eq!(Bind::from_json(&json!(true)), Bind::Bool(true));
        assert_eq!(Bind::from_json(&json!("abc")), Bind::Str("abc".into()));
    }
}
