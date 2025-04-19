use crate::{
    duel::user::User,
    sql::{POOL, with_commit},
};
use anyhow::{Ok, Result};

pub async fn get_user(qq: i64) -> Result<User> {
    let sql = POOL.get().unwrap();

    let res: User = sqlx::query_as(
        r#"
        SELECT * FROM user WHERE qq = ?
        "#,
    )
    .bind(qq)
    .fetch_one(sql)
    .await?;

    Ok(res)
}

pub async fn update_user(user: &User) -> Result<()> {
    with_commit(async |trans| {
        let _ = sqlx::query(
            r#"
            UPDATE user SET rating = ?, cf_id = ?, daily_score = ?, last_daily = ? WHERE qq = ?
            "#,
        )
        .bind(user.rating)
        .bind(&user.cf_id)
        .bind(user.daily_score)
        .bind(&user.last_daily)
        .bind(user.qq)
        .execute(&mut **trans)
        .await?;

        Ok(())
    })
    .await
}

pub async fn update_two_user_rating(user1: &User, user2: &User) -> Result<()> {
    with_commit(async |trans| {
        let _ = sqlx::query(
            r#"
            UPDATE user SET rating = ? WHERE qq = ?
            "#,
        )
        .bind(user1.rating)
        .bind(user1.qq)
        .execute(&mut **trans)
        .await?;

        let _ = sqlx::query(
            r#"
            UPDATE user SET rating = ? WHERE qq = ?
            "#,
        )
        .bind(user2.rating)
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
    let sql = POOL.get().unwrap();

    let users: Vec<User> = sqlx::query_as(
        r#"
        SELECT * FROM user ORDER BY daily_score DESC LIMIT 20
        "#,
    )
    .fetch_all(sql)
    .await?;

    Ok(users)
}

pub async fn get_top_20_ranklist() -> Result<Vec<User>> {
    let sql = POOL.get().unwrap();

    let users: Vec<User> = sqlx::query_as(
        r#"
        SELECT * FROM user ORDER BY rating DESC LIMIT 20
        "#,
    )
    .fetch_all(sql)
    .await?;

    Ok(users)
}
