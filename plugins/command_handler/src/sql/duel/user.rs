use crate::{
    duel::user::User,
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
    with_commit(async |trans| {
        let _ = sqlx::query(
            r#"
            UPDATE user SET rating = ?, cf_id = ?, daily_score = ? WHERE qq = ?
            "#,
        )
        .bind(user.rating)
        .bind(&user.cf_id)
        .bind(user.daily_score)
        .bind(user.qq)
        .execute(&mut **trans)
        .await?;

        Ok(())
    })
    .await
}

pub async fn update_two_user(user1: &User, user2: &User) -> Result<()> {
    with_commit(async |trans| {
        let _ = sqlx::query(
            r#"
            UPDATE user SET rating = ?, cf_id = ?, daily_score = ? WHERE qq = ?
            "#,
        )
        .bind(user1.rating)
        .bind(&user1.cf_id)
        .bind(user1.daily_score)
        .bind(user1.qq)
        .execute(&mut **trans)
        .await?;

        let _ = sqlx::query(
            r#"
            UPDATE user SET rating = ?, cf_id = ?, daily_score = ? WHERE qq = ?
            "#,
        )
        .bind(user2.rating)
        .bind(&user2.cf_id)
        .bind(user2.daily_score)
        .bind(user2.qq)
        .execute(&mut **trans)
        .await?;

        Ok(())
    })
    .await
}

pub async fn add_user(qq: i64) -> Result<User> {
    with_commit(async |trans| {
        let _ = sqlx::query(
            r#"
            INSERT INTO user (qq, rating, cf_id, daily_score, last_daily) VALUES (?, 1500, NULL, 0, "")
            "#,
        )
        .bind(qq)
        .execute(&mut **trans)
        .await?;

        Ok(User::new(qq, 1500, None, 0, "".to_string()))
    })
    .await
}

pub async fn get_top_20_daily() -> Result<Vec<User>> {
    let sql = POOL.read().await;
    let sql = sql.as_ref().unwrap();

    let res: Vec<(i64, i64, Option<String>, i64, String)> = sqlx::query_as(
        r#"
        SELECT * FROM user ORDER BY daily_score DESC LIMIT 20
        "#,
    )
    .fetch_all(sql)
    .await?;

    Ok(res
        .into_iter()
        .map(|(qq, rating, cf_id, daily_score, last_daily)| {
            User::new(qq, rating, cf_id, daily_score, last_daily)
        })
        .collect())
}

pub async fn get_top_20_ranklist() -> Result<Vec<User>> {
    let sql = POOL.read().await;
    let sql = sql.as_ref().unwrap();

    let res: Vec<(i64, i64, Option<String>, i64, String)> = sqlx::query_as(
        r#"
        SELECT * FROM user ORDER BY rating DESC LIMIT 20
        "#,
    )
    .fetch_all(sql)
    .await?;

    Ok(res
        .into_iter()
        .map(|(qq, rating, cf_id, daily_score, last_daily)| {
            User::new(qq, rating, cf_id, daily_score, last_daily)
        })
        .collect())
}
