use anyhow::Result;
use kovi::{chrono, serde_json};

use crate::duel::challenge::Challenge;
use crate::duel::user::User;

use super::POOL;

pub async fn get_ongoing_challenges() -> Result<Vec<Challenge>> {
    let sql = POOL.read().await;
    let sql = sql.as_ref().unwrap();

    let res: Vec<(i64, i64, String, String, Option<i64>, i64)> = sqlx::query_as(
        r#"
        SELECT * FROM duel WHERE started = 1
        "#,
    )
    .fetch_all(sql)
    .await?;

    let challenges = res
        .iter()
        .map(|(user1, user2, time, problem, result, started)| {
            let time = chrono::DateTime::parse_from_rfc3339(time)
                .map(|dst| dst.to_utc())
                .unwrap();
            let problem = serde_json::from_str(problem).unwrap();
            Challenge::new(*user1, *user2, time, problem, *result, *started)
        })
        .collect();

    Ok(challenges)
}

pub async fn add_challenge(challenge: &Challenge) -> Result<()> {
    assert!(challenge.started == 1);

    let time = challenge.time.to_rfc3339();
    let problem = serde_json::to_string(&challenge.problem).unwrap();

    let sql = POOL.read().await;
    let sql = sql.as_ref().unwrap();

    let _ = sqlx::query(
        r#"
        INSERT INTO duel (user1, user2, time, problem, result, started) VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(challenge.user1)
    .bind(challenge.user2)
    .bind(time)
    .bind(problem)
    .bind(challenge.result)
    .bind(challenge.started)
    .execute(sql)
    .await?;

    Ok(())
}

pub async fn get_user(qq: i64) -> Result<User> {
    let sql = POOL.read().await;
    let sql = sql.as_ref().unwrap();

    let res: (i64, i64, Option<String>, i64) = sqlx::query_as(
        r#"
        SELECT * FROM user WHERE qq = ?
        "#,
    )
    .bind(qq)
    .fetch_one(sql)
    .await?;

    Ok(User::new(res.0, res.1, res.2, res.3))
}

pub async fn update_user(user: &User) -> Result<()> {
    let sql = POOL.read().await;
    let sql = sql.as_ref().unwrap();

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
}

pub async fn add_user(qq: i64) -> Result<User> {
    let sql = POOL.read().await;
    let sql = sql.as_ref().unwrap();

    let _ = sqlx::query(
        r#"
        INSERT INTO user (qq, rating, cf_id, daily_score) VALUES (?, 1500, NULL, 0)
        "#,
    )
    .bind(qq)
    .execute(sql)
    .await?;

    Ok(User::new(qq, 1500, None, 0))
}

pub async fn set_daily_problem(problem: &str) -> Result<()> {
    let sql = POOL.read().await;
    let sql = sql.as_ref().unwrap();

    let now = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let _ = sqlx::query(
        r#"
        INSERT INTO daily_problem (problem, time) VALUES (?, ?)
        "#,
    )
    .bind(problem)
    .bind(now)
    .execute(sql)
    .await?;

    Ok(())
}

pub async fn get_daily_problem() -> Result<String> {
    let sql = POOL.read().await;
    let sql = sql.as_ref().unwrap();

    let now = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let res: (String,) = sqlx::query_as(
        r#"
        SELECT problem FROM daily_problem WHERE time = ?
        "#,
    )
    .bind(now)
    .fetch_one(sql)
    .await?;

    Ok(res.0)
}
