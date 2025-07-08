use std::future::Future;

pub async fn retry<T, E, F>(mut f: F, max_retry_times: usize) -> Result<T, E>
where
    F: AsyncFnMut() -> Result<T, E>,
{
    assert!(
        max_retry_times > 0,
        "max_retry_times must be greater than 0"
    );
    let mut err = None;
    for _ in 0..max_retry_times {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                err = Some(e);
            }
        }
    }
    Err(err.unwrap_or_else(|| {
        panic!("All retries failed, but no error was captured. This should not happen.");
    }))
}
