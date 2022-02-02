use async_trait::async_trait;
use bb8_redis::bb8;
use bb8_redis::redis::cluster::ClusterConnection;
use bb8_redis::redis::{ErrorKind, IntoConnectionInfo, RedisError};
use redis::cluster::ClusterClient;

pub struct RedisClusterConnectionManager {
    client: ClusterClient,
}

impl RedisClusterConnectionManager {
    pub fn new<T: IntoConnectionInfo>(
        info: Vec<T>,
    ) -> Result<RedisClusterConnectionManager, RedisError> {
        let connection_info = info
            .into_iter()
            .map(|x| x.into_connection_info().unwrap())
            .collect::<_>();
        Ok(RedisClusterConnectionManager {
            client: ClusterClient::open(connection_info)?,
        })
    }
}

#[async_trait]
impl bb8::ManageConnection for RedisClusterConnectionManager {
    type Connection = ClusterConnection;
    type Error = RedisError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        self.client.get_connection()
    }

    async fn is_valid(
        &self,
        conn: &mut bb8::PooledConnection<'_, Self>,
    ) -> Result<(), Self::Error> {
        match conn.check_connection() {
            true => Ok(()),
            false => Err((ErrorKind::ResponseError, "connection fail").into()),
        }
    }

    fn has_broken(&self, _: &mut Self::Connection) -> bool {
        false
    }
}
