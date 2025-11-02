use std::sync::{Arc, OnceLock};

use kovi::{
    Message, PluginBuilder as plugin, RuntimeBot, bot::message::Segment, serde_json::json,
    utils::load_json_data,
};

pub(crate) mod contest;
pub(crate) mod getter;

static CONFIG: OnceLock<Config> = OnceLock::new();

static BOT: OnceLock<Arc<RuntimeBot>> = OnceLock::new();

#[kovi::plugin]
async fn main() {
    let bot = plugin::get_runtime_bot();
    BOT.get_or_init(|| Arc::clone(&bot));
    let data_path = bot.get_data_path();

    let config_path = data_path.join("config.json");
    let config = load_json_data(Default::default(), config_path).unwrap();
    CONFIG.get_or_init(|| config);

    plugin::cron("0 8 * * *", || async {
        contest::daily_init().await;
    })
    .unwrap();

    kovi::spawn(contest::init());

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

impl Default for Config {
    fn default() -> Self {
        Self {
            notify_group: vec![],
            notify_time: vec![],
            clist_contest: vec![],
            api_key: "".to_string(),
            username: "".to_string(),
        }
    }
}
