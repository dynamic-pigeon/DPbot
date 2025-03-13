use std::collections::HashSet;
use std::sync::{Arc, LazyLock};

use anyhow::{Error, Result};
use kovi::log::info;
use kovi::serde_json::Value;
use kovi::tokio::sync::{Mutex, RwLock};
use rand::seq::{IteratorRandom, SliceRandom};

use crate::duel::config::MAX_DAILY_RATING;

use super::config::TAGS;

type ProblemSet = Vec<Arc<Problem>>;

const URL: &str = "https://codeforces.com/api/problemset.problems";
static PROBLEMS: LazyLock<RwLock<Option<Arc<ProblemSet>>>> = LazyLock::new(|| RwLock::new(None));

static DAILY_LOC: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Problem {
    pub contest_id: i64,
    pub index: String,
    pub rating: i64,
    pub tags: Vec<String>,
}

impl Problem {
    pub fn from_value(value: &Value) -> Option<Self> {
        let contest_id = value["contestId"].as_i64()?;
        let index = value["index"].as_str()?.to_string();
        let rating = value["rating"].as_i64()?;
        let tags = value["tags"]
            .as_array()?
            .iter()
            .map(|tag| tag.as_str().unwrap().to_string())
            .collect();

        Some(Self {
            contest_id,
            index,
            rating,
            tags,
        })
    }

    pub fn new(contest_id: i64, index: String, rating: i64, tags: Vec<String>) -> Self {
        Self {
            contest_id,
            index,
            rating,
            tags,
        }
    }
}

pub async fn get_problems_by(tags: &[String], rating: i64, qq: i64) -> Result<Vec<Arc<Problem>>> {
    if rating < 800 || rating > 3500 || rating % 100 != 0 {
        return Err(anyhow::anyhow!("rating 应该是 800 到 3500 之间的整数"));
    }

    let mut new = false;
    let mut not_seen = false;

    let tags = tags
        .iter()
        .filter(|tag| {
            if *tag == "new" {
                new = true;
                false
            } else if *tag == "not-seen" {
                not_seen = true;
                false
            } else {
                true
            }
        })
        .map(|tag| tag.replace("_", " "))
        .collect::<Vec<_>>();

    check_tags(&tags)?;

    let problems = get_problems().await?;

    let seen = if not_seen {
        let Some(cf_id) = crate::sql::duel::user::get_user(qq).await?.cf_id else {
            return Err(anyhow::anyhow!(
                "你还没有绑定 CF 账号，不能使用 not-seen 标签"
            ));
        };

        let submissions = get_recent_submissions(&cf_id).await.unwrap_or_default();
        let seen = submissions
            .into_iter()
            .filter(|submission| {
                submission.get("verdict") == Some(&Value::String("OK".to_string()))
            })
            .map(|submission| {
                let problem = submission["problem"].as_object().unwrap();
                let contest_id = problem["contestId"].as_i64().unwrap();
                let index = problem["index"].as_str().unwrap().to_string();
                (contest_id, index)
            })
            .collect::<HashSet<_>>();
        seen
    } else {
        HashSet::new()
    };

    let problems = problems
        .iter()
        // filter by rating
        .filter(|problem| problem.rating == rating)
        // filter by tags
        .filter(|problem| {
            tags.is_empty()
                || tags
                    .iter()
                    .any(|tag| problem.tags.contains(&tag.to_string()))
        })
        // filter by special tags
        .filter(|p| {
            if new && p.contest_id <= 1000 {
                return false;
            }
            if not_seen && seen.contains(&(p.contest_id, p.index.clone())) {
                return false;
            }
            true
        })
        .cloned()
        .collect::<Vec<_>>();

    Ok(problems)
}

fn check_tags(tags: &[String]) -> Result<()> {
    for tag in tags {
        if !TAGS.contains(&tag.as_str()) {
            let similar = TAGS
                .iter()
                .max_by_key(|&&t| {
                    (strsim::normalized_damerau_levenshtein(t, &*tag) * 1000.0) as i64
                })
                .unwrap();

            let diff = strsim::normalized_damerau_levenshtein(similar, tag);
            if diff > 0.6 {
                return Err(anyhow::anyhow!(
                    "{tag} 不是一个合法的标签，你是不是想找 {similar}？"
                ));
            } else {
                return Err(anyhow::anyhow!("{tag} 不是一个合法的标签"));
            }
        }
    }
    Ok(())
}

pub async fn get_recent_submissions(cf_id: &str) -> Option<Vec<Value>> {
    let res = reqwest::get(&format!(
        "https://codeforces.com/api/user.status?handle={}",
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

    match body {
        Value::Object(mut map) => {
            let submissions = map.remove("result")?;
            match submissions {
                Value::Array(submissions) => Some(submissions),
                _ => None,
            }
        }
        _ => unreachable!("Invalid response"),
    }
}

/// 得到用户最近一次提交的信息
pub async fn get_last_submission(cf_id: &str) -> Option<Value> {
    let res = reqwest::get(&format!(
        "https://codeforces.com/api/user.status?handle={}&count=1",
        cf_id
    ))
    .await
    .ok()?;

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

async fn fetch_problems() -> Result<ProblemSet, Error> {
    let client = reqwest::Client::new();
    let mut header = reqwest::header::HeaderMap::new();
    header.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36"),
    );
    let res = client.get(URL).headers(header).send().await?;

    let body = res.json::<Value>().await?;
    let status = body
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if status != "OK" {
        return Err(anyhow::anyhow!("Failed to fetch problems"));
    };

    let problems = match &body.get("result").and_then(|v| v.get("problems")) {
        Some(Value::Array(prblems)) => prblems
            .iter()
            .filter_map(Problem::from_value)
            .map(Arc::new)
            .collect::<Vec<Arc<Problem>>>(),
        _ => return Err(anyhow::anyhow!("Failed to fetch problems")),
    };

    Ok(problems)
}

pub async fn get_problems() -> Result<Arc<ProblemSet>, Error> {
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

#[allow(dead_code)]
pub async fn random_problem() -> Result<Arc<Problem>, Error> {
    let problems = get_problems().await?;
    let problem = problems.choose(&mut rand::thread_rng()).unwrap();
    Ok(problem.clone())
}

pub async fn get_daily_problem() -> Result<Arc<Problem>, Error> {
    let _lock = DAILY_LOC.lock().await;
    match crate::sql::duel::problem::get_daily_problem().await {
        Ok(problem) => Ok(Arc::new(problem)),
        Err(_) => {
            let problem = match get_problems()
                .await?
                .iter()
                .filter(|problem| problem.rating <= MAX_DAILY_RATING)
                .choose(&mut rand::thread_rng())
                .cloned()
            {
                Some(problem) => problem,
                None => return Err(anyhow::anyhow!("没有找到题目，请稍后再试")),
            };
            crate::sql::duel::problem::set_daily_problem(&problem).await?;
            Ok(problem)
        }
    }
}
