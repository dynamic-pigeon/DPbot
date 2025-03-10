use std::{collections::HashMap, sync::LazyLock};

use crate::{duel::problem::get_recent_submission, sql};
use anyhow::Result;
use kovi::{chrono, log::info, tokio::sync::RwLock};

static USER: LazyLock<RwLock<HashMap<i64, User>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

pub async fn add_to(user: User) {
    let mut user_map = USER.write().await;
    user_map.insert(user.qq, user);
}

pub async fn user_inside(qq: i64) -> bool {
    let map = USER.read().await;
    map.contains_key(&qq)
}

pub async fn get_user(qq: i64) -> Option<User> {
    let mut map = USER.write().await;
    map.remove(&qq)
}

/// 用户信息
pub struct User {
    pub qq: i64,
    pub rating: i64,
    pub cf_id: Option<String>,
    bind: Option<Bind>,
    pub daily_score: i64,
}

struct Bind {
    /// 绑定的 cf 账号
    cf_id: String,
    /// 绑定开始时间
    start_time: chrono::DateTime<chrono::Utc>,
}

impl User {
    pub fn new(qq: i64, rating: i64, cf_id: Option<String>, daily_score: i64) -> Self {
        Self {
            qq,
            rating,
            cf_id,
            bind: None,
            daily_score,
        }
    }

    pub fn bind(&mut self, cf_id: String) {
        self.bind = Some(Bind::new(cf_id));
    }

    pub async fn finish_bind(&mut self) -> Result<()> {
        let bind = std::mem::replace(&mut self.bind, None);
        if bind.is_none() {
            return Err(anyhow::anyhow!("你似乎没有在绑定哦"));
        }

        let bind = bind.unwrap();

        if !bind.finish().await {
            return Err(anyhow::anyhow!("绑定失败"));
        }

        self.cf_id = Some(bind.cf_id);

        sql::update_user(self).await?;

        Ok(())
    }
}

impl Bind {
    fn new(cf_id: String) -> Self {
        let start_time = chrono::Utc::now();
        Self { cf_id, start_time }
    }

    async fn finish(&self) -> bool {
        let Some(submission) = get_recent_submission(&self.cf_id).await else {
            return false;
        };

        info!("Submission: {:?}", submission);

        let Some(contest_id) = submission.get("contestId").and_then(|v| v.as_i64()) else {
            return false;
        };

        let Some(index) = submission.get("index").and_then(|v| v.as_str()) else {
            return false;
        };

        let Some(verdict) = submission.get("verdict").and_then(|v| v.as_str()) else {
            return false;
        };

        if contest_id != 1 || index != "A" || verdict != "COMPILATION_ERROR" {
            return false;
        }

        let end_time = chrono::DateTime::from_timestamp(
            submission["creationTimeSeconds"].as_i64().unwrap(),
            0,
        )
        .unwrap();

        if end_time < self.start_time {
            return false;
        }

        let duration = end_time - self.start_time;
        if duration.num_seconds() > 120 {
            return false;
        }

        true
    }
}
