use std::{collections::HashMap, sync::LazyLock};

use crate::{
    duel::problem::get_last_submission,
    sql::{self, duel::user::CommitUserExt},
    utils::today_utc,
};
use anyhow::Result;
use kovi::{chrono, log::info, tokio::sync::RwLock};
use sqlx::{FromRow, Row, sqlite::SqliteRow};

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
#[derive(Clone)]
pub struct User {
    pub qq: i64,
    pub rating: i64,
    pub cf_id: Option<String>,
    bind: Option<Bind>,
    pub daily_score: i64,
    pub last_daily: String,
}

#[derive(Clone)]
struct Bind {
    /// 绑定的 cf 账号
    cf_id: String,
    /// 绑定开始时间
    start_time: chrono::DateTime<chrono::Utc>,
}

impl<'r> FromRow<'r, SqliteRow> for User {
    fn from_row(row: &'r SqliteRow) -> std::result::Result<Self, sqlx::Error> {
        let qq: i64 = row.try_get("qq")?;
        let rating: i64 = row.try_get("rating")?;
        let cf_id: Option<String> = row.try_get("cf_id")?;
        let daily_score: i64 = row.try_get("daily_score")?;
        let last_daily: String = row.try_get("last_daily")?;

        Ok(Self {
            qq,
            rating,
            cf_id,
            bind: None,
            daily_score,
            last_daily,
        })
    }
}

impl User {
    #[allow(dead_code)]
    pub fn new(
        qq: i64,
        rating: i64,
        cf_id: Option<String>,
        daily_score: i64,
        last_daily: String,
    ) -> Self {
        Self {
            qq,
            rating,
            cf_id,
            bind: None,
            daily_score,
            last_daily,
        }
    }

    #[inline]
    pub fn start_bind(&mut self, cf_id: String) {
        self.bind = Some(Bind::new(cf_id));
    }

    pub async fn finish_bind(&mut self) -> Result<()> {
        let bind = self.bind.take();
        if bind.is_none() {
            return Err(anyhow::anyhow!("你似乎没有在绑定哦"));
        }

        let bind = bind.unwrap();

        bind.finish().await?;

        self.cf_id = Some(bind.cf_id);

        sql::utils::Commit::start()
            .await?
            .update_user(self)
            .await?
            .commit()
            .await?;

        Ok(())
    }
}

impl Bind {
    fn new(cf_id: String) -> Self {
        let start_time = today_utc();
        Self { cf_id, start_time }
    }

    async fn finish(&self) -> Result<()> {
        let Ok(submission) = get_last_submission(&self.cf_id).await else {
            return Err(anyhow::anyhow!("获取提交记录失败"));
        };

        info!("Submission: {:#?}", submission);

        let Some(problem) = submission.get("problem").and_then(|v| v.as_object()) else {
            return Err(anyhow::anyhow!("未知错误"));
        };

        let Some(contest_id) = problem.get("contestId").and_then(|v| v.as_i64()) else {
            return Err(anyhow::anyhow!("未知错误"));
        };

        let Some(index) = problem.get("index").and_then(|v| v.as_str()) else {
            return Err(anyhow::anyhow!("未知错误"));
        };

        let Some(verdict) = submission.get("verdict").and_then(|v| v.as_str()) else {
            return Err(anyhow::anyhow!("未知错误"));
        };

        if contest_id != 1 || index != "A" {
            return Err(anyhow::anyhow!("没有交到指定题目"));
        }

        if verdict != "COMPILATION_ERROR" {
            return Err(anyhow::anyhow!("没有交 CE"));
        }

        let end_time = chrono::DateTime::from_timestamp(
            submission["creationTimeSeconds"].as_i64().unwrap(),
            0,
        )
        .unwrap();

        let duration = end_time - self.start_time;
        if end_time < self.start_time || duration.num_seconds() > 120 {
            return Err(anyhow::anyhow!("未在规定时间内提交"));
        }

        Ok(())
    }
}
