use std::sync::{Arc, OnceLock};

use duel::handlers;
use duel::user::BindingUsers;
use kovi::serde_json::Value;
use kovi::utils::load_json_data;
use kovi::{MsgEvent, PluginBuilder as plugin, tokio};
use utils::{change, mes_to_text};

pub(crate) mod atcoder;
pub(crate) mod codeforces;
pub(crate) mod config;
pub(crate) mod duel;
pub(crate) mod error;
pub(crate) mod sql;
pub(crate) mod utils;

static PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
static CONFIG: OnceLock<utils::Config> = OnceLock::new();
static BINDING_USERS: OnceLock<BindingUsers> = OnceLock::new();

#[kovi::plugin]
async fn main() {
    let bot = plugin::get_runtime_bot();
    let data_path = bot.get_data_path();

    PATH.get_or_init(|| data_path.clone());

    let sql_path = data_path.join("data.db");
    sql::init(sql_path.to_str().unwrap()).await.unwrap();

    duel::init().await;
    BINDING_USERS.get_or_init(BindingUsers::new);

    let config_path = data_path.join("config.json");
    let config = load_json_data(Default::default(), config_path).unwrap();
    CONFIG.get_or_init(|| config);

    plugin::on_msg(|e| async move {
        handle(e, &config::COMMAND).await;
    });
}

async fn handle(event: Arc<MsgEvent>, command: &Value) {
    let text = mes_to_text(&event.message);

    let text = text.trim();
    let Some(text) = text.strip_prefix('/') else {
        return;
    };

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
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    match cmd.as_str() {
        "cf_analyze" => {
            codeforces::analyze(&event, &args).await;
        }
        "cf_rating" => {
            codeforces::rating(&event, &args).await;
        }
        "daily_problem" => {
            handlers::daily_problem(&event).await;
        }
        "bind" => {
            handlers::bind(&event, &args, BINDING_USERS.get().unwrap()).await;
        }
        "finish_bind" => {
            handlers::finish_bind(&event, BINDING_USERS.get().unwrap()).await;
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
        "at_rating" => {
            atcoder::rating(&event, &args).await;
        }
        _ => {
            event.reply("还没写好");
        }
    }
}
