use config::Config;
use kovi::MsgEvent;
use kovi::bot::message::Segment;
use kovi::bot::runtimebot::kovi_api::SetAccessControlList;
use kovi::serde_json::{Value, json};
use kovi::utils::load_json_data;
use kovi::{Message, PluginBuilder as plugin};
use std::iter::Iterator;
use std::sync::Arc;

mod config;

const PLUGINS: &[&str] = &[
    "command_handler",
    "manager",
    "contest",
    "aichat",
    "word_cloud",
];

#[kovi::plugin]
async fn main() {
    let bot = plugin::get_runtime_bot();
    let data_path = bot.get_data_path();
    let config_path = data_path.join("config.json");
    let config = load_json_data(Config::empty(), config_path).unwrap();

    // Initialize the whitelist
    let whitelist = &config.whitelist;

    for plugin_name in PLUGINS {
        bot.set_plugin_access_control(plugin_name, true).unwrap();
        bot.set_plugin_access_control_list(
            plugin_name,
            true,
            SetAccessControlList::Changes(whitelist.clone()),
        )
        .unwrap();
    }

    let help_path = data_path.join("help.json");
    let help = load_json_data(Value::default(), help_path).unwrap();
    let help = Arc::new(help);

    let duel_help_path = data_path.join("duel_help.json");
    let duel_help = load_json_data(Value::default(), duel_help_path).unwrap();
    let duel_help = Arc::new(duel_help);

    plugin::on_msg(move |event| {
        let help = help.clone();
        let duel_help = duel_help.clone();
        async move {
            let text = event.borrow_text().unwrap_or_default();
            if text.starts_with("/help") {
                handle_help(&event, &help, &duel_help).await;
            }
        }
    });
}

async fn handle_help(event: &MsgEvent, help: &Value, duel_help: &Value) {
    let text = event.borrow_text().unwrap_or_default();
    let text = text[5..].trim();

    if text.is_empty() {
        let list = help
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        event.reply(format!(
            "本群组中可用的模块如下：\n{}\n输入 /help [模块名称] 查询详细用法",
            list
        ));
        return;
    }

    // 特殊处理
    if text == "duel" {
        let arr = match duel_help {
            Value::Array(arr) => arr,
            _ => {
                event.reply("未找到该模块");
                return;
            }
        };

        let segs = arr
            .iter()
            .map(|v| {
                Segment::new(
                    "node",
                    json!({
                        "content": [v]
                    }),
                )
            })
            .collect::<Vec<_>>();

        let msg = Message::from(segs);

        event.reply(msg);

        return;
    }

    let cmd = match help.get(text) {
        Some(cmd) => cmd,
        None => {
            event.reply("未找到该模块");
            return;
        }
    };

    let msg = match cmd {
        Value::String(s) => s.clone(),
        Value::Array(obj) => obj
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => {
            event.reply("未找到该模块");
            return;
        }
    };

    event.reply(msg);
}
