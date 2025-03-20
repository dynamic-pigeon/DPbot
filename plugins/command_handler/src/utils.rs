use kovi::{
    Message,
    chrono::{self, Utc},
};

use anyhow::{Error, Result};
use kovi::serde_json::Value;

pub enum UIT<'a> {
    Text(&'a str),
    At(i64),
}

pub fn user_id_or_text(text: &str) -> UIT {
    if let Some(user_id) = text.strip_prefix("@") {
        UIT::At(user_id.parse().unwrap())
    } else {
        UIT::Text(text)
    }
}

pub fn user_id_or_text_str(text: &str) -> &str {
    if let Some(user_id) = text.strip_prefix("@") {
        user_id
    } else {
        text
    }
}

pub fn mes_to_text(msg: &Message) -> String {
    let mut text = String::new();
    for seg in msg.iter() {
        match seg.type_.as_str() {
            "text" => {
                text.push_str(seg.data["text"].as_str().unwrap());
            }
            "at" => {
                text.push_str(&format!("@{}", seg.data["qq"].as_str().unwrap()));
            }
            _ => {}
        }
    }
    text
}

pub fn today_utc() -> chrono::DateTime<Utc> {
    let offset = chrono::FixedOffset::east_opt(8 * 3600).unwrap();
    chrono::Utc::now().with_timezone(&offset).to_utc()
}

// 解析指令并替换
pub fn change(args: &mut [String], command: &Value) -> Result<(String, bool)> {
    let mut changed = false;

    let mut point = command;

    let mut i = 0;
    let s = loop {
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

    Ok((s, changed))
}
