use std::sync::LazyLock;

use kovi::{
    Message,
    chrono::{self, Utc},
    tokio,
};

use anyhow::{Error, Result};
use kovi::serde_json::Value;

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

pub enum IdOrText<'a> {
    Text(&'a str),
    At(i64),
}

pub fn user_id_or_text(text: &str) -> Result<IdOrText<'_>> {
    if let Some(user_id) = text.strip_prefix("@") {
        Ok(IdOrText::At(user_id.parse()?))
    } else {
        Ok(IdOrText::Text(text))
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

#[inline]
pub fn today_utc() -> chrono::DateTime<Utc> {
    let offset = chrono::FixedOffset::east_opt(8 * 3600).unwrap();
    chrono::Utc::now().with_timezone(&offset).to_utc()
}

// 解析指令并替换
pub fn change(args: &mut [String], commands: &Value) -> Result<(String, bool)> {
    let mut changed = false;

    let command = args.iter_mut().try_fold(commands, |point, arg| {
        let map = match point {
            Value::String(_) => return Ok(point),
            Value::Object(map) => map,
            _ => return Err(Error::msg("Invalid command structure")),
        };

        let (key, flag) = map
            .iter()
            .filter_map(|(k, _)| {
                let diff = strsim::normalized_damerau_levenshtein(k, arg);
                if diff > 0.6 { Some((k, diff)) } else { None }
            })
            .max_by(|(_, diff1), (_, diff2)| {
                diff1
                    .partial_cmp(diff2)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(k, diff)| {
                let flag = (diff - 1.0).abs() >= 1e-6;
                (k, flag)
            })
            .ok_or_else(|| Error::msg("Invalid command"))?;

        if flag {
            changed = true;
            *arg = key.to_string();
        }

        map.get(key).ok_or_else(|| Error::msg("Invalid command"))
    })?;

    let command = match command {
        Value::String(cmd) => cmd.clone(),
        _ => return Err(Error::msg("Invalid command structure")),
    };

    Ok((command, changed))
}

pub(crate) async fn fetch(url: &str) -> Result<reqwest::Response> {
    wait(async move { reqwest::get(url).await.map_err(Into::into) }).await
}

/// 用于对 api 的访问，防止访问过快被 api ban 了
///
/// ## 参数
/// - `f`: Future
pub(crate) async fn wait<F: Future>(future: F) -> F::Output {
    static LOCK: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));
    let lock = LOCK.lock().await;
    kovi::spawn(async move {
        let _ = lock;
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    });
    let res = future.await;
    res
}
