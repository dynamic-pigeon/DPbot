use std::sync::{Arc, LazyLock};

use anyhow::{Ok, Result};
use kovi::chrono::{self, DateTime};
use kovi::log::debug;
use kovi::tokio::sync::RwLock;
use rand::seq::SliceRandom;

use crate::duel::problem::get_problems_by;
use crate::sql;

use super::problem::{Problem, get_last_submission};

static CHALLENGES: LazyLock<RwLock<Vec<Challenge>>> = LazyLock::new(|| RwLock::new(Vec::new()));

pub async fn add_challenge(challenge: Challenge) {
    let mut challenges = CHALLENGES.write().await;
    challenges.push(challenge);
}

pub async fn get_challenge(user_id: i64) -> Option<Challenge> {
    let challenges = CHALLENGES.read().await;
    challenges
        .iter()
        .find(|challenge| challenge.user1 == user_id || challenge.user2 == user_id)
        .cloned()
}

pub async fn get_challenge_by_user2(user_id: i64) -> Option<Challenge> {
    let challenges = CHALLENGES.read().await;
    challenges
        .iter()
        .find(|challenge| challenge.user2 == user_id)
        .cloned()
}

pub async fn get_challenge_by_user1(user_id: i64) -> Option<Challenge> {
    let challenges = CHALLENGES.read().await;
    challenges
        .iter()
        .find(|challenge| challenge.user1 == user_id)
        .cloned()
}

pub async fn remove_challenge(user1: i64, user2: i64) -> Option<Challenge> {
    let mut challenges = CHALLENGES.write().await;
    let index = challenges.iter().position(|challenge| {
        (challenge.user1 == user1 && challenge.user2 == user2)
            || (challenge.user1 == user2 && challenge.user2 == user1)
    })?;
    Some(challenges.remove(index))
}

pub async fn user_inside(user_id: i64) -> bool {
    let challenges = CHALLENGES.read().await;
    challenges
        .iter()
        .any(|challenge| challenge.user1 == user_id || challenge.user2 == user_id)
        || sql::duel::challenge::get_chall_ongoing_by_user(user_id)
            .await
            .is_ok()
}

#[derive(Clone)]
pub struct Challenge {
    pub user1: i64,
    pub user2: i64,
    pub time: DateTime<chrono::Utc>,
    pub rating: i64,
    pub tags: Vec<String>,
    pub problem: Option<Problem>,
    pub result: Option<i64>,
    pub started: i64,
    pub changed: Option<i64>,
}

impl Challenge {
    pub fn new(
        user1: i64,
        user2: i64,
        time: DateTime<chrono::Utc>,
        tags: Vec<String>,
        rating: i64,
        problem: Option<Problem>,
        result: Option<i64>,
        started: i64,
    ) -> Self {
        Self {
            user1,
            user2,
            time,
            rating,
            tags,
            problem,
            result,
            started,
            changed: None,
        }
    }

    pub async fn start(&mut self) -> Result<Arc<Problem>> {
        let problems = get_problems_by(&self.tags, self.rating, self.user1).await?;
        let problem = problems
            .choose(&mut rand::thread_rng())
            .ok_or_else(|| anyhow::anyhow!("没有找到题目"))?;
        self.problem = Some(problem.as_ref().clone());
        self.started = 1;
        sql::duel::challenge::add_challenge(self).await?;
        Ok(Arc::clone(problem))
    }

    pub async fn give_up(&mut self, user_id: i64) -> Result<()> {
        if user_id == self.user1 {
            self.result = Some(1);
        } else if user_id == self.user2 {
            self.result = Some(0);
        } else {
            return Err(anyhow::anyhow!("你不是这场对局的参与者"));
        }

        let mut user1 = sql::duel::user::get_user(self.user1).await?;
        let mut user2 = sql::duel::user::get_user(self.user2).await?;

        let user1_rating = user1.rating;
        let user2_rating = user2.rating;

        let (new_user1_rating, new_user2_rating) =
            calculate_elo_rating(user1_rating, user2_rating, self.result.unwrap() == 0);

        user1.rating = new_user1_rating;
        user2.rating = new_user2_rating;

        sql::duel::user::update_two_user(&user1, &user2).await?;
        sql::duel::challenge::change_result(self).await?;

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

        let score = |submission: kovi::serde_json::Value| {
            debug!("Submission: {:#?}", submission);
            let problem = submission
                .get("problem")
                .and_then(crate::duel::problem::Problem::from_value)
                .unwrap_or_else(|| Problem::new(0, "".to_string(), 0, vec![]));

            if problem.contest_id != self.problem.as_ref().unwrap().contest_id
                || problem.index != self.problem.as_ref().unwrap().index
            {
                return Ok((0, 0));
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

            Ok((if pass { 1 } else { 0 }, -time))
        };

        let user1_score = score(user1_sub)?;
        let user2_score = score(user2_sub)?;

        if user1_score.0 + user2_score.0 == 0 {
            return Err(anyhow::anyhow!("你还没有通过题目哦"));
        }

        let result = user1_score > user2_score;
        self.result = Some(if result { 0 } else { 1 });
        sql::duel::challenge::change_result(self).await?;

        let user1_rating = user1.rating;
        let user2_rating = user2.rating;

        let (new_user1_rating, new_user2_rating) =
            calculate_elo_rating(user1_rating, user2_rating, result);

        let mut user1 = user1;
        let mut user2 = user2;

        user1.rating = new_user1_rating;
        user2.rating = new_user2_rating;

        sql::duel::user::update_two_user(&user1, &user2).await?;

        Ok(())
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
