//! 元数据查询：表列表、表结构、索引
//! 原项目通过 JDBC DatabaseMetaData 获取，这里改为各数据库的系统表 / information_schema 查询

use crate::db::conn::Db;
use crate::db::exec::{fetch_all, Bind};
use crate::model::{ColumnInfo, DbType, IndexInfo, TableInfo};
use serde_json::Value as Json;

/// 取字符串值
fn s(v: &Json) -> String {
    match v {
        Json::String(s) => s.clone(),
        Json::Null => String::new(),
        other => other.to_string(),
    }
}

/// 取整数值
fn n(v: &Json) -> i64 {
    match v {
        Json::Number(num) => num.as_i64().or_else(|| num.as_f64().map(|f| f as i64)).unwrap_or(0),
        Json::String(s) => s.parse::<i64>().unwrap_or(0),
        Json::Bool(b) => *b as i64,
        _ => 0,
    }
}

fn b(v: &Json) -> bool {
    match v {
        Json::Bool(x) => *x,
        Json::Number(num) => num.as_i64().unwrap_or(0) != 0,
        Json::String(s) => {
            let lower = s.to_lowercase();
            lower == "1" || lower == "true" || lower == "yes" || lower == "y"
        }
        _ => false,
    }
}

/// 列出所有表
pub async fn list_tables(db: &Db) -> Result<Vec<TableInfo>, String> {
    // 四种数据库各自的表清单查询
    let rows = match db {
        Db::Sqlite(_) => {
            // SQLite 没有行数统计表，先取出表清单，再逐表 COUNT(*)
            let sql = "SELECT m.name AS table_name, m.type AS table_type, '' AS comment, \
                              0 AS row_count, 0 AS data_size, '' AS create_time, '' AS update_time, \
                              CASE WHEN m.type = 'table' THEN 1 ELSE 0 END AS countable \
                       FROM sqlite_master m WHERE m.type IN ('table','view') AND m.name NOT LIKE 'sqlite_%' \
                       ORDER BY m.type, m.name";
            let mut result = fetch_all(db, sql, &[], 0).await?;
            for row in result.rows.iter_mut() {
                let name = s(&row[0]);
                let countable = row.get(7).map(b).unwrap_or(false);
                if countable {
                    let count_sql = format!(
                        "SELECT COUNT(*) FROM {}",
                        crate::db::Dialect::of(DbType::Sqlite).quote_identifier(&name)
                    );
                    if let Ok(r) = fetch_all(db, &count_sql, &[], 1).await {
                        if let Some(v) = r.rows.first().and_then(|x| x.first()) {
                            row[3] = Json::from(n(v));
                        }
                    }
                }
            }
            result
        }
        Db::MySql(_) => {
            let sql = "SELECT TABLE_NAME AS table_name, TABLE_TYPE AS table_type, TABLE_COMMENT AS comment, \
                       TABLE_ROWS AS row_count, DATA_LENGTH AS data_size, CREATE_TIME AS create_time, \
                       UPDATE_TIME AS update_time FROM information_schema.TABLES \
                       WHERE TABLE_SCHEMA = DATABASE() ORDER BY TABLE_NAME";
            fetch_all(db, sql, &[], 0).await?
        }
        Db::Postgres(_) => {
            let sql = "SELECT c.relname AS table_name, \
                              CASE c.relkind WHEN 'v' THEN 'VIEW' WHEN 'm' THEN 'VIEW' ELSE 'TABLE' END AS table_type, \
                              COALESCE(obj_description(c.oid, 'pg_class'), '') AS comment, \
                              GREATEST(c.reltuples::bigint, 0) AS row_count, \
                              pg_total_relation_size(c.oid) AS data_size, '' AS create_time, '' AS update_time \
                       FROM pg_class c JOIN pg_namespace ns ON ns.oid = c.relnamespace \
                       WHERE ns.nspname NOT IN ('pg_catalog','information_schema') AND c.relkind IN ('r','v','m','p') \
                       ORDER BY c.relname";
            fetch_all(db, sql, &[], 0).await?
        }
        Db::Mssql(config) => {
            let sql = "SELECT t.name AS table_name, \
                              CASE WHEN t.type = 'V' THEN 'VIEW' ELSE 'TABLE' END AS table_type, \
                              CAST(ISNULL(CAST(ep.value AS NVARCHAR(500)), '') AS NVARCHAR(500)) AS comment, \
                              ISNULL(p.rows, 0) AS row_count, 0 AS data_size, '' AS create_time, '' AS update_time \
                       FROM sys.tables t \
                       LEFT JOIN sys.extended_properties ep ON ep.major_id = t.object_id AND ep.minor_id = 0 \
                            AND ep.name = 'MS_Description' \
                       LEFT JOIN sys.partitions p ON p.object_id = t.object_id AND p.index_id IN (0,1) \
                       UNION ALL \
                       SELECT v.name, 'VIEW', '', 0, 0, '', '' FROM sys.views v \
                       ORDER BY 1";
            let _ = config;
            fetch_all(db, sql, &[], 0).await?
        }
    };

    let mut tables = Vec::new();
    for row in &rows.rows {
        let name = s(&row[0]);
        let table_type = s(&row[1]);
        tables.push(TableInfo {
            name,
            table_type: if table_type.eq_ignore_ascii_case("view") { "VIEW".to_string() } else { "TABLE".to_string() },
            comment: s(&row[2]),
            row_count: n(&row[3]),
            data_size: row.get(4).map(n).unwrap_or(0),
            create_time: row.get(5).map(s).unwrap_or_default(),
            update_time: row.get(6).map(s).unwrap_or_default(),
            ..Default::default()
        });
    }
    Ok(tables)
}

/// 获取表结构（列 + 主键 + 注释）
pub async fn get_table_info(db: &Db, table: &str) -> Result<TableInfo, String> {
    let mut info = TableInfo {
        name: table.to_string(),
        ..Default::default()
    };

    let col_rows = match db {
        Db::Sqlite(_) => {
            let sql = "SELECT name, type, \"notnull\", dflt_value, pk FROM pragma_table_info(?) ORDER BY cid";
            fetch_all(db, sql, &[Bind::Str(table.to_string())], 0).await?
        }
        Db::MySql(_) => {
            let sql = "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT, COLUMN_KEY, \
                              EXTRA, COLUMN_COMMENT, COLUMN_TYPE, ORDINAL_POSITION, CHARACTER_MAXIMUM_LENGTH, \
                              NUMERIC_SCALE FROM information_schema.COLUMNS \
                       WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION";
            fetch_all(db, sql, &[Bind::Str(table.to_string())], 0).await?
        }
        Db::Postgres(_) => {
            let sql = "SELECT a.attname AS column_name, \
                              format_type(a.atttypid, a.atttypmod) AS data_type, \
                              CASE WHEN a.attnotnull THEN 'NO' ELSE 'YES' END AS is_nullable, \
                              COALESCE(pg_get_expr(d.adbin, d.adrelid), '') AS column_default, \
                              COALESCE(col_description(a.attrelid, a.attnum), '') AS comment, \
                              a.attnum AS ordinal, \
                              COALESCE(typ.typname, '') AS type_name, \
                              CASE WHEN d.adbin IS NOT NULL AND pg_get_expr(d.adbin, d.adrelid) LIKE 'nextval%' \
                                   THEN 'YES' ELSE 'NO' END AS is_identity \
                       FROM pg_attribute a \
                       JOIN pg_class c ON c.oid = a.attrelid \
                       JOIN pg_namespace ns ON ns.oid = c.relnamespace \
                       LEFT JOIN pg_type typ ON typ.oid = a.atttypid \
                       LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
                       WHERE ns.nspname NOT IN ('pg_catalog','information_schema') AND c.relname = ? \
                             AND a.attnum > 0 AND NOT a.attisdropped \
                       ORDER BY a.attnum";
            fetch_all(db, sql, &[Bind::Str(table.to_string())], 0).await?
        }
        Db::Mssql(_) => {
            let sql = "SELECT c.name AS column_name, ty.name AS data_type, \
                              CASE WHEN c.is_nullable = 1 THEN 'YES' ELSE 'NO' END AS is_nullable, \
                              ISNULL(OBJECT_DEFINITION(c.default_object_id), '') AS column_default, \
                              CAST(ISNULL(CAST(ep.value AS NVARCHAR(500)), '') AS NVARCHAR(500)) AS comment, \
                              c.column_id AS ordinal, \
                              CASE WHEN c.is_identity = 1 THEN 'YES' ELSE 'NO' END AS is_identity, \
                              c.max_length, c.precision, c.scale \
                       FROM sys.columns c \
                       JOIN sys.types ty ON ty.user_type_id = c.user_type_id \
                       LEFT JOIN sys.extended_properties ep ON ep.major_id = c.object_id \
                            AND ep.minor_id = c.column_id AND ep.name = 'MS_Description' \
                       WHERE c.object_id = OBJECT_ID(@P1) ORDER BY c.column_id";
            fetch_all(db, sql, &[Bind::Str(table.to_string())], 0).await?
        }
    };

    let mut columns = Vec::new();
    let mut primary_keys = Vec::new();

    for row in &col_rows.rows {
        let mut col = ColumnInfo {
            name: s(&row[0]),
            col_type: s(&row[1]),
            length: 0,
            scale: 0,
            nullable: true,
            auto_increment: false,
            primary_key: false,
            unique: false,
            default_value: String::new(),
            comment: String::new(),
            ordinal: 0,
        };

        match db {
            Db::Sqlite(_) => {
                // name, type, notnull, dflt_value, pk
                col.nullable = !b(&row[2]);
                col.default_value = s(&row[3]);
                let pk_flag = n(&row[4]) > 0;
                col.primary_key = pk_flag;
                if pk_flag {
                    // SQLite 的 INTEGER PRIMARY KEY 即自增
                    col.auto_increment = col.col_type.eq_ignore_ascii_case("INTEGER");
                    primary_keys.push(col.name.clone());
                }
                let (base_type, len, scale) = crate::db::meta::split_type(&col.col_type);
                col.col_type = base_type;
                col.length = len;
                col.scale = scale;
            }
            Db::MySql(_) => {
                // name, data_type, is_nullable, default, key, extra, comment, column_type, ordinal, max_len, scale
                col.col_type = s(&row[1]).to_uppercase();
                col.nullable = s(&row[2]).eq_ignore_ascii_case("YES");
                col.default_value = s(&row[3]);
                col.primary_key = s(&row[4]).eq_ignore_ascii_case("PRI");
                col.auto_increment = s(&row[5]).to_lowercase().contains("auto_increment");
                col.comment = s(&row[6]);
                col.ordinal = n(&row[8]) as i32;
                col.length = n(&row[9]) as i32;
                col.scale = n(&row[10]) as i32;
                // 从 COLUMN_TYPE 中解析长度与精度，兼容 integer 显示宽度
                let (_, len, scale) = split_type(&s(&row[7]));
                if len > 0 {
                    col.length = len;
                    col.scale = scale;
                }
                if col.primary_key {
                    primary_keys.push(col.name.clone());
                }
            }
            Db::Postgres(_) => {
                // name, data_type, is_nullable, default, comment, ordinal, type_name, is_identity
                let full = s(&row[1]);
                let (base_type, len, scale) = split_type(&full);
                col.col_type = base_type.to_uppercase();
                col.length = len;
                col.scale = scale;
                col.nullable = s(&row[2]).eq_ignore_ascii_case("YES");
                let def = s(&row[3]);
                col.default_value = def.clone();
                col.comment = s(&row[4]);
                col.ordinal = n(&row[5]) as i32;
                col.auto_increment = s(&row[7]).eq_ignore_ascii_case("YES");
                if col.auto_increment {
                    col.default_value = String::new();
                }
            }
            Db::Mssql(_) => {
                // name, data_type, is_nullable, default, comment, ordinal, is_identity, max_length, precision, scale
                col.col_type = s(&row[1]).to_uppercase();
                col.nullable = s(&row[2]).eq_ignore_ascii_case("YES");
                col.default_value = strip_default_wrapper(&s(&row[3]));
                col.comment = s(&row[4]);
                col.ordinal = n(&row[5]) as i32;
                col.auto_increment = s(&row[6]).eq_ignore_ascii_case("YES");
                let max_len = n(&row[7]);
                let precision = n(&row[8]) as i32;
                let scale = n(&row[9]) as i32;
                let upper = col.col_type.to_uppercase();
                if upper.contains("CHAR") || upper.contains("BINARY") {
                    col.length = if max_len == -1 { 0 } else if upper.starts_with('N') { (max_len / 2) as i32 } else { max_len as i32 };
                } else if upper.contains("DECIMAL") || upper.contains("NUMERIC") {
                    col.length = precision;
                    col.scale = scale;
                }
            }
        }

        columns.push(col);
    }

    // 主键（MySQL/SQL Server/PostgreSQL 需单独查询）
    if primary_keys.is_empty() {
        let pk_rows = match db {
            Db::MySql(_) => {
                let sql = "SELECT COLUMN_NAME FROM information_schema.KEY_COLUMN_USAGE \
                           WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND CONSTRAINT_NAME = 'PRIMARY' \
                           ORDER BY ORDINAL_POSITION";
                fetch_all(db, sql, &[Bind::Str(table.to_string())], 0).await?
            }
            Db::Postgres(_) => {
                let sql = "SELECT a.attname FROM pg_index i \
                           JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey) \
                           JOIN pg_class c ON c.oid = i.indrelid \
                           JOIN pg_namespace ns ON ns.oid = c.relnamespace \
                           WHERE i.indisprimary AND c.relname = ? AND ns.nspname NOT IN ('pg_catalog','information_schema') \
                           ORDER BY a.attnum";
                fetch_all(db, sql, &[Bind::Str(table.to_string())], 0).await?
            }
            Db::Mssql(_) => {
                let sql = "SELECT c.name FROM sys.index_columns ic \
                           JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
                           WHERE ic.object_id = OBJECT_ID(@P1) AND ic.is_primary_key = 1 ORDER BY ic.key_ordinal";
                fetch_all(db, sql, &[Bind::Str(table.to_string())], 0).await?
            }
            Db::Sqlite(_) => crate::db::exec::RawResult::default(),
        };
        for row in &pk_rows.rows {
            primary_keys.push(s(&row[0]));
        }
    }

    for pk in &primary_keys {
        if let Some(c) = columns.iter_mut().find(|c| c.name == *pk) {
            c.primary_key = true;
        }
    }

    // 表类型、注释与行数
    let tables = list_tables(db).await?;
    if let Some(t) = tables.into_iter().find(|t| t.name == table) {
        info.table_type = t.table_type;
        info.comment = t.comment;
        info.create_time = t.create_time;
        info.update_time = t.update_time;
    }
    info.row_count = count_rows(db, table).await.unwrap_or(0);
    info.columns = columns;
    info.primary_keys = primary_keys;
    Ok(info)
}

/// 统计行数
pub async fn count_rows(db: &Db, table: &str) -> Result<i64, String> {
    let dialect = crate::db::Dialect::of(db.db_type());
    let sql = format!("SELECT COUNT(*) FROM {}", dialect.quote_identifier(table));
    let r = fetch_all(db, &sql, &[], 1).await?;
    Ok(r.rows.first().and_then(|r| r.first()).map(n).unwrap_or(0))
}

/// 列出索引
pub async fn list_indexes(db: &Db, table: &str) -> Result<Vec<IndexInfo>, String> {
    let rows = match db {
        Db::Sqlite(_) => {
            let sql = "SELECT il.name AS index_name, ii.name AS column_name, \
                              CASE WHEN il.\"unique\" = 1 THEN 1 ELSE 0 END AS is_unique, \
                              CASE WHEN il.origin = 'pk' THEN 1 ELSE 0 END AS is_primary, \
                              '' AS asc_or_desc, ii.seqno AS ordinal \
                       FROM pragma_index_list(?) il \
                       JOIN pragma_index_info(il.name) ii WHERE il.name NOT LIKE 'sqlite_autoindex%' \
                       ORDER BY il.name, ii.seqno";
            fetch_all(db, sql, &[Bind::Str(table.to_string())], 0).await?
        }
        Db::MySql(_) => {
            let sql = "SELECT INDEX_NAME, COLUMN_NAME, CASE WHEN NON_UNIQUE = 0 THEN 1 ELSE 0 END AS is_unique, \
                              CASE WHEN INDEX_NAME = 'PRIMARY' THEN 1 ELSE 0 END AS is_primary, \
                              COLLATION, SEQ_IN_INDEX FROM information_schema.STATISTICS \
                       WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? \
                       ORDER BY INDEX_NAME, SEQ_IN_INDEX";
            fetch_all(db, sql, &[Bind::Str(table.to_string())], 0).await?
        }
        Db::Postgres(_) => {
            let sql = "SELECT i.relname AS index_name, a.attname AS column_name, \
                              CASE WHEN ix.indisunique THEN 1 ELSE 0 END AS is_unique, \
                              CASE WHEN ix.indisprimary THEN 1 ELSE 0 END AS is_primary, \
                              '' AS asc_or_desc, k.ord AS ordinal \
                       FROM pg_index ix \
                       JOIN pg_class i ON i.oid = ix.indexrelid \
                       JOIN pg_class t ON t.oid = ix.indrelid \
                       JOIN pg_namespace ns ON ns.oid = t.relnamespace \
                       JOIN LATERAL unnest(ix.indkey) WITH ORDINALITY k(attnum, ord) ON TRUE \
                       JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = k.attnum \
                       WHERE t.relname = ? AND ns.nspname NOT IN ('pg_catalog','information_schema') \
                       ORDER BY i.relname, k.ord";
            fetch_all(db, sql, &[Bind::Str(table.to_string())], 0).await?
        }
        Db::Mssql(_) => {
            let sql = "SELECT i.name AS index_name, c.name AS column_name, \
                              CASE WHEN i.is_unique = 1 THEN 1 ELSE 0 END AS is_unique, \
                              CASE WHEN i.is_primary_key = 1 THEN 1 ELSE 0 END AS is_primary, \
                              CASE WHEN ic.is_descending_key = 1 THEN 'D' ELSE 'A' END AS asc_or_desc, \
                              ic.key_ordinal AS ordinal \
                       FROM sys.indexes i \
                       JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id \
                       JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
                       WHERE i.object_id = OBJECT_ID(@P1) AND i.name IS NOT NULL \
                       ORDER BY i.name, ic.key_ordinal";
            fetch_all(db, sql, &[Bind::Str(table.to_string())], 0).await?
        }
    };

    let mut indexes = Vec::new();
    for row in &rows.rows {
        indexes.push(IndexInfo {
            name: s(&row[0]),
            column_name: s(&row[1]),
            unique: b(&row[2]),
            primary_key: b(&row[3]),
            asc_or_desc: s(&row[4]),
            ordinal: row.get(5).map(|v| n(v) as i32).unwrap_or(0),
        });
    }
    Ok(indexes)
}

/// 把 "VARCHAR(255)"/"numeric(10,2)" 之类拆成 基础类型、长度、小数位
pub fn split_type(full: &str) -> (String, i32, i32) {
    let full = full.trim();
    // 去掉 unsigned / 数组等修饰
    let base_part = full.split('(').next().unwrap_or(full).trim();
    let base = base_part
        .split_whitespace()
        .next()
        .unwrap_or(base_part)
        .to_string();
    let mut len = 0;
    let mut scale = 0;
    if let Some(start) = full.find('(') {
        if let Some(end) = full[start..].find(')') {
            let inner = &full[start + 1..start + end];
            let mut parts = inner.split(',');
            if let Some(l) = parts.next() {
                len = l.trim().parse::<i32>().unwrap_or(0);
            }
            if let Some(sc) = parts.next() {
                scale = sc.trim().parse::<i32>().unwrap_or(0);
            }
        }
    }
    (base, len, scale)
}

/// 去掉 SQL Server 默认值外层的包裹括号
/// 仅当首尾括号确实成对（内部括号平衡）时才剥离，避免破坏 (getdate()) 这类表达式
fn strip_default_wrapper(v: &str) -> String {
    let mut s = v.trim().to_string();
    loop {
        if !(s.starts_with('(') && s.ends_with(')') && s.len() > 2) {
            break;
        }
        let inner = &s[1..s.len() - 1];
        // 判断外层括号是否是唯一包裹：内部括号必须平衡
        let mut depth: i32 = 0;
        let mut balanced = true;
        for ch in inner.chars() {
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth -= 1;
                if depth < 0 {
                    balanced = false;
                    break;
                }
            }
        }
        if balanced && depth == 0 {
            s = inner.trim().to_string();
        } else {
            break;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn splits_type_strings() {
        assert_eq!(split_type("VARCHAR(255)"), ("VARCHAR".to_string(), 255, 0));
        assert_eq!(split_type("DECIMAL(10,2)"), ("DECIMAL".to_string(), 10, 2));
        assert_eq!(split_type("INT"), ("INT".to_string(), 0, 0));
        assert_eq!(split_type("CHARACTER VARYING(50)"), ("CHARACTER".to_string(), 50, 0));
        assert_eq!(split_type("numeric(12,4)"), ("numeric".to_string(), 12, 4));
    }

    #[test]
    fn strips_defaults() {
        assert_eq!(strip_default_wrapper("((0))"), "0");
        assert_eq!(strip_default_wrapper("(getdate())"), "getdate()");
        assert_eq!(strip_default_wrapper("1"), "1");
    }

    #[test]
    fn coerces_values() {
        assert_eq!(s(&json!("a")), "a");
        assert_eq!(s(&json!(null)), "");
        assert_eq!(n(&json!(5)), 5);
        assert_eq!(n(&json!("7")), 7);
        assert!(b(&json!(1)));
        assert!(b(&json!("yes")));
        assert!(!b(&json!(0)));
    }
}
