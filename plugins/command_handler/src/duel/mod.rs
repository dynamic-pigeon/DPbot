use kovi::{
    PluginBuilder as plugin,
    log::{debug, error},
};

pub(crate) mod challenge;
pub(crate) mod config;
pub(crate) mod handlers;
pub(crate) mod problem;
pub(crate) mod user;

pub async fn init() {
    kovi::spawn(async move {
        // 初始化题库
        match problem::get_problems().await {
            Ok(_) => debug!("初始化题库成功"),
            Err(e) => {
                error!("初始化题库失败: {}", e);
            }
        };
    });

    plugin::cron("0 0 * * *", || async {
        // 每天 0 点刷新题库
        match problem::get_problems().await {
            Ok(_) => debug!("初始化题库成功"),
            Err(e) => {
                error!("初始化题库失败: {}", e);
            }
        };
    })
    .unwrap();
}
