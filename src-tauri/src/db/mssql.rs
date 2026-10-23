//! SQL Server 驱动（tiberius）
//! sqlx 不支持 SQL Server，因此单独实现一套执行逻辑
//! tiberius 使用 @P1/@P2 形式的命名参数，且每次操作使用独立连接

use crate::db::exec::{Bind, RawResult};
use crate::model::ConnectionConfig;
use futures_util::TryStreamExt;
use serde_json::Value as Json;
use tiberius::{AuthMethod, Client, Config};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

/// 建立到 SQL Server 的连接
async fn connect(config: &ConnectionConfig) -> Result<Client<tokio_util::compat::Compat<TcpStream>>, String> {
    let mut cfg = Config::new();
    cfg.host(&config.host);
    cfg.port(config.effective_port());
    cfg.database(&config.database);
    cfg.authentication(AuthMethod::sql_server(&config.username, &config.password));
    // 本地/内网环境常使用自签证书，默认信任以匹配原 JDBC 的 trustServerCertificate 行为
    cfg.trust_cert();

    let tcp = TcpStream::connect(cfg.get_addr())
        .await
        .map_err(|e| format!("连接 SQL Server 失败: {e}"))?;
    tcp.set_nodelay(true).map_err(|e| e.to_string())?;
    Client::connect(cfg, tcp.compat_write())
        .await
        .map_err(|e| format!("连接 SQL Server 失败: {e}"))
}

/// 把参数收集为 tiberius 可用的 ToSql 对象
fn build_params(binds: &[Bind]) -> Vec<Box<dyn tiberius::ToSql + Send + Sync>> {
    binds
        .iter()
        .map(|b| -> Box<dyn tiberius::ToSql + Send + Sync> {
            match b {
                Bind::Null => Box::new(None::<String>),
                Bind::Str(s) => Box::new(s.clone()),
                Bind::Int(i) => Box::new(*i),
                Bind::Float(f) => Box::new(*f),
                Bind::Bool(v) => Box::new(*v),
            }
        })
        .collect()
}

pub async fn fetch(
    config: &ConnectionConfig,
    sql: &str,
    binds: &[Bind],
    limit: usize,
) -> Result<RawResult, String> {
    let mut client = connect(config).await?;
    let params = build_params(binds);
    let param_refs: Vec<&dyn tiberius::ToSql> = params
        .iter()
        .map(|p| p.as_ref() as &dyn tiberius::ToSql)
        .collect();

    let mut stream = client
        .query(sql, &param_refs)
        .await
        .map_err(|e| format!("执行 SQL 失败: {e}"))?;

    let mut result = RawResult::default();
    let mut columns_ready = false;

    while let Some(item) = stream.try_next().await.map_err(|e| format!("读取结果失败: {e}"))? {
        match item {
            tiberius::QueryItem::Metadata(meta) => {
                result.columns = meta.columns().iter().map(|c| c.name().to_string()).collect();
                result.column_types = meta
                    .columns()
                    .iter()
                    .map(|c| format!("{:?}", c.column_type()).to_uppercase())
                    .collect();
                columns_ready = true;
            }
            tiberius::QueryItem::Row(row) => {
                if !columns_ready {
                    result.columns = row.columns().iter().map(|c| c.name().to_string()).collect();
                    result.column_types = row
                        .columns()
                        .iter()
                        .map(|c| format!("{:?}", c.column_type()).to_uppercase())
                        .collect();
                    columns_ready = true;
                }
                if limit > 0 && result.rows.len() >= limit {
                    result.truncated = true;
                    break;
                }
                result.rows.push(row_to_json(&row));
            }
        }
    }

    drop(stream);
    let _ = client.close().await;
    Ok(result)
}

pub async fn execute(config: &ConnectionConfig, sql: &str, binds: &[Bind]) -> Result<i64, String> {
    let mut client = connect(config).await?;
    let params = build_params(binds);
    let param_refs: Vec<&dyn tiberius::ToSql> = params
        .iter()
        .map(|p| p.as_ref() as &dyn tiberius::ToSql)
        .collect();
    let result = client
        .execute(sql, &param_refs)
        .await
        .map_err(|e| format!("执行 SQL 失败: {e}"))?;
    let affected = result.total();
    let _ = client.close().await;
    Ok(affected as i64)
}

pub async fn test_connect(config: &ConnectionConfig) -> Result<String, String> {
    let mut client = connect(config).await?;
    let stream = client
        .simple_query("SELECT @@VERSION")
        .await
        .map_err(|e| format!("执行测试查询失败: {e}"))?;
    let rows: Vec<tiberius::Row> = stream
        .into_first_result()
        .await
        .map_err(|e| format!("读取测试结果失败: {e}"))?;
    let version = rows
        .first()
        .and_then(|r| r.get::<&str, _>(0))
        .unwrap_or("SQL Server")
        .to_string();
    // @@VERSION 返回多行大段文本，只取首行
    let short = version.lines().next().unwrap_or("SQL Server").trim().to_string();
    let _ = client.close().await;
    Ok(short)
}

/// 把一行 TDS 数据转换为 JSON 数组
/// 时间/数值等类型借助 tiberius 的 FromSql 适配层取值，失败时回退原始值
fn row_to_json(row: &tiberius::Row) -> Vec<Json> {
    let mut out = Vec::with_capacity(row.len());
    for i in 0..row.len() {
        out.push(cell_to_json(row, i));
    }
    out
}

fn cell_to_json(row: &tiberius::Row, i: usize) -> Json {
    // 依次尝试常见 Rust 类型；NULL 会被解析为 None
    if let Ok(Some(v)) = row.try_get::<&str, _>(i) {
        return Json::String(v.to_string());
    }
    if let Ok(Some(v)) = row.try_get::<i64, _>(i) {
        return Json::from(v);
    }
    if let Ok(Some(v)) = row.try_get::<f64, _>(i) {
        return serde_json::Number::from_f64(v)
            .map(Json::Number)
            .unwrap_or(Json::Null);
    }
    if let Ok(Some(v)) = row.try_get::<bool, _>(i) {
        return Json::from(v);
    }
    if let Ok(Some(v)) = row.try_get::<chrono::NaiveDateTime, _>(i) {
        return Json::String(v.to_string());
    }
    if let Ok(Some(v)) = row.try_get::<chrono::NaiveDate, _>(i) {
        return Json::String(v.to_string());
    }
    if let Ok(Some(v)) = row.try_get::<chrono::NaiveTime, _>(i) {
        return Json::String(v.to_string());
    }
    if let Ok(Some(v)) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(i) {
        return Json::String(v.to_rfc3339());
    }
    if let Ok(Some(v)) = row.try_get::<uuid::Uuid, _>(i) {
        return Json::String(v.to_string());
    }
    if let Ok(Some(v)) = row.try_get::<&[u8], _>(i) {
        return Json::String(crate::db::encode_base64(v));
    }
    // 全部失败则判定为 NULL（或无法表示的类型）
    Json::Null
}
