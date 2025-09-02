use crate::sql::POOL;

/// 不进行 commit/rollback 操作直接析构会调用 rollback
pub struct Commit {
    pub tx: Option<sqlx::Transaction<'static, sqlx::Sqlite>>,
}

impl Commit {
    pub async fn start() -> Result<Self, sqlx::Error> {
        let sql = POOL.get().unwrap();
        let tx = sql.begin().await?;
        Ok(Self { tx: Some(tx) })
    }

    pub async fn commit(&mut self) -> Result<(), sqlx::Error> {
        if let Some(tx) = self.tx.take() {
            tx.commit().await
        } else {
            Ok(())
        }
    }

    #[allow(dead_code)]
    pub async fn rollback(&mut self) -> Result<(), sqlx::Error> {
        if let Some(tx) = self.tx.take() {
            tx.rollback().await
        } else {
            Ok(())
        }
    }
}

impl Drop for Commit {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            // 如果 Commit 没有被 commit 或 rollback，析构时会自动回滚
            kovi::tokio::spawn(async move {
                if let Err(e) = tx.rollback().await {
                    eprintln!("Failed to rollback transaction: {}", e);
                }
            });
        }
    }
}
