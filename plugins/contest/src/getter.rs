use std::sync::{Arc, LazyLock};

use anyhow::{Ok, Result};
use kovi::{
    serde_json::{self, Value},
    tokio,
};
use reqwest::Response;

use crate::{CONFIG, contest::Contest};

static LOCK: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

async fn fetch(url: &str) -> Result<Response> {
    // 同时只有一个请求可以发出
    let _lock = LOCK.lock().await;
    let res = reqwest::get(url).await?;
    // 避免请求过快
    // 这里的时间可以根据实际情况调整
    kovi::tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    Ok(res)
}

pub async fn fetch_contest() -> Result<Vec<Arc<Contest>>> {
    let mut contests = Vec::new();

    let config = CONFIG.get().unwrap();

    for contest_id in config.clist_contest.iter() {
        let url = format!(
            "https://clist.by/api/v4/json/contest/?resource={}&filtered=false&order_by=-start&limit=20&offset=0&username={}&api_key={}",
            contest_id, config.username, config.api_key
        );

        let res = fetch(url.as_str()).await?;
        let body = res.json::<Value>().await?;

        let contests_data = match body {
            Value::Object(mut map) => {
                let contests = map.remove("objects");
                match contests {
                    Some(Value::Array(contests)) => contests,
                    _ => return Err(anyhow::anyhow!("Invalid response")),
                }
            }
            _ => return Err(anyhow::anyhow!("Invalid response")),
        };

        contests.extend(
            contests_data
                .into_iter()
                .map(serde_json::from_value)
                .filter_map(Result::ok)
                .map(Arc::new),
        );
    }

    Ok(contests)
}
