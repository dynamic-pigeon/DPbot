use std::sync::{Arc, LazyLock};

use anyhow::{Ok, Result};
use kovi::{
    serde_json::{self, Value},
    tokio,
};
use reqwest::Response;

use crate::{CONFIG, contest::Contest, retry::retry};

async fn fetch(url: &str) -> Result<Response> {
    static LOCK: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));
    // 同时只有一个请求可以发出
    let lock = LOCK.lock().await;
    kovi::spawn(async move {
        let _lock = lock;
        // 避免请求过快
        // 这里的时间可以根据实际情况调整
        kovi::tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    });
    let res = reqwest::get(url).await?;
    Ok(res)
}

pub async fn fetch_contest() -> Result<Vec<Arc<Contest>>> {
    let mut contests = Vec::new();

    let config = CONFIG.get().unwrap();

    for contest_name in config.clist_contest.iter() {
        let url = format!(
            "https://clist.by/api/v4/json/contest/?resource={}&filtered=false&order_by=-start&limit=20&offset=0&username={}&api_key={}",
            contest_name, config.username, config.api_key
        );

        let res = retry(async move || fetch(&url).await, 3).await?;
        let body = res.json::<Value>().await?;

        let contests_data = if let Value::Object(mut map) = body
            && let Some(Value::Array(contests)) = map.remove("objects")
        {
            contests
        } else {
            return Err(anyhow::anyhow!("Invalid response"));
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
