use std::sync::LazyLock;

use anyhow::Result;
use kovi::tokio::sync::RwLock;
use kovi::{chrono, serde_json};

use crate::duel::user::User;

pub(crate) mod duel;

static POOL: LazyLock<RwLock<Option<sqlx::SqlitePool>>> = LazyLock::new(|| RwLock::new(None));
static PATH: LazyLock<RwLock<Option<String>>> = LazyLock::new(|| RwLock::new(None));

/// 初始化数据库
/// 有且只有一次，在插件启动时调用
pub async fn init(path: &str) -> Result<()> {
    PATH.write().await.replace(path.to_string());
    connect(path).await?;
    let sql = POOL.write().await;
    let sql = sql.as_ref().unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS daily_problem
        (context_id INTEGER, idx TEXT, rating INTEGER, time TEXT)
        "#,
    )
    .execute(sql)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user
        (qq INTEGER PRIMARY KEY, rating INTEGER, cf_id TEXT, daily_score INTEGER, last_daily TEXT)
        "#,
    )
    .execute(sql)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS duel
        (user1 INTEGER, user2 INTEGER, time TEXT, tags TEXT, rating INTEGER, problem TEXT, result INTEGER)
        "#,
    )
    .execute(sql)
    .await?;

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

async fn with_commit<F, T>(f: F) -> Result<T>
where
    F: AsyncFnOnce(&sqlx::SqlitePool) -> Result<T>,
{
    let sql = POOL.read().await;
    let sql = sql.as_ref().unwrap();

    let trans = sql.begin().await?;

    let ret = f(sql).await?;

    trans.commit().await?;
    Ok(ret)
}
