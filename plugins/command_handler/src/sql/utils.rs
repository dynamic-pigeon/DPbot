use crate::sql::POOL;

pub async fn with_commit<F, T, E>(f: F) -> Result<T, E>
where
    F: for<'c> AsyncFnOnce(&'c mut sqlx::Transaction<'_, sqlx::Sqlite>) -> Result<T, E>,
{
    let sql = POOL.get().unwrap();
    let mut tx = sql.begin().await.unwrap();

    match f(&mut tx).await {
        Ok(ret) => {
            tx.commit().await.unwrap();
            Ok(ret)
        }
        Err(e) => {
            tx.rollback().await.unwrap();
            Err(e)
        }
    }
}

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
            // Rollback if not committed
            let _ = tx.rollback();
        }
    }
}
