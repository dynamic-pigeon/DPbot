use std::sync::{Arc, OnceLock};

use duel::{config, handlers};
use kovi::serde_json::{self, Value};
use kovi::utils::load_json_data;
use kovi::{MsgEvent, PluginBuilder as plugin, tokio};
use utils::{change, mes_to_text};

pub(crate) mod codeforces;
pub(crate) mod duel;
pub(crate) mod sql;
pub(crate) mod utils;

static PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
static CONFIG: OnceLock<Arc<utils::Config>> = OnceLock::new();

#[kovi::plugin]
async fn main() {
    let bot = plugin::get_runtime_bot();
    let data_path = bot.get_data_path();

    PATH.get_or_init(|| data_path.clone());

    let sql_path = data_path.join("data.db");
    sql::init(sql_path.to_str().unwrap()).await.unwrap();

    duel::init().await;

    let config_path = data_path.join("config.json");
    let config = load_json_data(Default::default(), config_path).unwrap();
    CONFIG.get_or_init(|| Arc::new(config));

    let command_path = data_path.join("command.json");

    let command: Value =
        serde_json::from_reader(std::fs::File::open(&command_path).unwrap()).unwrap();

    let command = Arc::new(command);

    plugin::on_msg(move |e| {
        let command = command.clone();
        async move {
            handle(e, &command).await;
        }
    });
}

async fn handle(event: Arc<MsgEvent>, command: &Value) {
    let text = mes_to_text(&event.message);

    let text = text.trim();
    if !text.starts_with("/") {
        return;
    }

    let text = &text[1..];

    let mut args = text
        .split_whitespace()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let (cmd, changed) = match change(&mut args, command) {
        Ok((cmd, changed)) => (cmd, changed),
        Err(_e) => {
            // event.reply(e.to_string());
            return;
        }
    };

    if changed {
        let new_text = format!("指令被解析为 /{}", args.join(" "));
        event.reply(new_text);
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    match cmd.as_str() {
        "analyze" => {
            codeforces::analyze(&event, &args).await;
        }
        "rating" => {
            codeforces::rating(&event, &args).await;
        }
        "daily_problem" => {
            handlers::daily_problem(&event).await;
        }
        "bind" => {
            handlers::bind(&event, &args).await;
        }
        "finish_bind" => {
            handlers::finish_bind(&event).await;
        }
        "problem" => {
            handlers::problem(&event, &args).await;
        }
        "challenge" => {
            handlers::challenge(&event, &args).await;
        }
        "accept" => {
            handlers::accept(&event).await;
        }
        "decline" => {
            handlers::decline(&event).await;
        }
        "cancel" => {
            handlers::cancel(&event).await;
        }
        "change" => {
            handlers::change(&event).await;
        }
        "judge" => {
            handlers::judge(&event).await;
        }
        "give_up" => {
            handlers::give_up(&event).await;
        }
        "daily_finish" => {
            handlers::daily_finish(&event).await;
        }
        "daily_ranklist" => {
            handlers::daily_ranklist(&event).await;
        }
        "ranklist" => {
            handlers::ranklist(&event).await;
        }
        "ongoing" => {
            handlers::ongoing(&event).await;
        }
        _ => {
            event.reply("还没写好");
        }
    }
}
