use std::{
    process::Stdio,
    sync::{Arc, LazyLock},
    time::Duration,
};

use anyhow::{Error, Result};
use base64::{Engine, engine::general_purpose};
use kovi::{
    log::debug,
    serde_json::{self, Value},
    tokio::{self, io::AsyncWriteExt},
};
use moka::future::Cache;

use crate::CONFIG;

static OCR_MEMORY: LazyLock<OcrMemory> = LazyLock::new(OcrMemory::new);

struct OcrMemory {
    cache: Cache<Arc<String>, Arc<String>>,
}

impl OcrMemory {
    fn new() -> Self {
        let cache = Cache::builder()
            .max_capacity(50)
            .time_to_live(Duration::from_secs(60 * 60 * 24))
            .time_to_idle(Duration::from_secs(60 * 64 * 10))
            .build();
        Self { cache }
    }

    async fn get_or_insert(&self, key: Arc<String>) -> Result<Arc<String>> {
        if let Some(value) = self.cache.get(&key).await {
            return Ok(value);
        }

        let guard = self
            .cache
            .entry(Arc::clone(&key))
            .or_try_insert_with(async {
                let v = get_ocr(&key).await?;
                debug!("OCR cache failed");
                anyhow::Ok(Arc::new(v))
            })
            .await
            .map_err(|e| Error::msg(e.to_string()))?;

        let value = guard.value();

        if value.is_empty() {
            return Err(Error::msg("OCR result is empty"));
        }

        Ok(Arc::clone(value))
    }
}

async fn get_ocr(img_base64: &str) -> Result<String> {
    let config = CONFIG.get().unwrap();

    let mut child = tokio::process::Command::new(&config.python_path)
        .arg(&config.ocr_path)
        .env("SECRET_ID", &config.secret_id)
        .env("SECRET_KEY", &config.secret_key)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(img_base64.as_bytes())
        .await?;

    let output = child.wait_with_output().await?;

    if !output.status.success() {
        Err(anyhow::anyhow!(
            "OCR failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    } else {
        let req = serde_json::from_slice(&output.stdout)?;

        let arr = if let Value::Object(mut res) = req
            && let Some(Value::Array(arr)) = res.remove("TextDetections")
        {
            arr
        } else {
            return Err(anyhow::anyhow!("Invalid response format"));
        };

        let result = arr
            .into_iter()
            .filter_map(|item| {
                if let Value::Object(mut item_map) = item
                    && let Some(Value::String(text)) = item_map.remove("DetectedText")
                {
                    Some(text)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        Ok(result)
    }
}

pub async fn ocr(img_url: &str) -> Result<Arc<String>> {
    let base64 = Arc::new(get_img_base64_from_url(img_url).await?);
    let result = OCR_MEMORY.get_or_insert(base64).await?;

    Ok(result)
}

async fn get_img_base64_from_url(img_url: &str) -> Result<String> {
    let req = reqwest::get(img_url).await?;
    if !req.status().is_success() {
        return Err(anyhow::anyhow!("Failed to get image"));
    }
    let bytes = req.bytes().await?;
    let base64 = general_purpose::STANDARD.encode(&bytes);
    Ok(base64)
}
