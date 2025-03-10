use std::sync::{Arc, LazyLock};

use anyhow::Error;
use kovi::log::info;
use kovi::serde_json::{self, Value};
use kovi::tokio::sync::RwLock;
use rand::seq::SliceRandom;

type ProblemSet = Arc<Vec<Value>>;

const URL: &str = "https://codeforces.com/api/problemset.problems";
static PROBLEMS: LazyLock<RwLock<Option<ProblemSet>>> = LazyLock::new(|| RwLock::new(None));

pub async fn get_recent_submission(cf_id: &str) -> Option<Value> {
    let res = reqwest::get(&format!(
        "https://codeforces.com/api/user.status?handle={}&count=1",
        cf_id
    ))
    .await
    .ok()?;

    info!("Got response: {:?}", res);

    let body = res.json::<Value>().await.ok()?;
    let status = body["status"].as_str()?;
    if status != "OK" {
        return None;
    }

    let submissions = match &body["result"] {
        Value::Array(submissions) => submissions,
        _ => return None,
    };

    let submission = submissions.get(0).cloned();

    submission
}

async fn fetch_problems() -> Result<Vec<Value>, Error> {
    let res = reqwest::get(URL).await?;
    let body = res.json::<Value>().await?;
    let status = body["status"].as_str().unwrap();
    if status != "OK" {
        return Err(anyhow::anyhow!("Failed to fetch problems"));
    };

    let problems = match &body["result"]["problems"] {
        Value::Array(prblems) => prblems
            .iter()
            .filter(|p| {
                let p = p.as_object().unwrap();
                p.contains_key("rating") && p.contains_key("index") && p.contains_key("contestId")
            })
            .cloned()
            .collect::<Vec<Value>>(),
        _ => return Err(anyhow::anyhow!("Failed to fetch problems")),
    };

    Ok(problems)
}

async fn get_problems() -> Result<ProblemSet, Error> {
    let problems = PROBLEMS.read().await;
    if let Some(problems) = &*problems {
        return Ok(problems.clone());
    }
    drop(problems);
    let problems = fetch_problems().await?;
    let problems = Arc::new(problems);
    PROBLEMS.write().await.replace(problems.clone());
    Ok(problems)
}

pub async fn random_problem() -> Result<serde_json::Value, Error> {
    let problems = get_problems().await?;
    let problem = problems.choose(&mut rand::thread_rng()).unwrap();
    Ok(problem.clone())
}

pub async fn get_daily_problem() -> Result<Value, Error> {
    match crate::sql::get_daily_problem().await {
        Ok(problem) => Ok(serde_json::from_str(&problem)?),
        Err(_) => {
            let problem = random_problem().await?;
            crate::sql::set_daily_problem(&problem.to_string()).await?;
            Ok(problem)
        }
    }
}
