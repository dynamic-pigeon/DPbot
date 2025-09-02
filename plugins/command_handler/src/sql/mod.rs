use std::sync::OnceLock;

use anyhow::Result;
use kovi::log::debug;

pub(crate) mod duel;
pub(crate) mod utils;

static POOL: OnceLock<sqlx::SqlitePool> = OnceLock::new();
static PATH: OnceLock<String> = OnceLock::new();

/// 初始化数据库
/// 有且只有一次，在插件启动时调用
/// 理论上多次调用也没有问题，就是会空转，然后白耗费时间
pub async fn init(path: &str) -> Result<()> {
    PATH.get_or_init(|| path.to_string());
    connect(path).await?;
    let sql = POOL.get().unwrap();

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
        (user1 INTEGER, user2 INTEGER, time TEXT, tags TEXT, rating INTEGER, problem TEXT, status TEXT)
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

    debug!("数据库连接成功: {}", path);
    POOL.get_or_init(|| pool);
    Ok(())
}
