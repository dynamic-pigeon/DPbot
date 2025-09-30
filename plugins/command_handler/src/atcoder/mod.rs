use base64::{Engine, engine::general_purpose::STANDARD};
use kovi::{Message, MsgEvent, tokio};

use crate::{CONFIG, PATH};

pub async fn rating(event: &MsgEvent, args: &[String]) {
    let at_id = args.get(2).unwrap();

    let path = PATH.get().unwrap().join("atcoder");
    let py_analyzer_path = CONFIG.get().unwrap().py_analyzer_path.clone();
    let py_path = path.join("rating.py");

    event.reply("正在查询用户rating记录");

    let output = match tokio::process::Command::new(py_analyzer_path)
        .arg(py_path)
        .arg(at_id)
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

    let image = output.stdout;
    let image = STANDARD.encode(image);

    event.reply(Message::new().add_image(&format!("base64://{}", image)));
}
