//! API 速率限制器模块
//!
//! 提供一个全局的、可跨 crate 使用的 API 访问管理器。
//!
//! # 示例
//! ```rust
//! use utils::api_limit::limit_api_call;
//! use std::time::Duration;
//!
//! // 手动限制
//! let result = limit_api_call("MY_API", Duration::from_secs(1), 5, async {
//!     // API 调用
//!     "data"
//! }).await;
//! ```

use kovi::tokio::{
    self,
    sync::{Mutex, OnceCell, Semaphore},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// 单个 API 的速率限制器。
/// 这个结构体是私有的，外部通过 `limit_api_call` 函数间接使用。
#[derive(Debug)]
struct ApiLimiter {
    sem: Arc<Semaphore>,
    gap: Duration,
}

impl ApiLimiter {
    /// 创建一个新的 API 限制器。
    fn new(gap: Duration, times: usize) -> Self {
        Self {
            sem: Arc::new(Semaphore::new(times)),
            gap,
        }
    }

    /// 获取一个许可并执行异步操作。
    /// 在操作完成后，许可不会立即释放，而是会等待 `gap` 时间后再释放，
    /// 以此来控制速率。这个过程在后台任务中进行，不会阻塞当前任务。
    async fn acquire<F, R>(&self, f: F) -> R
    where
        F: std::future::Future<Output = R>,
    {
        let permit = self.sem.clone().acquire_owned().await.unwrap();

        let gap = self.gap;
        kovi::spawn(async move {
            tokio::time::sleep(gap).await;
            drop(permit);
        });

        f.await
    }
}

type LimiterRegistry = Mutex<HashMap<&'static str, Arc<ApiLimiter>>>;

async fn global_registry() -> &'static LimiterRegistry {
    static INSTANCE: OnceCell<LimiterRegistry> = OnceCell::const_new();
    INSTANCE
        .get_or_init(|| async { Mutex::new(HashMap::new()) })
        .await
}

/// 对一个 API 调用进行速率限制。
///
/// 这是提供给外部使用的主要函数。它会根据 `api_identifier` 查找或创建一个
/// 全局唯一的速率限制器，并用它来约束传入的异步操作 `f`。
///
/// # 参数
/// * `api_identifier`: API 的静态字符串标识符，例如 `"GET /api/v1/users"`。
/// * `gap`: 两次调用之间的最小时间间隔。
/// * `times`: 在一个 `gap` 时间窗口内允许的最大调用次数。
/// * `f`: 需要被限制速率的异步操作。
///
/// # 示例
/// ```
/// use std::time::Duration;
///
/// async fn my_api_call() -> &'static str {
///     // 模拟网络请求
///     tokio::time::sleep(Duration::from_millis(50)).await;
///     "Data received"
/// }
///
/// // 限制 "MY_API" 每秒最多调用 5 次
/// let result = limit_api_call(
///     "MY_API",
///     Duration::from_secs(1),
///     5,
///     my_api_call()
/// ).await;
///
/// assert_eq!(result, "Data received");
/// ```
pub async fn limit_api_call<F, R>(
    api_identifier: &'static str,
    gap: Duration,
    times: usize,
    f: F,
) -> R
where
    F: std::future::Future<Output = R>,
{
    let mut registry = global_registry().await.lock().await;

    // 使用 entry API 来原子性地获取或插入限制器
    let limiter = registry
        .entry(api_identifier)
        .or_insert_with(|| Arc::new(ApiLimiter::new(gap, times)))
        .clone();

    // 释放 registry 的锁，允许其他任务访问
    drop(registry);

    // 使用获取到的限制器执行操作
    limiter.acquire(f).await
}
