use std::sync::Arc;

use anyhow::Result;
use kovi::chrono::{self, DateTime};
use kovi::log::debug;
use kovi::serde_json;
use rand::seq::SliceRandom;
use sqlx::{Decode, Encode, Sqlite, Type};

use crate::duel::problem::get_problems_by;
use crate::sql;

use super::problem::{Problem, get_last_submission};

#[derive(Clone)]
pub struct Challenge {
    pub user1: i64,
    pub user2: i64,
    pub time: DateTime<chrono::Utc>,
    pub rating: i64,
    pub tags: Vec<String>,
    pub problem: Option<Problem>,
    pub status: ChallengeStatus,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum ChallengeStatus {
    Ongoing,
    Finished(i64),
    Panding,
    ChangeProblem(i64),
}

impl Type<Sqlite> for ChallengeStatus {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <i64 as Type<Sqlite>>::type_info()
    }
}

impl<'q> Encode<'q, Sqlite> for ChallengeStatus {
    fn encode_by_ref(
        &self,
        buf: &mut <Sqlite as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        match self {
            ChallengeStatus::Panding => <i64 as sqlx::Encode<Sqlite>>::encode_by_ref(&1i64, buf),
            ChallengeStatus::Ongoing => <i64 as sqlx::Encode<Sqlite>>::encode_by_ref(&2i64, buf),
            ChallengeStatus::Finished(res) => {
                <i64 as sqlx::Encode<Sqlite>>::encode_by_ref(&-res, buf)
            }
            ChallengeStatus::ChangeProblem(id) => {
                <i64 as sqlx::Encode<Sqlite>>::encode_by_ref(&id, buf)
            }
        }
    }
}

impl<'r> Decode<'r, Sqlite> for ChallengeStatus {
    fn decode(
        value: <Sqlite as sqlx::Database>::ValueRef<'r>,
    ) -> std::result::Result<Self, sqlx::error::BoxDynError> {
        let num: i64 = <i64 as sqlx::Decode<Sqlite>>::decode(value)?;
        match num {
            1 => Ok(ChallengeStatus::Panding),
            2 => Ok(ChallengeStatus::Ongoing),
            num if num > 0 => Ok(ChallengeStatus::ChangeProblem(num)),
            num => Ok(ChallengeStatus::Finished(-num)),
        }
    }
}

impl Challenge {
    pub fn new(
        user1: i64,
        user2: i64,
        time: DateTime<chrono::Utc>,
        tags: Vec<String>,
        rating: i64,
        problem: Option<Problem>,
        status: ChallengeStatus,
    ) -> Self {
        Self {
            user1,
            user2,
            time,
            rating,
            tags,
            problem,
            status,
        }
    }

    pub fn started(&self) -> bool {
        !matches!(self.status, ChallengeStatus::Panding)
    }

    pub async fn start(&mut self) -> Result<Arc<Problem>> {
        let problems = get_problems_by(&self.tags, self.rating, self.user1).await?;
        let problem = problems
            .choose(&mut rand::thread_rng())
            .ok_or_else(|| anyhow::anyhow!("没有找到题目"))?;
        self.problem = Some(problem.as_ref().clone());
        self.status = ChallengeStatus::Ongoing;
        sql::duel::challenge::change_status(self).await?;
        sql::duel::challenge::change_problem(self).await?;
        Ok(Arc::clone(problem))
    }

    pub async fn give_up(&mut self, user_id: i64) -> Result<()> {
        let status = if user_id == self.user1 {
            ChallengeStatus::Finished(1)
        } else if user_id == self.user2 {
            ChallengeStatus::Finished(0)
        } else {
            return Err(anyhow::anyhow!("你不是这场对局的参与者"));
        };

        let mut user1 = sql::duel::user::get_user(self.user1).await?;
        let mut user2 = sql::duel::user::get_user(self.user2).await?;

        let user1_rating = user1.rating;
        let user2_rating = user2.rating;

        let (new_user1_rating, new_user2_rating) = calculate_elo_rating(
            user1_rating,
            user2_rating,
            self.status == ChallengeStatus::Finished(0),
        );

        user1.rating = new_user1_rating;
        user2.rating = new_user2_rating;

        sql::duel::user::update_two_user_rating(&user1, &user2).await?;
        self.change_status(status).await?;

        Ok(())
    }

    pub async fn judge(&mut self) -> Result<()> {
        let user1 = sql::duel::user::get_user(self.user1).await?;
        let user2 = sql::duel::user::get_user(self.user2).await?;

        let user1_sub = get_last_submission(user1.cf_id.as_ref().unwrap())
            .await
            .ok_or(anyhow::anyhow!("获取提交记录失败"))?;

        let user2_sub = get_last_submission(user2.cf_id.as_ref().unwrap())
            .await
            .ok_or(anyhow::anyhow!("获取提交记录失败"))?;

        let user1_score = self.calc_score(user1_sub)?;
        let user2_score = self.calc_score(user2_sub)?;

        if !user1_score.0 && !user2_score.0 {
            return Err(anyhow::anyhow!("你还没有通过题目哦"));
        }

        let result = user1_score > user2_score;
        let status = ChallengeStatus::Finished(if result { 0 } else { 1 });

        let user1_rating = user1.rating;
        let user2_rating = user2.rating;

        let (new_user1_rating, new_user2_rating) =
            calculate_elo_rating(user1_rating, user2_rating, result);

        let mut user1 = user1;
        let mut user2 = user2;

        user1.rating = new_user1_rating;
        user2.rating = new_user2_rating;

        sql::duel::user::update_two_user_rating(&user1, &user2).await?;
        self.change_status(status).await?;

        Ok(())
    }

    /// @param submission 提交记录
    /// @return (是否通过, -提交时间)
    fn calc_score(&self, submission: kovi::serde_json::Value) -> Result<(bool, i64)> {
        debug!("Submission: {:#?}", submission);
        let mut submission = match submission {
            serde_json::Value::Object(map) => map,
            _ => Err(anyhow::anyhow!("获取提交记录失败"))?,
        };
        let problem: Problem = submission
            .remove("problem")
            .and_then(|p| serde_json::from_value(p).ok())
            .ok_or(anyhow::anyhow!("获取题目信息失败"))?;

        if problem.contest_id != self.problem.as_ref().unwrap().contest_id
            || problem.index != self.problem.as_ref().unwrap().index
        {
            return Ok((false, 0));
        }

        let pass = submission
            .get("verdict")
            .and_then(|v| v.as_str())
            .ok_or(anyhow::anyhow!("获取提交结果失败"))?
            == "OK";
        let time = submission
            .get("creationTimeSeconds")
            .and_then(|v| v.as_i64())
            .ok_or(anyhow::anyhow!("获取提交时间失败"))?;

        Ok((pass, -time))
    }

    pub async fn change_status(&mut self, status: ChallengeStatus) -> Result<()> {
        self.status = status;
        sql::duel::challenge::change_status(self).await
    }
    pub async fn change(&mut self) -> Result<Arc<Problem>> {
        let problems = get_problems_by(&self.tags, self.rating, self.user1).await?;
        let problem = problems
            .choose(&mut rand::thread_rng())
            .ok_or_else(|| anyhow::anyhow!("没有找到题目"))?;
        self.problem = Some(problem.as_ref().clone());
        sql::duel::challenge::change_problem(self).await?;
        Ok(Arc::clone(problem))
    }
}

// 计算 ELO Rating 的函数
fn calculate_elo_rating(rating1: i64, rating2: i64, result: bool) -> (i64, i64) {
    let k = super::config::ELO_K;
    let r1 = rating1 as f64;
    let r2 = rating2 as f64;

    let e1 = 1.0 / (1.0 + 10.0f64.powf((r2 - r1) / 400.0));
    let e2 = 1.0 / (1.0 + 10.0f64.powf((r1 - r2) / 400.0));

    let s1 = if result { 1.0 } else { 0.0 };
    let s2 = if result { 0.0 } else { 1.0 };

    let new_rating1 = r1 + k * (s1 - e1);
    let new_rating2 = r2 + k * (s2 - e2);

    // 调整 K 值以确保总分不变
    let total_rating_before = r1 + r2;
    let total_rating_after = new_rating1 + new_rating2;
    let adjustment = (total_rating_before - total_rating_after) / 2.0;

    let adjusted_new_rating1 = new_rating1 + adjustment;
    let adjusted_new_rating2 = new_rating2 + adjustment;

    (
        adjusted_new_rating1.round() as i64,
        adjusted_new_rating2.round() as i64,
    )
}

pub async fn user_in_ongoing_challenge(user_id: i64) -> bool {
    get_ongoing_challenge_by_user(user_id).await.is_ok()
}

pub async fn get_ongoing_challenge_by_user(user_id: i64) -> Result<Challenge> {
    sql::duel::challenge::get_chall_ongoing_by_user(user_id).await
}

pub async fn add_challenge(challenge: &Challenge) -> Result<()> {
    sql::duel::challenge::add_challenge(challenge).await
}

pub async fn get_challenge_by_user2(user_id: i64) -> Result<Challenge> {
    sql::duel::challenge::get_chall_ongoing_by_user(user_id)
        .await
        .and_then(|c| {
            if c.user2 == user_id {
                Ok(c)
            } else {
                Err(anyhow::anyhow!("没有找到对局"))
            }
        })
}

pub async fn get_challenge_by_user1(user_id: i64) -> Result<Challenge> {
    sql::duel::challenge::get_chall_ongoing_by_user(user_id)
        .await
        .and_then(|c| {
            if c.user2 == user_id {
                Ok(c)
            } else {
                Err(anyhow::anyhow!("没有找到对局"))
            }
        })
}

pub async fn get_challenge(user1: i64, user2: i64) -> Result<Challenge> {
    sql::duel::challenge::get_chall_ongoing_by_2user(user1, user2).await
}

pub async fn remove_challenge(challenge: &Challenge) -> Result<()> {
    sql::duel::challenge::remove_challenge(challenge).await
}
