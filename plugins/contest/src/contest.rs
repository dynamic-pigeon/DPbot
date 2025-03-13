use std::{
    cmp::Reverse,
    collections::HashMap,
    os::unix::raw::time_t,
    sync::{Arc, LazyLock},
    time::Duration,
};

use anyhow::Result;
use kovi::{
    chrono::{self, DateTime, Datelike, FixedOffset, Timelike, Utc, format},
    log::{error, info},
    tokio::sync::RwLock,
};
use serde::Deserialize;

use crate::today_utc;

type ContestSet = Vec<Arc<Contest>>;

pub(crate) static CONTESTS: LazyLock<RwLock<Arc<ContestSet>>> =
    LazyLock::new(|| RwLock::new(Arc::new(Vec::new())));

#[derive(Deserialize)]
pub struct Contest {
    pub duration: u64,
    pub end: String,
    pub event: String,
    pub host: String,
    pub href: String,
    pub resource: String,
    pub start: String,
}

impl Contest {
    pub fn duration(&self) -> String {
        seconds_to_str(self.duration as i64)
    }

    pub fn start_time(&self) -> String {
        let start = chrono::NaiveDateTime::parse_from_str(&self.start, "%Y-%m-%dT%H:%M:%S")
            .unwrap()
            .and_utc();
        let offset = FixedOffset::east_opt(8 * 3600).unwrap();
        let start = start.with_timezone(&offset);
        format!("{}", start.format("%Y-%m-%d(%A) %H:%M"))
    }
}

pub async fn get_all_contests() -> ContestSet {
    if CONTESTS.read().await.is_empty() {
        init().await.unwrap();
    }
    let contests = CONTESTS.read().await.clone();
    let now = kovi::chrono::Utc::now();
    contests
        .iter()
        .cloned()
        .filter(|c| {
            let start = chrono::NaiveDateTime::parse_from_str(&c.start, "%Y-%m-%dT%H:%M:%S")
                .unwrap()
                .and_utc();
            start > now
        })
        .collect()
}

pub async fn init() -> Result<usize> {
    match update_contests().await {
        Ok(_) => {
            info!("Contest 加载完成");
        }
        Err(e) => {
            error!("Contest 加载失败: {:?}", e);
            return Err(e);
        }
    };
    let contests = CONTESTS.read().await.clone();
    let now = today_utc();
    let mut map: HashMap<_, Vec<Arc<Contest>>> = HashMap::new();

    for contest in contests.iter().cloned() {
        let start =
            chrono::NaiveDateTime::parse_from_str(&contest.start, "%Y-%m-%dT%H:%M:%S")?.and_utc();
        if start < now || start.num_days_from_ce() != now.num_days_from_ce() {
            continue;
        }
        map.entry(start).or_default().push(contest);
    }

    let config = {
        let config = crate::CONFIG.read().await;
        Arc::clone(&*config)
    };

    let mut count = 0;

    for (time, contests) in map {
        count += contests.len();
        for sub_time in config.notify_time.iter() {
            info!(
                "{} contests are going to start in {} minutes.",
                contests.len(),
                sub_time
            );

            let mut msg = format!("选手注意，以下比赛还有不到 {} 分钟就要开始了：\n", sub_time);
            for contest in contests.iter().cloned() {
                let add = format!("\n{}\n{}\n", contest.event, contest.href);

                msg.push_str(&add);
            }
            let offset = FixedOffset::east_opt(8 * 3600).unwrap();
            let start = time.with_timezone(&offset).to_utc();
            let now = today_utc();

            let duration = start - now;
            if duration.num_minutes() < 0 {
                continue;
            }

            let config = config.clone();
            kovi::spawn(async move {
                kovi::tokio::time::sleep(duration.to_std().unwrap()).await;
                let bot = crate::BOT.read().await.as_ref().unwrap().clone();
                for group in config.notify_group.iter() {
                    bot.send_group_msg(*group, &msg);
                }
            });
        }
    }

    Ok(count)
}

pub async fn daily_init() {
    let count = init().await;
    if let Ok(count) = count {
        info!("{} contests are going to start today.", count);
        let bot = crate::BOT.read().await.as_ref().unwrap().clone();

        let msg = if count == 0 {
            "今天没有比赛，但也不要松懈哦。".to_string()
        } else {
            format!("今天装填了 {} 场比赛，准备发射！", count)
        };

        let config = crate::CONFIG.read().await.clone();
        for group in config.notify_group.iter() {
            bot.send_group_msg(*group, &msg);
        }
    }
}

pub(crate) async fn update_contests() -> Result<()> {
    let mut contests = super::getter::fetch_contest().await?;
    contests.sort_by_key(|contest| contest.start.clone());
    *CONTESTS.write().await = Arc::new(contests);

    Ok(())
}

fn seconds_to_str(seconds: i64) -> String {
    let total_minutes = seconds / 60;
    let hour = total_minutes / 60;
    let minute = total_minutes % 60;
    format!("{:02}:{:02}", hour, minute)
}
