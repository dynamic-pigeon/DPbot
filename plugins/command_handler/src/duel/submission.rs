use kovi::serde_json::{self, Value};

use crate::{duel::problem::Problem, utils::fetch};

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct Submission {
    #[serde(rename = "creationTimeSeconds")]
    pub creation_time_seconds: i64,
    pub problem: Problem,
    pub verdict: Option<String>,
    pub author: Author,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct Author {
    #[serde(rename = "participantType")]
    pub participant_type: String,
}

#[derive(thiserror::Error, Debug)]
pub enum SubmissionError {
    #[error("Failed to fetch response")]
    FetchError,
    #[error("No submission found")]
    NoSubmission,
}

impl Submission {
    pub fn is_accepted(&self) -> bool {
        matches!(self.verdict.as_deref(), Some("OK"))
    }

    #[allow(dead_code)]
    pub fn is_practice(&self) -> bool {
        self.author.participant_type == "PRACTICE"
    }
}

pub async fn get_recent_submissions(cf_id: &str) -> Result<Vec<Submission>, SubmissionError> {
    let res = fetch(&format!(
        "https://codeforces.com/api/user.status?handle={}",
        cf_id
    ))
    .await
    .map_err(|_| SubmissionError::FetchError)?;

    let body = res
        .json::<Value>()
        .await
        .map_err(|_| SubmissionError::FetchError)?;

    let status = body["status"].as_str().ok_or(SubmissionError::FetchError)?;
    if status != "OK" {
        return Err(SubmissionError::FetchError);
    }

    match body {
        Value::Object(mut map) => match map.remove("result") {
            Some(Value::Array(submissions)) => Ok(submissions
                .into_iter()
                .map(serde_json::from_value)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| SubmissionError::FetchError)?),
            _ => Err(SubmissionError::NoSubmission),
        },
        _ => unreachable!("Invalid response"),
    }
}

/// 得到用户最近一次提交的信息
pub async fn get_last_submission(cf_id: &str) -> Result<Submission, SubmissionError> {
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

    let status = body["status"].as_str().ok_or(SubmissionError::FetchError)?;
    if status != "OK" {
        return Err(SubmissionError::FetchError);
    }

    match body {
        Value::Object(mut map) => match map.remove("result") {
            Some(Value::Array(mut submissions)) => {
                // 获取最近一次提交
                assert!(submissions.len() <= 1);
                submissions
                    .pop()
                    .ok_or(SubmissionError::NoSubmission)
                    .and_then(|v| {
                        serde_json::from_value(v).map_err(|_| SubmissionError::FetchError)
                    })
            }
            _ => Err(SubmissionError::NoSubmission),
        },
        _ => unreachable!("Invalid response"),
    }
}
