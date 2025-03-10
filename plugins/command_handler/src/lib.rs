use std::sync::Arc;

use anyhow::{Error, Result};
use duel::handlers;
use kovi::log::{debug, info};
use kovi::serde_json::{self, Value};
use kovi::{MsgEvent, PluginBuilder as plugin};

pub(crate) mod duel;
pub(crate) mod sql;

#[kovi::plugin]
async fn main() {
    let bot = plugin::get_runtime_bot();
    let data_path = bot.get_data_path();

    let sql_path = data_path.join("data.db");
    sql::init(sql_path.to_str().unwrap()).await.unwrap();

    let command_path = data_path.join("command.json");

    plugin::on_msg(move |e| {
        let command: Value =
            serde_json::from_reader(std::fs::File::open(&command_path).unwrap()).unwrap();
        async move {
            handle(e, command).await;
        }
    });
}

async fn handle(event: Arc<MsgEvent>, command: Value) {
    let Some(text) = event.borrow_text() else {
        return;
    };

    info!("Received command: {:?}", text);

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
            event.reply(e.to_string());
            return;
        }
    };

    if changed {
        let new_text = format!("指令被解析为 /{}", args.join(" "));
        event.reply(new_text);
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
        _ => {
            // do nothing
        }
    }
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
            let diff = strsim::normalized_damerau_levenshtein(&k, &args[i]);
            if diff > 0.6 && diff > best_match {
                key = Some(k);
                best_match = diff;
                flag = true;
            }
            if diff == 1.0 {
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
