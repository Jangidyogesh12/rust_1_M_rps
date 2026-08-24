use crate::service::message_service::MessageService;
use shared::config::redis::Redis;
use sqlx::PgPool;

#[derive(Clone)]

pub struct MessageState {
    pub message_service: MessageService,
}

impl MessageState {
    pub fn new(db_conn: Redis, pg_pool: PgPool) -> Self {
        Self {
            message_service: MessageService::new(db_conn, pg_pool),
        }
    }
}
