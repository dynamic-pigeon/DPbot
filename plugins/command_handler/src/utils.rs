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

pub fn user_id_or_text(text: &str) -> Result<IdOrText<'_>> {
    if let Some(user_id) = text.strip_prefix("@") {
        Ok(IdOrText::At(user_id.parse()?))
    } else {
        Ok(IdOrText::Text(text))
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
    fetch_cf_api(async move { reqwest::get(url).await.map_err(Into::into) }).await
}

pub(crate) async fn fetch_cf_api<F: Future>(future: F) -> F::Output {
    utils::api_limit::limit_api_call("codeforces", std::time::Duration::from_secs(2), 1, future)
        .await
}

pub async fn get_user_rating(cf_id: &str) -> Result<i64> {
    let res = fetch(&format!(
        "https://codeforces.com/api/user.info?handles={}",
        cf_id
    ))
    .await
    .map_err(|_| anyhow::anyhow!("Failed to fetch user info"))?;

    let body: Value = res
        .json()
        .await
        .map_err(|_| anyhow::anyhow!("Failed to parse response"))?;

    let status = body["status"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid response"))?;
    if status != "OK" {
        return Err(anyhow::anyhow!("API returned error"));
    }

    let users = body["result"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No user found"))?;
    if users.is_empty() {
        return Err(anyhow::anyhow!("User not found"));
    }

    let user = &users[0];
    let rating = user["rating"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("User has no rating"))?;

    Ok(rating)
}
