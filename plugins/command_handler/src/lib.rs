use std::sync::Arc;

use anyhow::{Error, Result};
use duel::handlers;
use kovi::chrono::Utc;
use kovi::serde_json::{self, Value};
use kovi::{Message, MsgEvent, PluginBuilder as plugin, chrono, tokio};

pub(crate) mod duel;
pub(crate) mod sql;

#[kovi::plugin]
async fn main() {
    let bot = plugin::get_runtime_bot();
    let data_path = bot.get_data_path();

    let sql_path = data_path.join("data.db");
    sql::init(sql_path.to_str().unwrap()).await.unwrap();

    duel::init().await;

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

    let (cmd, changed) = match change(&mut args, &command) {
        Ok((cmd, changed)) => (cmd, changed),
        Err(e) => {
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
        _ => {
            event.reply("还没写好");
        }
    }
}

fn mes_to_text(msg: &Message) -> String {
    let mut text = String::new();
    for seg in msg.iter() {
        match seg.type_.as_str() {
            "text" => {
                text.push_str(&seg.data["text"].as_str().unwrap());
            }
            "at" => {
                text.push_str(seg.data["qq"].as_str().unwrap());
            }
            _ => {}
        }
    }
    text
}

fn today_utc() -> chrono::DateTime<Utc> {
    let offset = chrono::FixedOffset::east_opt(8 * 3600).unwrap();
    chrono::Utc::now().with_timezone(&offset).to_utc()
}

// 解析指令并替换
fn change(args: &mut Vec<String>, command: &Value) -> Result<(String, bool)> {
    let mut changed = false;

    let mut point = command;

    let mut i = 0;
    let s = loop {
        let map = match point {
            Value::String(s) => break s.clone(),
            Value::Object(obj) => obj,
            _ => {
                unreachable!("Invalid command");
            }
        };

        if i >= args.len() {
            return Err(Error::msg("Invalid command"));
        }

        let mut key = None;
        let mut best_match = 0.0;
        let mut flag = false;
        for (k, _) in map {
            let diff = strsim::normalized_levenshtein(&k, &args[i]);
            if diff > 0.6 && diff > best_match {
                key = Some(k);
                best_match = diff;
                flag = true;
            }
            if (diff - 1.0).abs() < 1e-6 {
                flag = false;
                break;
            }
        }

        if key.is_none() {
            return Err(Error::msg("Invalid command"));
        }

        if flag {
            args[i] = key.unwrap().clone();
            changed = true;
        }

        point = &map[key.unwrap()];

        i += 1;
    };

    Ok((s, changed))
}
