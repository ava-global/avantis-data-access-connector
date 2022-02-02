pub use inner::*;
#[cfg(test)]
use mocktopus::macros::mockable;

#[cfg_attr(test, mockable)]
pub mod inner {
    use std::future::Future;

    use anyhow::anyhow;
    use bb8_redis::bb8::{self, Pool, PooledConnection};
    use once_cell::sync::OnceCell;
    use serde::de::DeserializeOwned;
    use serde::Serialize;
    use serde_json::json;
    use tracing::error;

    use crate::redis_cluster::RedisClusterConnectionManager;
    use redis::Commands;

    static CONNECTION_POOL: OnceCell<Pool<RedisClusterConnectionManager>> = OnceCell::new();

    pub struct Setting {
        hosts: Vec<&'static str>,
        max_connections: u32
    }

    #[tracing::instrument(name = "redis::set_with_expire_seconds", skip_all)]
    pub async fn set_with_expire_seconds(
        key: &str,
        value: &str,
        expire_seconds: usize,
    ) -> anyhow::Result<()> {
        get_connection()
            .await?
            .set_ex(key, value, expire_seconds)
            .map_err(|err| err.into())
    }

    #[tracing::instrument(name = "redis::get")]
    pub async fn get(key: &str) -> anyhow::Result<Option<String>> {
        get_connection().await?.get(key).map_err(|err| err.into())
    }

    pub async fn initialize(setting: Setting) -> anyhow::Result<()> {
        CONNECTION_POOL
            .set(create_connection_pool(setting).await?)
            .map_err(|_error| return anyhow!("redis connection pool is already initialized"))
    }

    async fn get_connection(
    ) -> anyhow::Result<PooledConnection<'static, RedisClusterConnectionManager>> {
        CONNECTION_POOL
            .get()
            .expect("redis connection pool is not initialized")
            .get()
            .await
            .map_err(|error| error.into())
    }

    async fn create_connection_pool(setting: Setting) -> anyhow::Result<Pool<RedisClusterConnectionManager>> {

        let manager = RedisClusterConnectionManager::new(setting.hosts)?;
        Ok(bb8::Pool::builder()
            .max_size(setting.max_connections)
            .build(manager)
            .await?)
    }

    pub async fn get_or_set_with_expire<F, Fut, T>(
        cache_key: String,
        data_loader: F,
        expire_seconds: usize
    ) -> anyhow::Result<T>
    where
        T: Serialize + DeserializeOwned + Send + Sync,
        Fut: Future<Output = anyhow::Result<T>> + Send,
        F: FnOnce() -> Fut + Send,
    {
        match get(&cache_key).await {
            Ok(Some(json_str)) => {
                let result: T = serde_json::from_str(&json_str)?;
                Ok(result)
            }
            Ok(None) => {
                let result = data_loader().await?;
                set_with_expire_seconds(
                    &cache_key,
                    &json!(result).to_string(),
                    expire_seconds,
                )
                .await?;
                Ok(result)
            }
            Err(err) => {
                error!("Failed to connect to redis: {}", err);
                Ok(data_loader().await?)
            }
        }
    }
}
