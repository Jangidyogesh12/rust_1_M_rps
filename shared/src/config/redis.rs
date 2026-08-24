use std::{io::Error, process::exit};

use async_trait::async_trait;
use log::error;
use redis::{Client, aio::ConnectionManager};

use crate::config::parameter;

#[derive(Clone)]
pub struct Redis {
    db_conn: ConnectionManager,
}

#[async_trait]
pub trait RedisTrait {
    async fn init() -> Result<Self, Error>
    where
        Self: Sized;

    fn get_connection(&self) -> ConnectionManager;
}

#[async_trait]
impl RedisTrait for Redis {
    async fn init() -> Result<Self, Error> {
        let host_url = parameter::get("REDIS_URL").unwrap_or_else(|e| {
            error!("{}", e);
            exit(1)
        });

        let client = match Client::open(host_url) {
            Ok(client) => client,
            Err(e) => {
                error!("{}", e);
                exit(1)
            }
        };

        match client.get_connection_manager().await {
            Ok(conn) => Ok(Self { db_conn: conn }),
            Err(e) => {
                error!("{}", e);
                exit(1)
            }
        }
    }

    /// Returns an owned clone — cheap (atomic refcount bump over one
    /// multiplexed connection), so no Arc wrapper is needed anywhere.
    fn get_connection(&self) -> ConnectionManager {
        self.db_conn.clone()
    }
}
