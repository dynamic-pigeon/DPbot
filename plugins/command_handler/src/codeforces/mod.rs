use std::{path::Path, sync::LazyLock};

use anyhow::Result;
use kovi::{Message, MsgEvent, log::info, tokio::sync::Mutex};

use crate::{
    PATH,
    utils::{UIT, user_id_or_text, user_id_or_text_str},
};

static RATING_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static ANALYZE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

pub async fn rating(event: &MsgEvent, args: &[String]) {
    let user = args.get(2).map(|s| user_id_or_text(&s)).unwrap();

    let cf_id = match user_cf_id(&user).await {
        Ok(cf_id) => cf_id,
        Err(e) => {
            event.reply(e.to_string());
            return;
        }
    };

    let path = PATH.get().unwrap().join("codeforces");
    let py_analyzer_path = path.join(".venv/bin/python");
    let py_path = path.join("rating.py");
    let image_path = path.join("rating.png");
    let image_path = image_path.to_str().unwrap();

    let _lock = RATING_LOCK.lock().await;

    event.reply("正在查询用户rating记录");

    let output = match kovi::tokio::process::Command::new(py_analyzer_path)
        .arg(py_path)
        .arg(cf_id)
        .arg(image_path)
        .output()
        .await
    {
        Ok(output) => output,
        Err(e) => {
            event.reply(e.to_string());
            return;
        }
    };

    if !output.status.success() {
        event.reply(format!(
            "查询失败: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
        return;
    }

    info!("image_path: {}", image_path);

    if !Path::new(&image_path.to_string()).exists() {
        event.reply("查询失败: 未知错误");
        return;
    }

    event.reply(Message::new().add_image(&format!("file:///{}", image_path)));
}

pub async fn analyze(event: &MsgEvent, args: &[String]) {
    let user = args.get(2).map(|s| user_id_or_text(&s)).unwrap();

    let cf_id = match user_cf_id(&user).await {
        Ok(cf_id) => cf_id,
        Err(e) => {
            event.reply(e.to_string());
            return;
        }
    };

    let path = PATH.get().unwrap().join("codeforces");
    let py_analyzer_path = path.join(".venv/bin/python");
    let py_path = path.join("analyze.py");
    let image_path = path.join("analyze.png");
    let image_path = image_path.to_str().unwrap();

    let _lock = ANALYZE_LOCK.lock().await;

    event.reply("正在查询用户contest记录");

    let output = match kovi::tokio::process::Command::new(py_analyzer_path)
        .arg(py_path)
        .arg(cf_id)
        .arg(image_path)
        .output()
        .await
    {
        Ok(output) => output,
        Err(e) => {
            event.reply(e.to_string());
            return;
        }
    };

    if !output.status.success() {
        event.reply(format!(
            "查询失败: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
        return;
    }

    info!("image_path: {}", image_path);

    if !Path::new(&image_path.to_string()).exists() {
        event.reply("查询失败: 未知错误");
        return;
    }

    event.reply(Message::new().add_image(&format!("file:///{}", image_path)));
}

async fn user_cf_id<'a>(uit: &UIT<'a>) -> Result<String> {
    match uit {
        UIT::At(qq) => {
            let user = crate::sql::duel::user::get_user(*qq)
                .await
                .map_err(|_| anyhow::anyhow!("未找到用户"))?;
            Ok(user
                .cf_id
                .ok_or_else(|| anyhow::anyhow!("用户未绑定 cf 账号"))?)
        }
        UIT::Text(text) => Ok(text.to_string()),
    }
}
