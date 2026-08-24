use async_trait::async_trait;
use log::{error, info};
use sqlx::{Error, PgPool};
use std::process::exit;

use crate::config::parameter;

pub struct Database {
    pool: PgPool,
}

#[async_trait]
pub trait DatabaseTrait {
    async fn init() -> Result<Self, Error>
    where
        Self: Sized;
    fn get_pool(&self) -> &PgPool;
}

#[async_trait]
impl DatabaseTrait for Database {
    async fn init() -> Result<Self, Error> {
        let db_url = parameter::get("DATABASE_URL").unwrap_or_else(|e| {
            error!("{}", e);
            exit(1);
        });
        info!("Connecting to Postgres");
        let pool: PgPool = PgPool::connect(&db_url).await?;
        Ok(Self { pool })
    }

    fn get_pool(&self) -> &PgPool {
        &self.pool
    }
}
