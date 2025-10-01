use kovi::{
    PluginBuilder as plugin,
    log::{error, info},
};
use utils::retry::retry;

pub(crate) mod challenge;
pub(crate) mod config;
pub(crate) mod handlers;
pub(crate) mod problem;
pub(crate) mod submission;
pub(crate) mod user;

pub async fn init() {
    kovi::spawn(async move {
        // 初始化题库
        match retry(problem::refresh_problems, 3).await {
            Ok(_) => info!("初始化题库成功"),
            Err(e) => {
                error!("初始化题库失败: {}", e);
            }
        };
    });

    plugin::cron("0 0 * * *", || async {
        // 每天 0 点刷新题库
        match retry(problem::refresh_problems, 3).await {
            Ok(_) => info!("初始化题库成功"),
            Err(e) => {
                error!("初始化题库失败: {}", e);
            }
        };
    })
    .unwrap();
}
