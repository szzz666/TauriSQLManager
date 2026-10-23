//! SQL 方言：屏蔽不同数据库之间的 SQL 语法差异
//! 对应原项目 database/DatabaseDialect + AbstractDialect

use crate::model::{ColumnDefinition, DbType};

/// 标识符引用方式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteStyle {
    /// 双引号包裹（SQL 标准）
    DoubleQuote,
    /// 反引号包裹（MySQL / MariaDB）
    Backtick,
    /// 方括号包裹（SQL Server）
    Bracket,
}

/// 方言：描述某个数据库的 SQL 语法特征
#[derive(Debug, Clone, Copy)]
pub struct Dialect {
    pub db_type: DbType,
    pub quote: QuoteStyle,
}

impl Dialect {
    pub fn of(db_type: DbType) -> Self {
        let quote = match db_type {
            DbType::Mysql | DbType::Mariadb => QuoteStyle::Backtick,
            DbType::Mssql => QuoteStyle::Bracket,
            _ => QuoteStyle::DoubleQuote,
        };
        Self { db_type, quote }
    }

    /// 引用标识符，防止关键字冲突
    pub fn quote_identifier(&self, name: &str) -> String {
        match self.quote {
            QuoteStyle::DoubleQuote => format!("\"{}\"", name.replace('"', "\"\"")),
            QuoteStyle::Backtick => {
                let bt = '\u{60}';
                let doubled = bt.to_string().repeat(2);
                format!("{bt}{}{bt}", name.replace(bt, &doubled))
            }
            QuoteStyle::Bracket => format!("[{}]", name.replace(']', "]]")),
        }
    }

    /// 引用字符串字面量
    pub fn quote_literal(&self, value: &str) -> String {
        format!("'{}'", value.replace('\'', "''"))
    }

    /// 生成分页 SQL
    pub fn build_paged_sql(&self, sql: &str, offset: i64, limit: i64) -> String {
        match self.db_type {
            DbType::Mysql | DbType::Mariadb => {
                if offset <= 0 {
                    format!("{sql} LIMIT {limit}")
                } else {
                    format!("{sql} LIMIT {offset}, {limit}")
                }
            }
            DbType::Mssql => {
                // SQL Server 的 OFFSET/FETCH 必须配合 ORDER BY
                let trimmed = sql.trim_end();
                let has_order_by = trimmed.to_uppercase().contains("ORDER BY");
                let base = if has_order_by {
                    trimmed.to_string()
                } else {
                    format!("{trimmed} ORDER BY (SELECT NULL)")
                };
                format!("{base} OFFSET {offset} ROWS FETCH NEXT {limit} ROWS ONLY")
            }
            _ => {
                // SQLite / PostgreSQL
                if offset <= 0 {
                    format!("{sql} LIMIT {limit}")
                } else {
                    format!("{sql} LIMIT {limit} OFFSET {offset}")
                }
            }
        }
    }
/// 生成列定义片段（建表与加列共用）
    pub fn build_column_def(&self, col: &ColumnDefinition) -> String {
        let mut sb = String::new();
        sb.push_str(&self.quote_identifier(&col.name));
        sb.push(' ');

        let type_name = if col.col_type.trim().is_empty() {
            "VARCHAR"
        } else {
            col.col_type.trim()
        };

        // PostgreSQL 自增使用 SERIAL 伪类型
        if self.db_type == DbType::Postgresql && col.auto_increment {
            let is_big = type_name.to_uppercase().contains("BIG");
            sb.push_str(if is_big { "BIGSERIAL" } else { "SERIAL" });
        } else {
            sb.push_str(&type_name.to_uppercase());
            if col.length > 0 {
                sb.push('(');
                sb.push_str(&col.length.to_string());
                if col.scale > 0 {
                    sb.push(',');
                    sb.push_str(&col.scale.to_string());
                }
                sb.push(')');
            }
            // 各数据库的自增语法不同
            if col.auto_increment {
                match self.db_type {
                    DbType::Mysql | DbType::Mariadb => sb.push_str(" AUTO_INCREMENT"),
                    // SQLite 的 AUTOINCREMENT 必须紧跟 "INTEGER PRIMARY KEY"
                    DbType::Sqlite => sb.push_str(" PRIMARY KEY AUTOINCREMENT"),
                    DbType::Mssql => sb.push_str(" IDENTITY(1,1)"),
                    DbType::Postgresql => {}
                }
            }
        }
        if !col.nullable {
            sb.push_str(" NOT NULL");
        }
        if col.unique {
            sb.push_str(" UNIQUE");
        }
        if let Some(dv) = &col.default_value {
            if !dv.trim().is_empty() {
                sb.push_str(" DEFAULT ");
                sb.push_str(dv.trim());
            }
        }
        // MySQL 支持列注释
        if matches!(self.db_type, DbType::Mysql | DbType::Mariadb) {
            if let Some(c) = &col.comment {
                if !c.trim().is_empty() {
                    sb.push_str(" COMMENT ");
                    sb.push_str(&self.quote_literal(c));
                }
            }
        }
        sb
    }

    /// 生成建表 SQL
    pub fn build_create_table_sql(
        &self,
        table_name: &str,
        columns: &[ColumnDefinition],
        comment: Option<&str>,
    ) -> String {
        let mut defs: Vec<String> = Vec::new();
        let mut pk_cols: Vec<String> = Vec::new();

        for col in columns {
            defs.push(format!("  {}", self.build_column_def(col)));
            if col.primary_key {
                pk_cols.push(self.quote_identifier(&col.name));
            }
        }

        // SQLite 的自增主键必须内联声明，避免重复
        let inline_pk = self.db_type == DbType::Sqlite && columns.iter().any(|c| c.auto_increment);
        if !pk_cols.is_empty() && !inline_pk {
            defs.push(format!("  PRIMARY KEY ({})", pk_cols.join(", ")));
        }

        let mut sql = format!(
            "CREATE TABLE {} (\n{}\n)",
            self.quote_identifier(table_name),
            defs.join(",\n")
        );

        if matches!(self.db_type, DbType::Mysql | DbType::Mariadb) {
            sql.push_str(" ENGINE=InnoDB DEFAULT CHARSET=utf8mb4");
            if let Some(c) = comment {
                if !c.trim().is_empty() {
                    sql.push_str(" COMMENT=");
                    sql.push_str(&self.quote_literal(c));
                }
            }
        }
        sql
    }

    pub fn build_drop_table_sql(&self, table_name: &str) -> String {
        format!("DROP TABLE IF EXISTS {}", self.quote_identifier(table_name))
    }

    pub fn build_rename_table_sql(&self, old_name: &str, new_name: &str) -> String {
        if self.db_type == DbType::Mssql {
            // SQL Server 使用系统存储过程重命名
            return format!(
                "EXEC sp_rename {}, {}",
                self.quote_literal(old_name),
                self.quote_literal(new_name)
            );
        }
        format!(
            "ALTER TABLE {} RENAME TO {}",
            self.quote_identifier(old_name),
            self.quote_identifier(new_name)
        )
    }

    pub fn build_truncate_table_sql(&self, table_name: &str) -> String {
        match self.db_type {
            DbType::Sqlite => format!("DELETE FROM {}", self.quote_identifier(table_name)),
            _ => format!("TRUNCATE TABLE {}", self.quote_identifier(table_name)),
        }
    }

    pub fn build_add_column_sql(&self, table_name: &str, col: &ColumnDefinition) -> String {
        format!(
            "ALTER TABLE {} ADD {}",
            self.quote_identifier(table_name),
            self.build_column_def(col)
        )
    }

    pub fn build_drop_column_sql(&self, table_name: &str, column: &str) -> String {
        format!(
            "ALTER TABLE {} DROP COLUMN {}",
            self.quote_identifier(table_name),
            self.quote_identifier(column)
        )
    }
pub fn build_modify_column_sql(
        &self,
        table_name: &str,
        col: &ColumnDefinition,
    ) -> Result<String, String> {
        let table = self.quote_identifier(table_name);
        let column = self.quote_identifier(&col.name);
        match self.db_type {
            DbType::Sqlite => Err(
                "SQLite 不支持直接修改列定义，请删除该列后重新添加，或重建表".to_string(),
            ),
            DbType::Mysql | DbType::Mariadb => Ok(format!(
                "ALTER TABLE {table} MODIFY COLUMN {}",
                self.build_column_def(col)
            )),
            DbType::Mssql => Ok(format!(
                "ALTER TABLE {table} ALTER COLUMN {}",
                self.build_column_def(col)
            )),
            DbType::Postgresql => {
                let mut s = format!(
                    "ALTER TABLE {table} ALTER COLUMN {column} TYPE {}",
                    if col.col_type.trim().is_empty() {
                        "VARCHAR".to_string()
                    } else {
                        col.col_type.trim().to_uppercase()
                    }
                );
                if col.length > 0 {
                    s.push('(');
                    s.push_str(&col.length.to_string());
                    if col.scale > 0 {
                        s.push(',');
                        s.push_str(&col.scale.to_string());
                    }
                    s.push(')');
                }
                if col.nullable {
                    s.push_str(&format!(
                        "; ALTER TABLE {table} ALTER COLUMN {column} DROP NOT NULL"
                    ));
                } else {
                    s.push_str(&format!(
                        "; ALTER TABLE {table} ALTER COLUMN {column} SET NOT NULL"
                    ));
                }
                Ok(s)
            }
        }
    }

    pub fn build_rename_column_sql(&self, table_name: &str, old: &str, new: &str) -> String {
        if self.db_type == DbType::Mssql {
            return format!(
                "EXEC sp_rename {}, {}, 'COLUMN'",
                self.quote_literal(&format!("{table_name}.{old}")),
                self.quote_literal(new)
            );
        }
        format!(
            "ALTER TABLE {} RENAME COLUMN {} TO {}",
            self.quote_identifier(table_name),
            self.quote_identifier(old),
            self.quote_identifier(new)
        )
    }

    /// 生成 INSERT 语句；mode: insert / replace / ignore
    /// conflict_cols 为冲突判定的键（通常是主键），PostgreSQL 的 ON CONFLICT 需要它
    /// SQL Server 不支持 REPLACE/IGNORE 语法，这里只生成普通 INSERT，
    /// 冲突处理由上层按行回退（见 service::import_data）
    pub fn build_insert_sql(
        &self,
        table_name: &str,
        columns: &[String],
        mode: &str,
        conflict_cols: &[String],
    ) -> String {
        let mode = mode.to_lowercase();
        let prefix = match (self.db_type, mode.as_str()) {
            (DbType::Sqlite, "replace") => "INSERT OR REPLACE INTO",
            (DbType::Sqlite, "ignore") => "INSERT OR IGNORE INTO",
            (DbType::Mysql | DbType::Mariadb, "replace") => "REPLACE INTO",
            (DbType::Mysql | DbType::Mariadb, "ignore") => "INSERT IGNORE INTO",
            _ => "INSERT INTO",
        };
        let cols = columns
            .iter()
            .map(|c| self.quote_identifier(c))
            .collect::<Vec<_>>()
            .join(", ");
        let placeholders = (1..=columns.len())
            .map(|i| match self.db_type {
                DbType::Mssql => format!("@P{i}"),
                _ => "?".to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let mut sql = format!(
            "{prefix} {} ({cols}) VALUES ({placeholders})",
            self.quote_identifier(table_name)
        );

        // PostgreSQL 的冲突处理使用 ON CONFLICT 子句
        if self.db_type == DbType::Postgresql {
            match mode.as_str() {
                "ignore" => sql.push_str(" ON CONFLICT DO NOTHING"),
                "replace" if !conflict_cols.is_empty() => {
                    let target = conflict_cols
                        .iter()
                        .map(|c| self.quote_identifier(c))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let updates = columns
                        .iter()
                        .filter(|c| !conflict_cols.iter().any(|k| k == *c))
                        .map(|c| {
                            format!(
                                "{} = EXCLUDED.{}",
                                self.quote_identifier(c),
                                self.quote_identifier(c)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    sql.push_str(&format!(" ON CONFLICT ({target}) "));
                    if updates.is_empty() {
                        sql.push_str("DO NOTHING");
                    } else {
                        sql.push_str(&format!("DO UPDATE SET {updates}"));
                    }
                }
                _ => {}
            }
        }
        sql
    }


    pub fn build_update_sql(
        &self,
        table_name: &str,
        columns: &[String],
        where_clause: Option<&str>,
    ) -> String {
        let sets = columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let placeholder = match self.db_type {
                    DbType::Mssql => format!("@P{}", i + 1),
                    _ => "?".to_string(),
                };
                format!("{} = {placeholder}", self.quote_identifier(c))
            })
            .collect::<Vec<_>>()
            .join(", ");
        let mut sql = format!("UPDATE {} SET {sets}", self.quote_identifier(table_name));
        if let Some(w) = where_clause {
            if !w.trim().is_empty() {
                sql.push_str(" WHERE ");
                sql.push_str(w);
            }
        }
        sql
    }

    pub fn build_delete_sql(&self, table_name: &str, where_clause: Option<&str>) -> String {
        let mut sql = format!("DELETE FROM {}", self.quote_identifier(table_name));
        if let Some(w) = where_clause {
            if !w.trim().is_empty() {
                sql.push_str(" WHERE ");
                sql.push_str(w);
            }
        }
        sql
    }

    /// 该方言下常用的列类型，用于建表下拉
    pub fn column_types(&self) -> Vec<&'static str> {
        match self.db_type {
            DbType::Sqlite => vec![
                "INTEGER", "TEXT", "REAL", "BLOB", "NUMERIC", "BOOLEAN", "DATE", "DATETIME",
                "VARCHAR", "BIGINT", "DOUBLE", "DECIMAL",
            ],
            DbType::Mysql | DbType::Mariadb => vec![
                "INT", "BIGINT", "SMALLINT", "TINYINT", "DECIMAL", "FLOAT", "DOUBLE", "VARCHAR",
                "TEXT", "LONGTEXT", "CHAR", "DATE", "DATETIME", "TIMESTAMP", "TIME", "BOOLEAN",
                "JSON", "BLOB",
            ],
            DbType::Postgresql => vec![
                "INTEGER", "BIGINT", "SMALLINT", "SERIAL", "BIGSERIAL", "NUMERIC", "REAL",
                "DOUBLE PRECISION", "VARCHAR", "TEXT", "CHAR", "BOOLEAN", "DATE", "TIMESTAMP",
                "TIMESTAMPTZ", "TIME", "JSON", "JSONB", "UUID", "BYTEA",
            ],
            DbType::Mssql => vec![
                "INT", "BIGINT", "SMALLINT", "TINYINT", "DECIMAL", "NUMERIC", "FLOAT", "REAL",
                "VARCHAR", "NVARCHAR", "CHAR", "NCHAR", "TEXT", "NTEXT", "BIT", "DATE", "DATETIME",
                "DATETIME2", "TIME", "UNIQUEIDENTIFIER", "VARBINARY",
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, ty: &str) -> ColumnDefinition {
        ColumnDefinition {
            name: name.to_string(),
            col_type: ty.to_string(),
            nullable: true,
            ..Default::default()
        }
    }

    #[test]
    fn quotes_identifiers_per_dialect() {
        let bt = '\u{60}';
        assert_eq!(
            Dialect::of(DbType::Mysql).quote_identifier(&format!("a{bt}b")),
            format!("{bt}a{bt}{bt}b{bt}")
        );
        assert_eq!(Dialect::of(DbType::Mssql).quote_identifier("a]b"), "[a]]b]");
        assert_eq!(
            Dialect::of(DbType::Postgresql).quote_identifier("a\"b"),
            "\"a\"\"b\""
        );
    }

    #[test]
    fn paginates_per_dialect() {
        let sql = "SELECT * FROM t";
        assert_eq!(
            Dialect::of(DbType::Mysql).build_paged_sql(sql, 20, 10),
            "SELECT * FROM t LIMIT 20, 10"
        );
        assert_eq!(
            Dialect::of(DbType::Mysql).build_paged_sql(sql, 0, 10),
            "SELECT * FROM t LIMIT 10"
        );
        assert_eq!(
            Dialect::of(DbType::Postgresql).build_paged_sql(sql, 20, 10),
            "SELECT * FROM t LIMIT 10 OFFSET 20"
        );
        assert_eq!(
            Dialect::of(DbType::Sqlite).build_paged_sql(sql, 0, 10),
            "SELECT * FROM t LIMIT 10"
        );
        assert_eq!(
            Dialect::of(DbType::Mssql).build_paged_sql(sql, 20, 10),
            "SELECT * FROM t ORDER BY (SELECT NULL) OFFSET 20 ROWS FETCH NEXT 10 ROWS ONLY"
        );
    }

    #[test]
    fn builds_auto_increment_per_dialect() {
        let mut c = col("id", "INT");
        c.auto_increment = true;
        c.primary_key = true;
        assert!(Dialect::of(DbType::Mysql)
            .build_column_def(&c)
            .contains("AUTO_INCREMENT"));
        assert!(Dialect::of(DbType::Sqlite)
            .build_column_def(&c)
            .contains("AUTOINCREMENT"));
        assert!(Dialect::of(DbType::Mssql)
            .build_column_def(&c)
            .contains("IDENTITY(1,1)"));
        assert!(Dialect::of(DbType::Postgresql)
            .build_column_def(&c)
            .contains("SERIAL"));
    }

    #[test]
    fn builds_insert_modes() {
        let cols = vec!["a".to_string(), "b".to_string()];
        let no_pk: Vec<String> = Vec::new();
        assert_eq!(
            Dialect::of(DbType::Sqlite).build_insert_sql("t", &cols, "insert", &no_pk),
            "INSERT INTO \"t\" (\"a\", \"b\") VALUES (?, ?)"
        );
        assert!(Dialect::of(DbType::Sqlite)
            .build_insert_sql("t", &cols, "replace", &no_pk)
            .starts_with("INSERT OR REPLACE"));
        assert!(Dialect::of(DbType::Mysql)
            .build_insert_sql("t", &cols, "ignore", &no_pk)
            .starts_with("INSERT IGNORE"));
        assert!(Dialect::of(DbType::Mssql)
            .build_insert_sql("t", &cols, "insert", &no_pk)
            .contains("@P1, @P2"));
    }

    #[test]
    fn postgres_uses_on_conflict_instead_of_or_ignore() {
        let cols = vec!["a".to_string(), "b".to_string()];
        let no_pk: Vec<String> = Vec::new();

        // PostgreSQL 不认识 INSERT OR IGNORE / REPLACE 语法
        let ignore = Dialect::of(DbType::Postgresql).build_insert_sql("t", &cols, "ignore", &no_pk);
        assert!(ignore.starts_with("INSERT INTO"), "实际: {ignore}");
        assert!(ignore.ends_with("ON CONFLICT DO NOTHING"), "实际: {ignore}");

        let pk = vec!["a".to_string()];
        let replace =
            Dialect::of(DbType::Postgresql).build_insert_sql("t", &cols, "replace", &pk);
        assert!(replace.contains("ON CONFLICT (\"a\")"), "实际: {replace}");
        assert!(replace.contains("DO UPDATE SET \"b\" = EXCLUDED.\"b\""), "实际: {replace}");

        // SQL Server 保持普通 INSERT，冲突由上层逐行回退处理
        let mssql =
            Dialect::of(DbType::Mssql).build_insert_sql("t", &cols, "replace", &pk);
        assert!(mssql.starts_with("INSERT INTO"), "实际: {mssql}");
        assert!(!mssql.contains("OR REPLACE"));
    }


    #[test]
    fn create_table_includes_primary_key() {
        let mut id = col("id", "INT");
        id.primary_key = true;
        let sql =
            Dialect::of(DbType::Mysql).build_create_table_sql("t", &[id, col("name", "varchar")], None);
        assert!(sql.contains("PRIMARY KEY"));
        assert!(sql.contains("ENGINE=InnoDB"));
    }

    #[test]
    fn sqlite_autoincrement_is_inline_primary_key() {
        let mut id = col("id", "INTEGER");
        id.auto_increment = true;
        id.primary_key = true;
        id.nullable = false;

        let ddl = Dialect::of(DbType::Sqlite).build_create_table_sql("t", &[id], None);
        // AUTOINCREMENT 必须紧跟 PRIMARY KEY，否则 SQLite 报语法错误
        assert!(
            ddl.contains("\"id\" INTEGER PRIMARY KEY AUTOINCREMENT"),
            "实际: {ddl}"
        );
        // 不能再出现独立的表级 PRIMARY KEY，避免重复定义
        assert!(!ddl.contains("PRIMARY KEY (\"id\")"), "实际: {ddl}");
    }

    #[test]
    fn mssql_rename_uses_sp_rename() {
        let sql = Dialect::of(DbType::Mssql).build_rename_table_sql("old", "new");
        assert!(sql.starts_with("EXEC sp_rename"));
    }
}
