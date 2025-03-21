use anyhow::Result;

use crate::duel::problem::Problem;
use crate::sql::with_commit;
use crate::utils::today_utc;

pub async fn set_daily_problem(problem: &Problem) -> Result<()> {
    with_commit(async |trans| {
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

        Ok(())
    })
    .await
}

pub async fn get_daily_problem() -> Result<Problem> {
    with_commit(async |trans| {
        let now = today_utc().format("%Y-%m-%d").to_string();

        let res: (i64, String, i64) = sqlx::query_as(
            r#"
        SELECT context_id, idx, rating FROM daily_problem WHERE time = ?
        "#,
        )
        .bind(now)
        .fetch_one(&mut **trans)
        .await?;

        let problem = Problem::new(res.0, res.1, res.2, vec![]);

        Ok(problem)
    })
    .await
}
