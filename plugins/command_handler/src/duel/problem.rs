use std::collections::HashSet;
use std::sync::{Arc, LazyLock};

use anyhow::{Error, Result};
use kovi::log::info;
use kovi::serde_json::{self, Value};
use kovi::tokio::sync::{Mutex, RwLock};
use rand::seq::{IndexedRandom, IteratorRandom};

use crate::duel::config::MAX_DAILY_RATING;
use crate::error::SubmissionError;
use crate::sql::duel::problem::CommitProblemExt;
use crate::sql::utils::Commit;

use super::config::TAGS;
use crate::utils::fetch;

type ProblemSet = Vec<Arc<Problem>>;

const URL: &str = "https://codeforces.com/api/problemset.problems";
static PROBLEMS: LazyLock<RwLock<Option<Arc<ProblemSet>>>> = LazyLock::new(|| RwLock::new(None));

static DAILY_LOC: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct Problem {
    #[serde(rename = "contestId")]
    pub contest_id: i64,
    pub index: String,
    pub rating: Option<i64>,
    pub tags: Vec<String>,
}

impl Problem {
    pub fn new(contest_id: i64, index: String, rating: Option<i64>, tags: Vec<String>) -> Self {
        Self {
            contest_id,
            index,
            rating,
            tags,
        }
    }

    #[inline]
    pub fn same_problem(&self, other: &Self) -> bool {
        self.contest_id == other.contest_id && self.index == other.index
    }
}

pub async fn get_problems_by(tags: &[String], rating: i64, qq: i64) -> Result<ProblemSet> {
    if !(800..=3500).contains(&rating) || rating % 100 != 0 {
        return Err(anyhow::anyhow!("rating 应该是 800 到 3500 之间的整数"));
    }

    let tags = tags
        .iter()
        .map(|tag| tag.replace("_", " "))
        .collect::<Vec<_>>();

    let (pos_tags, nag_tags) = {
        let mut pos_tags = Vec::new();
        let mut nag_tags = Vec::new();
        for tag in tags.iter() {
            if let Some(tag) = tag.strip_prefix('!') {
                nag_tags.push(tag);
            } else {
                pos_tags.push(tag.as_str());
            }
        }
        (pos_tags, nag_tags)
    };

    check_tags(&pos_tags)?;
    check_tags(&nag_tags)?;

    let problems = get_problems().await?;

    let problems = problems
        .iter()
        .filter(|problem| problem.rating == Some(rating))
        .filter(filter_by_nag(&nag_tags, qq).await?)
        .filter(filter_by_pos(&pos_tags, qq).await?)
        .cloned()
        .collect::<Vec<_>>();

    Ok(problems)
}

async fn filter_by_nag(tags: &[&str], qq: i64) -> Result<impl FnMut(&&Arc<Problem>) -> bool> {
    let (new, not_seen, seen, tags) = filter_help(tags, qq).await?;

    let filter = move |problem: &&Arc<Problem>| -> bool {
        // filter by tags
        if !tags.is_empty()
            && tags
                .iter()
                .any(|tag| problem.tags.contains(&tag.to_string()))
        {
            return false;
        }
        // filter by special tags
        if new && problem.contest_id > 1000 {
            return false;
        }
        if not_seen && !seen.contains(&(problem.contest_id, problem.index.clone())) {
            return false;
        }
        true
    };

    Ok(filter)
}

async fn filter_by_pos(tags: &[&str], qq: i64) -> Result<impl FnMut(&&Arc<Problem>) -> bool> {
    let (new, not_seen, seen, tags) = filter_help(tags, qq).await?;

    let filter = move |problem: &&Arc<Problem>| -> bool {
        // filter by tags
        if !tags.is_empty()
            && !tags
                .iter()
                .all(|tag| problem.tags.contains(&tag.to_string()))
        {
            return false;
        }
        // filter by special tags
        if new && problem.contest_id <= 1000 {
            return false;
        }
        if not_seen && seen.contains(&(problem.contest_id, problem.index.clone())) {
            return false;
        }
        true
    };

    Ok(filter)
}

/// 过滤掉 new 和 not-seen 标签
/// 以及不合法的标签
/// 返回值：
/// - new: 是否有 new 标签
/// - not_seen: 是否有 not-seen 标签
/// - seen: 已经提交过的题目
/// - tags: 过滤后的标签
async fn filter_help<'a>(
    tags: &[&'a str],
    qq: i64,
) -> Result<(bool, bool, HashSet<(i64, String)>, Vec<&'a str>)> {
    let mut new = false;
    let mut not_seen = false;

    let tags = tags
        .iter()
        .filter(|tag| {
            if **tag == "new" {
                new = true;
                false
            } else if **tag == "not-seen" {
                not_seen = true;
                false
            } else {
                true
            }
        })
        .cloned()
        .collect::<Vec<_>>();

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
            .filter_map(|submission| {
                let problem = submission.get("problem")?.as_object()?;
                let contest_id = problem.get("contestId")?.as_i64()?;
                let index = problem.get("index")?.as_str()?.to_string();
                Some((contest_id, index))
            })
            .collect::<HashSet<_>>();
        seen
    } else {
        HashSet::new()
    };

    Ok((new, not_seen, seen, tags))
}

fn check_tags(tags: &[&str]) -> Result<()> {
    for tag in tags {
        if !TAGS.contains(tag) {
            let similar = TAGS
                .iter()
                .max_by_key(|&&t| (strsim::normalized_damerau_levenshtein(t, tag) * 1000.0) as i64)
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

pub async fn get_recent_submissions(cf_id: &str) -> Result<Vec<Value>, SubmissionError> {
    let res = fetch(&format!(
        "https://codeforces.com/api/user.status?handle={}",
        cf_id
    ))
    .await
    .map_err(|_| SubmissionError::FetchError)?;

    info!("Got response: {:?}", res);

    let body = res
        .json::<Value>()
        .await
        .map_err(|_| SubmissionError::FetchError)?;
    let status = body["status"]
        .as_str()
        .ok_or_else(|| SubmissionError::FetchError)?;
    if status != "OK" {
        return Err(SubmissionError::FetchError);
    }

    match body {
        Value::Object(mut map) => match map.remove("result") {
            Some(Value::Array(submissions)) => Ok(submissions),
            _ => Err(SubmissionError::NoSubmission),
        },
        _ => unreachable!("Invalid response"),
    }
}

/// 得到用户最近一次提交的信息
pub async fn get_last_submission(cf_id: &str) -> Result<Value, SubmissionError> {
    let res = fetch(&format!(
        "https://codeforces.com/api/user.status?handle={}&count=1",
        cf_id
    ))
    .await
    .map_err(|_| SubmissionError::FetchError)?;

    let body = res
        .json::<Value>()
        .await
        .map_err(|_| SubmissionError::FetchError)?;

    let status = body["status"]
        .as_str()
        .ok_or_else(|| SubmissionError::FetchError)?;
    if status != "OK" {
        return Err(SubmissionError::FetchError);
    }

    match body {
        Value::Object(mut map) => match map.remove("result") {
            Some(Value::Array(mut submissions)) => {
                // 获取最近一次提交
                assert!(submissions.len() <= 1);
                submissions.pop().ok_or(SubmissionError::NoSubmission)
            }
            _ => Err(SubmissionError::NoSubmission),
        },
        _ => unreachable!("Invalid response"),
    }
}

async fn fetch_problems() -> Result<ProblemSet, Error> {
    let res = fetch(URL).await?;

    let body = res.json::<Value>().await?;
    let status = body
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if status != "OK" {
        return Err(anyhow::anyhow!("Failed to fetch problems"));
    };

    let problems = if let Value::Object(mut map) = body
        && let Some(Value::Object(mut result)) = map.remove("result")
        && let Some(Value::Array(problems)) = result.remove("problems")
    {
        problems
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .map(Arc::new)
            .collect::<Vec<_>>()
    } else {
        return Err(anyhow::anyhow!("Failed to fetch problems"));
    };

    Ok(problems)
}

pub async fn get_problems() -> Result<Arc<ProblemSet>, Error> {
    if let Some(ref problems) = *PROBLEMS.read().await {
        Ok(Arc::clone(problems))
    } else {
        let problems = fetch_problems().await?;
        let problems = Arc::new(problems);
        PROBLEMS.write().await.replace(Arc::clone(&problems));
        Ok(problems)
    }
}

#[allow(dead_code)]
pub async fn random_problem() -> Result<Arc<Problem>, Error> {
    let problems = get_problems().await?;
    let problem = problems.choose(&mut rand::rng()).unwrap();
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
                .filter(|problem| {
                    problem.rating.unwrap_or(4000) <= MAX_DAILY_RATING
                        && !problem.tags.iter().any(|tag| tag == "*special")
                })
                .choose(&mut rand::rng())
                .cloned()
            {
                Some(problem) => problem,
                None => return Err(anyhow::anyhow!("没有找到题目，请稍后再试")),
            };

            Commit::start()
                .await?
                .set_daily_problem(&problem)
                .await?
                .commit()
                .await?;

            Ok(problem)
        }
    }
}
