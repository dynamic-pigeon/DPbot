use anyhow::Result;

use crate::duel::problem::Problem;
use crate::sql::utils::Commit;
use crate::sql::{POOL, with_commit};
use crate::utils::today_utc;

pub trait CommitProblemExt {
    async fn set_daily_problem(&mut self, problem: &Problem) -> Result<&mut Self>;
}

impl CommitProblemExt for Commit {
    async fn set_daily_problem(&mut self, problem: &Problem) -> Result<&mut Self> {
        let trans = self
            .tx
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Transaction not started"))?;

        let now = today_utc().format("%Y-%m-%d").to_string();

        let _ = sqlx::query(
            r#"
        INSERT INTO daily_problem (context_id, idx, rating, time) VALUES (?, ?, ?, ?)
        "#,
        )
        .bind(problem.contest_id)
        .bind(&problem.index)
        .bind(problem.rating)
        .bind(now)
        .execute(&mut **trans)
        .await?;

        Ok(self)
    }
}

pub async fn get_daily_problem() -> Result<Problem> {
    let sql = POOL.get().unwrap();
    let now = today_utc().format("%Y-%m-%d").to_string();

    let res: (i64, String, i64) = sqlx::query_as(
        r#"
        SELECT context_id, idx, rating FROM daily_problem WHERE time = ?
        "#,
    )
    .bind(now)
    .fetch_one(sql)
    .await?;

    let problem = Problem::new(res.0, res.1, Some(res.2), vec![]);

    Ok(problem)
}
