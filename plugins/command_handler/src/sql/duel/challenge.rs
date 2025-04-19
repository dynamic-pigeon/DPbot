use kovi::serde_json;

use crate::{
    duel::challenge::Challenge,
    sql::{POOL, with_commit},
};
use anyhow::Result;

pub async fn get_ongoing_challenges() -> Result<Vec<Challenge>> {
    let sql = POOL.get().unwrap();

    let challenges: Vec<Challenge> = sqlx::query_as(
        r#"
        SELECT * FROM duel WHERE status > 0
        "#,
    )
    .fetch_all(sql)
    .await?;

    Ok(challenges)
}

pub async fn add_challenge(challenge: &Challenge) -> Result<()> {
    let time = challenge.time.to_rfc3339();
    let problem = challenge
        .problem
        .as_ref()
        .map(|problem| serde_json::to_string(problem).unwrap());

    with_commit(async |trans| {
        sqlx::query(
            r#"
            INSERT INTO duel (user1, user2, time, tags, rating, problem, status) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(challenge.user1)
        .bind(challenge.user2)
        .bind(time)
        .bind(serde_json::to_string(&challenge.tags).unwrap())
        .bind(challenge.rating)
        .bind(problem)
        .bind(challenge.status)
        .execute(&mut **trans)
        .await?;

        Ok(())
    })
    .await
}

pub async fn change_problem(challenge: &Challenge) -> Result<()> {
    let problem = challenge
        .problem
        .as_ref()
        .map(|problem| serde_json::to_string(problem).unwrap());

    with_commit(async |trans| {
        sqlx::query(
            r#"
        UPDATE duel SET problem = ? WHERE user1 = ? AND user2 = ? AND time = ?
        "#,
        )
        .bind(problem)
        .bind(challenge.user1)
        .bind(challenge.user2)
        .bind(challenge.time.to_rfc3339())
        .execute(&mut **trans)
        .await?;

        Ok(())
    })
    .await
}

#[allow(dead_code)]
pub async fn remove_challenge(challenge: &Challenge) -> Result<()> {
    with_commit(async |trans| {
        sqlx::query(
            r#"
            DELETE FROM duel WHERE user1 = ? AND user2 = ? AND time = ?
            "#,
        )
        .bind(challenge.user1)
        .bind(challenge.user2)
        .bind(challenge.time.to_rfc3339())
        .execute(&mut **trans)
        .await?;

        Ok(())
    })
    .await
}

pub async fn get_chall_ongoing_by_user(user_id: i64) -> Result<Challenge> {
    let sql = POOL.get().unwrap();

    let res: Challenge = sqlx::query_as(
        r#"
        SELECT * FROM duel WHERE (user2 = $1 OR user1 = $1) AND status > 0
        "#,
    )
    .bind(user_id)
    .fetch_one(sql)
    .await?;

    Ok(res)
}

pub async fn get_chall_ongoing_by_2user(user1: i64, user2: i64) -> Result<Challenge> {
    let sql = POOL.get().unwrap();

    let res: Challenge = sqlx::query_as(
        r#"
        SELECT * FROM duel WHERE (user1 = $1 AND user2 = $2) AND status > 0
        "#,
    )
    .bind(user1)
    .bind(user2)
    .fetch_one(sql)
    .await?;

    Ok(res)
}

pub async fn change_status(chall: &Challenge) -> Result<()> {
    with_commit(async |trans| {
        sqlx::query(
            r#"
            UPDATE duel SET status = ? WHERE user1 = ? AND user2 = ? AND time = ?
            "#,
        )
        .bind(chall.status)
        .bind(chall.user1)
        .bind(chall.user2)
        .bind(chall.time.to_rfc3339())
        .execute(&mut **trans)
        .await?;

        Ok(())
    })
    .await
}
