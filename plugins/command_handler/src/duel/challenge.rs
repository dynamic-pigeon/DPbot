use std::sync::LazyLock;

use kovi::chrono::{self, DateTime};
use kovi::serde_json::Value;
use kovi::tokio::sync::RwLock;

static CHALLENGES: LazyLock<RwLock<Vec<Challenge>>> = LazyLock::new(|| RwLock::new(Vec::new()));

pub async fn add_challenge(challenge: Challenge) {
    let mut challenges = CHALLENGES.write().await;
    challenges.push(challenge);
}

pub struct Challenge {
    pub user1: i64,
    pub user2: i64,
    pub time: DateTime<chrono::Utc>,
    pub problem: Value,
    pub result: Option<i64>,
    pub started: i64,
}

impl Challenge {
    pub fn new(
        user1: i64,
        user2: i64,
        time: DateTime<chrono::Utc>,
        problem: Value,
        result: Option<i64>,
        started: i64,
    ) -> Self {
        Self {
            user1,
            user2,
            time,
            problem,
            result,
            started,
        }
    }
}
