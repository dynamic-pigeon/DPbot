use std::sync::{Arc, LazyLock};

use kovi::{
    Message, PluginBuilder as plugin, RuntimeBot,
    bot::message::Segment,
    chrono::{DateTime, FixedOffset, Utc},
    log::info,
    serde_json::json,
    tokio::sync::RwLock,
    utils::load_json_data,
};

pub(crate) mod contest;
pub(crate) mod getter;

static CONFIG: LazyLock<RwLock<Arc<Config>>> =
    LazyLock::new(|| RwLock::new(Arc::new(Config::empty())));

static BOT: LazyLock<RwLock<Option<Arc<RuntimeBot>>>> = LazyLock::new(|| RwLock::new(None));

#[kovi::plugin]
async fn main() {
    let bot = plugin::get_runtime_bot();
    BOT.write().await.replace(Arc::clone(&bot));
    let data_path = bot.get_data_path();

    let config_path = data_path.join("config.json");
    let config = load_json_data(Config::empty(), config_path).unwrap();
    *CONFIG.write().await = Arc::new(config);

    plugin::cron("0 0 * * *", || async {
        contest::daily_init().await;
    })
    .unwrap();

    let _ = contest::init().await;

    info!("contest load sucessfully.");
    plugin::on_msg(|event| async move {
        let Some(text) = event.borrow_text() else {
            return;
        };

        if !text.starts_with("/contest") {
            return;
        }

        let contests = contest::get_all_contests().await;
        let mut msg = String::new();
        for contest in contests.iter().cloned() {
            let add = format!(
                "{}\n[duaration: {}]\n{}\n{}\n\n",
                contest.event,
                contest.duration(),
                contest.start_time(),
                contest.href
            );

            msg.push_str(&add);
        }
        let seg = Segment::new(
            "node",
            json!({
                "user_id": "2722708584",
                "nickname": "呵呵哒",
                "content": [{
                    "type": "text",
                    "data": {
                        "text": msg
                    }
                }]
            }),
        );

        let msg = Message::from(vec![seg]);
        event.reply(msg);
    });
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Config {
    pub notify_group: Vec<i64>,
    pub notify_time: Vec<i64>,
    pub clist_contest: Vec<String>,
    pub api_key: String,
    pub username: String,
}

impl Config {
    pub fn empty() -> Self {
        Self {
            notify_group: vec![],
            notify_time: vec![],
            clist_contest: vec![],
            api_key: "".to_string(),
            username: "".to_string(),
        }
    }
}

fn today_utc() -> DateTime<Utc> {
    let offset = FixedOffset::east_opt(8 * 3600).unwrap();
    Utc::now().with_timezone(&offset).to_utc()
}
