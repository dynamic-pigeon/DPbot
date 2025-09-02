use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use kovi::{Message, MsgEvent};

use crate::{
    CONFIG, PATH,
    utils::{IdOrText, user_id_or_text},
};

pub async fn rating(event: &MsgEvent, args: &[String]) {
    let user = match args.get(2).map(|s| user_id_or_text(s)).unwrap() {
        Ok(v) => v,
        Err(_) => {
            event.reply("参数非法");
            return;
        }
    };

    let cf_id = match get_cf_id(&user).await {
        Ok(cf_id) => cf_id,
        Err(e) => {
            event.reply(e.to_string());
            return;
        }
    };

    let path = PATH.get().unwrap().join("codeforces");
    let py_analyzer_path = CONFIG.get().unwrap().py_analyzer_path.clone();
    let py_path = path.join("rating.py");

    event.reply("正在查询用户rating记录");

    let output: std::process::Output = match crate::utils::wait(async move {
        kovi::tokio::process::Command::new(py_analyzer_path)
            .arg(py_path)
            .arg(cf_id)
            .output()
            .await
            .map_err(anyhow::Error::from)
    })
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

    let image = output.stdout;
    let image = STANDARD.encode(image);

    event.reply(Message::new().add_image(&format!("base64://{}", image)));
}

pub async fn analyze(event: &MsgEvent, args: &[String]) {
    let user = match args.get(2).map(|s| user_id_or_text(s)).unwrap() {
        Ok(v) => v,
        Err(_) => {
            event.reply("参数非法");
            return;
        }
    };

    let cf_id = match get_cf_id(&user).await {
        Ok(cf_id) => cf_id,
        Err(e) => {
            event.reply(e.to_string());
            return;
        }
    };

    let path = PATH.get().unwrap().join("codeforces");
    let py_analyzer_path = CONFIG.get().unwrap().py_analyzer_path.clone();
    let py_path = path.join("analyze.py");

    event.reply("正在查询用户做题记录");

    let output: std::process::Output = match crate::utils::wait(async move {
        kovi::tokio::process::Command::new(py_analyzer_path)
            .arg(py_path)
            .arg(cf_id)
            .output()
            .await
            .map_err(anyhow::Error::from)
    })
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

    let image_raw = output.stdout;
    let image = STANDARD.encode(image_raw);

    event.reply(Message::new().add_image(&format!("base64://{}", image)));
}

async fn get_cf_id(uit: &IdOrText<'_>) -> Result<String> {
    match uit {
        IdOrText::At(qq) => {
            let user = crate::sql::duel::user::get_user(*qq)
                .await
                .map_err(|_| anyhow::anyhow!("未找到用户"))?;
            Ok(user
                .cf_id
                .ok_or_else(|| anyhow::anyhow!("用户未绑定 cf 账号"))?)
        }
        IdOrText::Text(text) => Ok(text.to_string()),
    }
}
