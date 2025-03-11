use kovi::{chrono, serde_json};

use crate::{
    duel::{challenge::Challenge, user::User},
    sql::{POOL, with_commit},
};
use anyhow::{Ok, Result};

pub async fn get_user(qq: i64) -> Result<User> {
    let sql = POOL.read().await;
    let sql = sql.as_ref().unwrap();

    let res: (i64, i64, Option<String>, i64, String) = sqlx::query_as(
        r#"
        SELECT * FROM user WHERE qq = ?
        "#,
    )
    .bind(qq)
    .fetch_one(sql)
    .await?;

    Ok(User::new(res.0, res.1, res.2, res.3, res.4))
}

pub async fn update_user(user: &User) -> Result<()> {
    with_commit(async |sql| {
        let _ = sqlx::query(
            r#"
            UPDATE user SET rating = ?, cf_id = ?, daily_score = ? WHERE qq = ?
            "#,
        )
        .bind(user.rating)
        .bind(&user.cf_id)
        .bind(user.daily_score)
        .bind(user.qq)
        .execute(sql)
        .await?;

        Ok(())
    })
    .await
}

pub async fn add_user(qq: i64) -> Result<User> {
    with_commit(async |sql| {
        let _ = sqlx::query(
            r#"
            INSERT INTO user (qq, rating, cf_id, daily_score, last_daily) VALUES (?, 1500, NULL, 0, "")
            "#,
        )
        .bind(qq)
        .execute(sql)
        .await?;

        Ok(User::new(qq, 1500, None, 0, "".to_string()))
    }).await
}
