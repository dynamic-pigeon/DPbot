use kovi::{chrono, serde_json};

use crate::{
    duel::challenge::Challenge,
    sql::{POOL, with_commit},
};
use anyhow::Result;

pub async fn get_ongoing_challenges() -> Result<Vec<Challenge>> {
    let sql = POOL.get().unwrap();

    let res: Vec<(i64, i64, String, String, i64, Option<String>, Option<i64>)> = sqlx::query_as(
        r#"
        SELECT * FROM duel WHERE result IS NULL
        "#,
    )
    .fetch_all(sql)
    .await?;

    let challenges = res
        .into_iter()
        .map(|(user1, user2, time, tags, rating, problem, result)| {
            let time = chrono::DateTime::parse_from_rfc3339(&time)
                .map(|dst| dst.to_utc())
                .unwrap();

            let tags = serde_json::from_str(&tags).unwrap();
            let problem = problem
                .as_ref()
                .map(|problem| serde_json::from_str(problem).unwrap());

            Challenge::new(user1, user2, time, tags, rating, problem, result, 1)
        })
        .collect();

    Ok(challenges)
}

pub async fn add_challenge(challenge: &Challenge) -> Result<()> {
    assert!(challenge.started == 1);

    let time = challenge.time.to_rfc3339();
    let problem = challenge
        .problem
        .as_ref()
        .map(|problem| serde_json::to_string(problem).unwrap());

    with_commit(async |trans| {
        sqlx::query(
            r#"
            INSERT INTO duel (user1, user2, time, tags, rating, problem, result) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(challenge.user1)
        .bind(challenge.user2)
        .bind(time)
        .bind(serde_json::to_string(&challenge.tags).unwrap())
        .bind(challenge.rating)
        .bind(problem)
        .bind(challenge.result)
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

pub async fn change_result(challenge: &Challenge) -> Result<()> {
    with_commit(async |trans| {
        sqlx::query(
            r#"
            UPDATE duel SET result = ? WHERE user1 = ? AND user2 = ? AND time = ?
            "#,
        )
        .bind(challenge.result)
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

    let res: (i64, i64, String, String, i64, Option<String>, Option<i64>) = sqlx::query_as(
        r#"
        SELECT * FROM duel WHERE (user2 = $1 OR user1 = $1) AND result IS NULL
        "#,
    )
    .bind(user_id)
    .fetch_one(sql)
    .await?;

    let time = chrono::DateTime::parse_from_rfc3339(&res.2)
        .map(|dst| dst.to_utc())
        .unwrap();

    let tags = serde_json::from_str(&res.3).unwrap();
    let problem = res
        .5
        .as_ref()
        .map(|problem| serde_json::from_str(problem).unwrap());

    Ok(Challenge::new(
        res.0, res.1, time, tags, res.4, problem, res.6, 1,
    ))
}
