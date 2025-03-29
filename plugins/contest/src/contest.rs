use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use anyhow::Result;
use kovi::{
    chrono::{self, Datelike, FixedOffset},
    log::info,
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
        .filter(|c| {
            let start = chrono::NaiveDateTime::parse_from_str(&c.start, "%Y-%m-%dT%H:%M:%S")
                .unwrap()
                .and_utc();
            start > now
        })
        .cloned()
        .collect()
}

pub async fn init() -> Result<usize> {
    (async {
        for _ in 0..3 {
            if (update_contests().await).is_ok() {
                return Ok(());
            }
        }
        send_to_super_admin("Failed to update contests").await;
        Err(anyhow::anyhow!("Failed to update contests"))
    })
    .await?;

    info!("Contest 加载完成");

    let contests = CONTESTS.read().await.clone();
    let now = today_utc();
    let mut map: HashMap<_, Vec<Arc<Contest>>> = HashMap::new();
    let offset = FixedOffset::east_opt(8 * 3600).unwrap();

    for contest in contests.iter().cloned() {
        let start = chrono::NaiveDateTime::parse_from_str(&contest.start, "%Y-%m-%dT%H:%M:%S")?
            .and_utc()
            .with_timezone(&offset)
            .to_utc();
        if start < now || start.num_days_from_ce() != now.num_days_from_ce() {
            continue;
        }
        map.entry(start).or_default().push(contest);
    }

    let config = {
        let config = crate::CONFIG.get().unwrap();
        Arc::clone(config)
    };

    let mut count = 0;

    for (time, contests) in map {
        count += contests.len();
        for sub_time in config.notify_time.iter() {
            let mut msg = format!("选手注意，以下比赛还有不到 {} 分钟就要开始了：\n", sub_time);
            for contest in contests.iter().cloned() {
                let add = format!("\n{}\n{}\n", contest.event, contest.href);

                msg.push_str(&add);
            }
            let start = time;
            let now = today_utc();

            let notify_time = start - chrono::Duration::minutes(*sub_time);

            info!(
                "{} contests are going to start at {}.",
                contests.len(),
                notify_time
            );

            let duration = notify_time - now;
            if duration.num_minutes() < 0 {
                continue;
            }

            let config = config.clone();
            kovi::spawn(async move {
                kovi::tokio::time::sleep(duration.to_std().unwrap()).await;
                let bot = crate::BOT.get().unwrap().clone();
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
        let bot = crate::BOT.get().unwrap().clone();

        let msg = if count == 0 {
            "今天没有比赛，但也不要松懈哦。".to_string()
        } else {
            format!("今天装填了 {} 场比赛，准备发射！", count)
        };

        let config = crate::CONFIG.get().unwrap().clone();
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
    let hour = total_minutes / 60 % 24;
    let minute = total_minutes % 60;
    let day = total_minutes / 60 / 24;
    if day > 0 {
        return format!("{}d {:02}h {:02}min", day, hour, minute);
    }
    format!("{:02}h {:02}min", hour, minute)
}

async fn send_to_super_admin(msg: &str) {
    let bot = crate::BOT.get().unwrap().clone();
    bot.send_private_msg(bot.get_main_admin().unwrap(), msg);
}
