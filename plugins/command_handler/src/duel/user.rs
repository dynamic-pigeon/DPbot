use std::{collections::HashMap, sync::Arc};

use crate::{
    duel::submission::get_last_submission,
    sql::{self, duel::user::CommitUserExt},
};
use anyhow::Result;
use kovi::{chrono, log::info, tokio::sync::RwLock};
use sqlx::{FromRow, Row, sqlite::SqliteRow};

#[derive(Clone, Default)]
pub struct BindingUsers(Arc<RwLock<HashMap<i64, User>>>);

impl BindingUsers {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert(&self, user: User) {
        let mut user_map = self.0.write().await;
        user_map.insert(user.qq, user);
    }

    pub async fn contains(&self, qq: i64) -> bool {
        let map = self.0.read().await;
        map.contains_key(&qq)
    }

    pub async fn take(&self, qq: i64) -> Option<User> {
        let mut map = self.0.write().await;
        map.remove(&qq)
    }
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
            .update_user_cf_id(self)
            .await?
            .commit()
            .await?;

        Ok(())
    }
}

impl Bind {
    fn new(cf_id: String) -> Self {
        Self {
            cf_id,
            start_time: chrono::Utc::now(),
        }
    }

    async fn finish(&self) -> Result<()> {
        let submission = get_last_submission(&self.cf_id)
            .await
            .map_err(|e| anyhow::anyhow!("获取提交记录失败: {}", e))?;

        info!("Submission: {:#?}", submission);

        let problem = submission.problem;

        let contest_id = problem.contest_id;

        let index = problem.index;

        let verdict = submission
            .verdict
            .ok_or(anyhow::anyhow!("该提交没有评测结果"))?;

        if contest_id != 1 || index != "A" {
            return Err(anyhow::anyhow!("请提交到指定题目 (Contest 1, Problem A)"));
        }

        if verdict != "COMPILATION_ERROR" {
            return Err(anyhow::anyhow!("需要提交一个导致编译错误(CE)的代码"));
        }

        let creation_time_seconds = submission.creation_time_seconds;

        let end_time = chrono::DateTime::from_timestamp(creation_time_seconds, 0)
            .ok_or_else(|| anyhow::anyhow!("无效的时间戳"))?;

        if end_time < self.start_time || (end_time - self.start_time).num_seconds() > 120 {
            return Err(anyhow::anyhow!("未在规定时间(2分钟)内完成绑定操作"));
        }

        Ok(())
    }
}
