use anyhow::Result;
use kovi::{log::info, serde_json, tokio};

use crate::CONFIG;

pub async fn ocr(img_url: &str) -> Result<String> {
    let config = CONFIG.get().unwrap();

    let output = tokio::process::Command::new(&config.python_path)
        .arg(&config.ocr_path)
        .env("SECRET_ID", &config.secret_id)
        .env("SECRET_KEY", &config.secret_key)
        .env("IMAGE_URL", img_url)
        .output()
        .await?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "OCR failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    } else {
        let result = String::from_utf8(output.stdout)?;
        let req = serde_json::from_str::<serde_json::Value>(&result)?;

        let result = match req {
            serde_json::Value::Object(mut res) => match res.remove("TextDetections") {
                Some(serde_json::Value::Array(res)) => res
                    .into_iter()
                    .filter_map(|item| {
                        if let serde_json::Value::Object(mut item_map) = item {
                            if let Some(serde_json::Value::String(text)) =
                                item_map.remove("DetectedText")
                            {
                                return Some(text);
                            }
                        }
                        None
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
                _ => {
                    return Err(anyhow::anyhow!("Invalid response format"));
                }
            },
            _ => {
                return Err(anyhow::anyhow!("Invalid response format"));
            }
        };

        Ok(result)
    }
}
