use crate::{
    duel::user::User,
    sql::{POOL, utils::Commit, with_commit},
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

pub trait CommitUserExt {
    async fn update_user(&mut self, user: &User) -> Result<&mut Self>;
    async fn update_user_rating(&mut self, user: &User) -> Result<&mut Self>;
    async fn add_user(&mut self, qq: i64) -> Result<&mut Self>;
}

impl CommitUserExt for Commit {
    async fn update_user(&mut self, user: &User) -> Result<&mut Self> {
        let trans = self
            .tx
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Transaction not started"))?;

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

        Ok(self)
    }

    async fn update_user_rating(&mut self, user: &User) -> Result<&mut Self> {
        let trans = self
            .tx
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Transaction not started"))?;

        let _ = sqlx::query(
            r#"
            UPDATE user SET rating = ? WHERE qq = ?
            "#,
        )
        .bind(user.rating)
        .bind(user.qq)
        .execute(&mut **trans)
        .await?;

        Ok(self)
    }

    async fn add_user(&mut self, qq: i64) -> Result<&mut Self> {
        let trans = self
            .tx
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Transaction not started"))?;

        let _ = sqlx::query(
            r#"
            INSERT INTO user (qq, rating, cf_id, daily_score, last_daily) VALUES (?, 1500, NULL, 0, "")
            "#,
        )
        .bind(qq)
        .execute(&mut **trans)
        .await?;

        Ok(self)
    }
}
