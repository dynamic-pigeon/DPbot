use std::sync::LazyLock;

use anyhow::Result;
use kovi::tokio::sync::RwLock;
use kovi::{chrono, serde_json};

use crate::duel::user::User;

mod duel;
pub(crate) use duel::*;

static POOL: LazyLock<RwLock<Option<sqlx::SqlitePool>>> = LazyLock::new(|| RwLock::new(None));
static PATH: LazyLock<RwLock<Option<String>>> = LazyLock::new(|| RwLock::new(None));

/// 初始化数据库
/// 有且只有一次，在插件启动时调用
pub async fn init(path: &str) -> Result<()> {
    PATH.write().await.replace(path.to_string());
    connect(path).await?;
    let sql = POOL.write().await;
    let sql = sql.as_ref().unwrap();

    let _ = sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS daily_problem
        (problem TEXT, time TEXT)
        "#,
    )
    .fetch_one(sql)
    .await;

    let _ = sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user
        (qq INTEGER PRIMARY KEY, rating INTEGER, cf_id TEXT, daily_score INTEGER)
        "#,
    )
    .fetch_one(sql)
    .await;

    let _ = sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS duel
        (user1 INTEGER, user2 INTEGER, time TEXT, problem TEXT, result INTEGER, started INTEGER)
        "#,
    )
    .fetch_one(sql)
    .await;

    Ok(())
}

pub async fn connect(path: &str) -> Result<()> {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(path)
        .await?;
    POOL.write().await.replace(pool);
    Ok(())
}
