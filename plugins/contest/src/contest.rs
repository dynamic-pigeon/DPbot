use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use anyhow::Result;
use kovi::{
    chrono::{self, Datelike, FixedOffset},
    log::{debug, error, info},
    tokio::sync::RwLock,
};
use serde::Deserialize;
use utils::retry::retry;

use crate::today_utc;

type ContestSet = Vec<Arc<Contest>>;

static CONTESTS: LazyLock<RwLock<Arc<ContestSet>>> =
    LazyLock::new(|| RwLock::new(Arc::new(Vec::new())));

#[allow(dead_code)]
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
    let contests = {
        let contests = CONTESTS.read().await;
        Arc::clone(&contests)
    };
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
    match retry(update_contests, 3).await {
        Ok(_) => {
            info!("Contest 加载完成");
        }
        Err(e) => {
            send_to_super_admin(&format!("Contest 初始化失败: {}", e));
            error!("Contest 初始化失败: {}，继续使用之前的数据初始化", e);
            kovi::spawn(async {
                debug!("Retrying to update contests after 1 hour...");
                kovi::tokio::time::sleep(kovi::tokio::time::Duration::from_secs(60 * 60)).await;
                if let Err(e) = update_contests().await {
                    send_to_super_admin(&format!("Contest 重试更新失败: {}", e));
                    error!("Contest 重试更新失败: {}", e);
                } else {
                    info!("Contest 重试更新成功");
                }
            });
        }
    }

    let contests = {
        let contests = CONTESTS.read().await;
        Arc::clone(&contests)
    };
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

    let config = crate::CONFIG.get().unwrap();

    let mut count = 0;

    for (time, contests) in map {
        count += contests.len();

        let mut contest_msg = String::new();
        for contest in contests.iter() {
            let add = format!("\n\n{}\n{}", contest.event, contest.href);

            contest_msg.push_str(&add);
        }

        for sub_time in config.notify_time.iter() {
            let start = time;
            let now = today_utc();

            let notify_time = start - chrono::Duration::minutes(*sub_time);

            let duration = notify_time - now;
            if duration.num_minutes() < 0 {
                continue;
            }

            let msg = format!(
                "选手注意，以下比赛还有不到 {} 分钟就要开始了：{}",
                sub_time, contest_msg
            );

            info!(
                "{} contests are going to notify at {}.",
                contests.len(),
                notify_time
            );

            kovi::spawn(async move {
                kovi::tokio::time::sleep(duration.to_std().unwrap()).await;
                let config = crate::CONFIG.get().unwrap();
                let bot = crate::BOT.get().unwrap();
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
        let bot = crate::BOT.get().unwrap();

        let msg = if count == 0 {
            "今天没有比赛，但也不要松懈哦。".to_string()
        } else {
            format!("今天装填了 {} 场比赛，准备发射！", count)
        };

        let config = crate::CONFIG.get().unwrap();
        for group in config.notify_group.iter() {
            bot.send_group_msg(*group, &msg);
        }
    }
}

pub(crate) async fn update_contests() -> Result<()> {
    let mut contests = super::getter::fetch_contest().await?;
    contests.sort_by(|a, b| a.start.cmp(&b.start));
    *CONTESTS.write().await = Arc::new(contests);

    Ok(())
}

fn seconds_to_str(seconds: i64) -> String {
    let total_minutes = seconds / 60;
    let hour = total_minutes / 60 % 24;
    let minute = total_minutes % 60;
    let day = total_minutes / 60 / 24;
    if day > 0 {
        return format!("{}day {:02}h {:02}min", day, hour, minute);
    }
    format!("{:02}h {:02}min", hour, minute)
}

fn send_to_super_admin(msg: &str) {
    let bot = crate::BOT.get().unwrap();
    bot.send_private_msg(bot.get_main_admin().unwrap(), msg);
}
