use std::sync::Arc;

use anyhow::{Ok, Result};
use kovi::serde_json::{self, Value};
use reqwest::Response;
use utils::retry::retry;

use crate::{CONFIG, contest::Contest};

async fn fetch(url: &str) -> Result<Response> {
    utils::api_limit::limit_api_call("clist", std::time::Duration::from_secs(1), 1, async move {
        reqwest::get(url).await.map_err(Into::into)
    })
    .await
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
