use std::sync::Arc;

use anyhow::{Ok, Result};
use kovi::{
    log::info,
    serde_json::{self, Value},
};

use crate::{CONFIG, contest::Contest};

pub async fn fetch_contest() -> Result<Vec<Arc<Contest>>> {
    let mut contests = Vec::new();

    let config = {
        let config = CONFIG.get().unwrap();
        Arc::clone(&*config)
    };

    for contest_id in config.clist_contest.iter() {
        let url = format!(
            "https://clist.by/api/v4/json/contest/?resource={}&filtered=false&order_by=-start&limit=20&offset=0&username={}&api_key={}",
            contest_id, config.username, config.api_key
        );

        kovi::tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let res = reqwest::get(&url).await?;
        let body = res.json::<serde_json::Value>().await?;

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
