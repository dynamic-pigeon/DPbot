use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, LazyLock},
};

use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use html::END;
use kovi::{
    Message, PluginBuilder as plugin,
    bot::message::Segment,
    log::{debug, error, info},
    serde_json::json,
    tokio::sync::{Mutex, RwLock},
    utils::load_json_data,
};
use pulldown_cmark::Options;

mod config;
mod html;
mod req;
mod screen_shot;

static MESSAGE: LazyLock<RwLock<HashMap<i64, Arc<Mutex<req::ChatBody>>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static SCREEN_SHOT: LazyLock<Mutex<screen_shot::ScreenshotManager>> =
    LazyLock::new(|| Mutex::new(screen_shot::ScreenshotManager::init().unwrap()));

#[kovi::plugin]
async fn main() {
    let bot = plugin::get_runtime_bot();
    let data_path = bot.get_data_path();
    let config_path = data_path.join("config.json");
    let config = load_json_data(config::Config::default(), config_path).unwrap();
    let config = Arc::new(config);

    let data_path = Arc::new(data_path);

    let chat = Arc::new(req::Chat::from_config((*config).clone()));

    plugin::on_group_msg(move |event| {
        let chat = chat.clone();
        let config = config.clone();
        let data_path = data_path.clone();
        async move {
            let text = event.borrow_text().unwrap_or_default().trim();

            if !text.starts_with("/chat") {
                return;
            }

            let group = event.group_id.unwrap();
            let msgs = {
                match MESSAGE.read().await.get(&group) {
                    Some(v) => v.clone(),
                    None => MESSAGE
                        .write()
                        .await
                        .entry(group)
                        .or_insert(Arc::new(Mutex::new(req::ChatBody::new(
                            config.model.clone(),
                        ))))
                        .clone(),
                }
            };

            let mut msgs = match msgs.try_lock() {
                Ok(v) => v,
                Err(_) => {
                    event.reply("请等待上次回答结束");
                    return;
                }
            };

            info!("chat: {}", text);

            let text = text[5..].trim();

            let md = match chat.chat(text.to_string(), &mut msgs).await {
                Ok(v) => v
                    .replace("\\[", "$$")
                    .replace("\\]", "$$")
                    .replace("\\(", "$")
                    .replace("\\)", "$"),
                Err(e) => {
                    error!("{}", e);
                    event.reply("未知错误");
                    return;
                }
            };

            debug!("receive form chat success");

            let img = match gen_img(&md, &data_path).await {
                Ok(v) => v,
                Err(e) => {
                    error!("{}", e);
                    event.reply("生成图片失败");
                    // send text only
                    let text_seg = Segment::new("text", json!({ "text": md }));
                    let seg = Segment::new("node", json!({ "content": [text_seg] }));
                    let msg = Message::from(vec![seg]);
                    event.reply(msg);
                    return;
                }
            };

            debug!("gen img success");

            let base64_img = STANDARD.encode(img);

            let text_seg = Segment::new("text", json!({ "text": md }));
            let img_seg = Segment::new(
                "image",
                json!({ "file": &format!("base64://{}", base64_img) }),
            );

            let seg = Segment::new("node", json!({ "content": [img_seg, text_seg] }));

            let msg = Message::from(vec![seg]);

            event.reply(msg);
        }
    });
}

async fn gen_img(md: &str, data_path: &PathBuf) -> Result<Vec<u8>> {
    // 因为截图同时依赖于 html 文件，所以需要提前锁上
    let mut screenshot_lock = SCREEN_SHOT.lock().await;

    let html = md_to_html(md).await;

    if !data_path.exists() {
        std::fs::create_dir_all(data_path).unwrap();
    }

    let file_path = data_path.join("output.html");

    std::fs::write(&file_path, html).unwrap();

    let png_data = match screenshot_lock.screenshot(&file_path).await {
        Ok(v) => v,
        Err(err) => {
            error!("{}", err);
            return Err(err);
        }
    };

    Ok(png_data)
}

async fn md_to_html(md: &str) -> String {
    let mut options = pulldown_cmark::Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_MATH);
    let parser = pulldown_cmark::Parser::new_ext(md, options);

    let mut html_output = String::new();
    html_output.push_str(&html::HTML_START_NEXT_IS_MD_CSS);

    html_output.push_str(html::GITHUB_MARKDOWN_LIGHT_NEXT_IS_HTML2);

    html_output.push_str(html::HTML_2_NEXT_IS_HIGHLIGHT_CSS);

    html_output.push_str(html::HIGH_LIGHT_LIGHT_CSS_NEXT_IS_HTML3);

    html_output.push_str(html::HTML_3_NEXT_IS_MD_BODY_AND_THEN_IS_HTML4);
    pulldown_cmark::html::push_html(&mut html_output, parser);
    html_output.push_str(html::HTML_4_NEXT_IS_HIGH_LIGHT_JS);
    html_output.push_str(html::HIGH_LIGHT_JS_NEXT_IS_HTML_END);
    html_output.push_str(html::HTML_END);
    html_output.push_str(&format!("<script>{}</script>", html::HTML_SCRIPT));
    html_output.push_str(END);

    html_output
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use kovi::tokio;

    #[tokio::test]
    async fn test_md_to_img() {
        let md = r#"# 你好呀!

```javascript
var s = "JavaScript syntax highlighting";
alert(s);
```

```python
s = "Python syntax highlighting"
print(s)
```

```
No language indicated, so no syntax highlighting.
But let's throw in a <b>tag</b>.
```

$$
\frac{1}{2}
$$

已知过点$A(-1, 0)$ 、 $B(1, 0)$两点的动抛物线的准线始终与圆$x^2 + y^2 = 9$相切，该抛物线焦点$P$的轨迹是某圆锥曲线$E$的一部分。<br>(1)求曲线$E$的标准方程；<br>(2)已知点$C(-3, 0)$ ， $D(2, 0)$ ，过点$D$的动直线与曲线$E$相交于$M$ 、 $N$ ，设$\triangle CMN$的外心为$Q$ ， $O$为坐标原点，问：直线$OQ$与直线$MN$的斜率之积是否为定值，如果为定值，求出该定值；如果不是定值，则说明理由。
"#;
        let img = super::gen_img(
            md,
            &PathBuf::from("/home/dynamic_pigeon/Public/workspace/rust-demo/bot/data"),
        )
        .await
        .unwrap();
        assert!(!img.is_empty());
        std::fs::write("data/output.png", img).unwrap();
    }
}
