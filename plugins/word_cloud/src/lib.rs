use std::{
    path::Path,
    process::Stdio,
    sync::{Arc, OnceLock},
};

use kovi::{
    Message, PluginBuilder as plugin, RuntimeBot, chrono,
    log::{self, debug, info},
    tokio::{self, io::AsyncWriteExt},
};

use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};

mod ocr;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[kovi::plugin]
async fn main() {
    let bot = plugin::get_runtime_bot();
    let path = bot.get_data_path();

    let config_path = path.join("config.json");
    let config = kovi::utils::load_json_data(Default::default(), config_path).unwrap();

    debug!("config: {:?}", config);

    CONFIG.get_or_init(|| config);

    let db_path = path.join("word_cloud.db");

    if !db_path.exists() {
        std::fs::create_dir_all(&path).unwrap();
        std::fs::File::create(&db_path).unwrap();
    }

    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(3)
        .connect(db_path.to_str().unwrap())
        .await
        .unwrap();

    let db = Arc::new(db);

    init(&db).await;

    let db_clone = Arc::clone(&db);

    let path = Arc::new(path);

    plugin::cron("0 21 * * *", move || {
        let path = Arc::clone(&path);
        let bot = Arc::clone(&bot);
        let db = Arc::clone(&db);
        async move {
            let config = CONFIG.get().unwrap();
            let notify_group = &config.notify_group;

            for &group_id in notify_group {
                let bot = Arc::clone(&bot);
                let path = Arc::clone(&path);
                let db = Arc::clone(&db);
                kovi::spawn(async move {
                    send_word_cloud(&bot, group_id, &path, &db).await;
                });
            }

            remove_before(&db, chrono::Utc::now() - chrono::Duration::days(7)).await;
        }
    })
    .unwrap();

    plugin::on_group_msg(move |event| {
        let db = Arc::clone(&db_clone);
        async move {
            let group_id = event.group_id;

            if !CONFIG.get().unwrap().notify_group.contains(&group_id) {
                return;
            }
            let msg = get_text(&event.message).await;

            if msg.is_empty() {
                return;
            }

            add_msg(&db, group_id, &msg).await;
        }
    });
}

async fn get_text(msg: &Message) -> String {
    let mut res = String::new();

    for seg in msg.iter() {
        if !res.is_empty() {
            res.push(' ');
        }
        match seg.type_.as_str() {
            "text" => res.push_str(seg.data["text"].as_str().unwrap()),
            "image" => match ocr::ocr(seg.data["url"].as_str().unwrap()).await {
                Ok(tx) => {
                    res.push_str(&tx);
                }
                Err(e) => {
                    log::error!("ocr failed: {}", e);
                }
            },
            _ => {}
        }
    }

    res
}

async fn init(db: &sqlx::SqlitePool) {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS group_message 
    (group_id INTEGER, message TEXT, time TEXT)
        "#,
    )
    .execute(db)
    .await
    .unwrap();
}

async fn add_msg(db: &sqlx::SqlitePool, group_id: i64, message: &str) {
    sqlx::query(
        r#"
        INSERT INTO group_message (group_id, message, time) VALUES (?, ?, ?)
        "#,
    )
    .bind(group_id)
    .bind(message)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(db)
    .await
    .unwrap();
}

async fn make_word_cloud(path: &Path, notify_group: i64, db: &sqlx::SqlitePool) -> Result<Vec<u8>> {
    let end_time = chrono::Utc::now();
    let start_time = end_time - chrono::Duration::days(1) - chrono::Duration::minutes(10);

    let messages = select_from_range(db, notify_group, start_time, end_time)
        .await?
        .join(" ");

    let msg = jieba_rs::Jieba::new();
    let messages = msg
        .cut(&messages, true)
        .into_iter()
        .filter(|s| s.chars().count() > 1)
        .collect::<Vec<_>>()
        .join(" ");

    let wc_cli = CONFIG.get().unwrap().wordcloud_cli_path.clone();
    let mask_path = path.join("mask.jpg");
    let stop_word_path = path.join("stopword.txt");
    let fontfile_path = path.join("font.otf");

    let mut child = tokio::process::Command::new(wc_cli)
        .arg("--mask")
        .arg(mask_path)
        .args(["--background", "white"])
        .arg("--stopwords")
        .arg(stop_word_path)
        .arg("--fontfile")
        .arg(fontfile_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    child
        .stdin
        .take()
        .unwrap()
        .write_all(messages.as_bytes())
        .await?;

    let output = child.wait_with_output().await?;

    Ok(output.stdout)
}

async fn send_word_cloud(bot: &RuntimeBot, group_id: i64, path: &Path, db: &sqlx::SqlitePool) {
    let image = match make_word_cloud(path, group_id, db).await {
        Ok(image) if !image.is_empty() => image,
        Ok(image) => {
            assert!(image.is_empty());
            info!("word cloud is empty, group_id: {}", group_id);
            return;
        }
        Err(e) => {
            log::error!("make word cloud failed: {}, group_id: {}", e, group_id);
            bot.send_private_msg(
                bot.get_main_admin().unwrap(),
                format!("make word cloud failed: {}, group_id: {}", e, group_id),
            );
            return;
        }
    };

    info!("send word cloud to group: {}", group_id);

    let image_base64 = STANDARD.encode(&image);
    let image = format!("base64://{}", image_base64);
    bot.send_group_msg(
        group_id,
        Message::new().add_text("今日词云").add_image(&image),
    );
}

async fn select_from_range(
    db: &sqlx::SqlitePool,
    group_id: i64,
    start_time: chrono::DateTime<chrono::Utc>,
    end_time: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<String>> {
    let result: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT message FROM group_message WHERE group_id = ? AND time BETWEEN ? AND ?
        "#,
    )
    .bind(group_id)
    .bind(start_time.to_rfc3339())
    .bind(end_time.to_rfc3339())
    .fetch_all(db)
    .await?;

    Ok(result.into_iter().map(|(msg,)| msg).collect())
}

async fn remove_before(db: &sqlx::SqlitePool, time: chrono::DateTime<chrono::Utc>) {
    sqlx::query(
        r#"
        DELETE FROM group_message WHERE time < ?
        "#,
    )
    .bind(time.to_rfc3339())
    .execute(db)
    .await
    .unwrap();
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct Config {
    pub wordcloud_cli_path: String,
    pub notify_group: Vec<i64>,
    #[serde(rename = "SecretId")]
    pub secret_id: String,
    #[serde(rename = "SecretKey")]
    pub secret_key: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            wordcloud_cli_path: "wordcloud_cli".to_string(),
            notify_group: vec![],
            secret_id: "".to_string(),
            secret_key: "".to_string(),
        }
    }
}
