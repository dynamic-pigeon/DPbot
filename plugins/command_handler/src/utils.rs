use kovi::{
    Message,
    chrono::{self, Utc},
};

use anyhow::{Error, Result};
use kovi::serde_json::Value;

pub enum IdOrText<'a> {
    Text(&'a str),
    At(i64),
}

pub fn user_id_or_text(text: &str) -> IdOrText {
    if let Some(user_id) = text.strip_prefix("@") {
        IdOrText::At(user_id.parse().unwrap())
    } else {
        IdOrText::Text(text)
    }
}

#[allow(dead_code)]
pub fn user_id_or_text_str(text: &str) -> &str {
    if let Some(user_id) = text.strip_prefix("@") {
        user_id
    } else {
        text
    }
}

pub fn mes_to_text(msg: &Message) -> String {
    msg.iter()
        .filter_map(|seg| match seg.type_.as_str() {
            "text" => Some(seg.data["text"].as_str().unwrap().to_string()),
            "at" => Some(format!("@{}", seg.data["qq"].as_str().unwrap())),
            _ => None,
        })
        .collect::<String>()
}

pub fn today_utc() -> chrono::DateTime<Utc> {
    let offset = chrono::FixedOffset::east_opt(8 * 3600).unwrap();
    chrono::Utc::now().with_timezone(&offset).to_utc()
}

// 解析指令并替换
pub fn change(args: &mut [String], commands: &Value) -> Result<(String, bool)> {
    let mut changed = false;

    let mut point = commands;

    let mut i = 0;
    let command = loop {
        let map = match point {
            Value::String(s) => break s.clone(),
            Value::Object(obj) => obj,
            _ => unreachable!("Invalid command"),
        };

        if i >= args.len() {
            return Err(Error::msg("Invalid command"));
        }

        let mut key = None;
        let mut best_match = 0.0;
        let mut flag = false;
        for (k, _) in map {
            let diff = strsim::normalized_damerau_levenshtein(k, &args[i]);
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

    Ok((command, changed))
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct Config {
    pub py_analyzer_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            py_analyzer_path: "".to_string(),
        }
    }
}
