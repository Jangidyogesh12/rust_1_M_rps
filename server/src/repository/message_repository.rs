use crate::dto::message_dto::MessageCreateDto;
use async_trait::async_trait;
use redis::AsyncTypedCommands;
use serde_json;
use shared::{
    config::redis::{Redis, RedisTrait},
    error::db_error::DbError,
    message::StreamMessage as Message,
};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone)]
pub struct MessageRepository {
    pub(crate) db_conn: Redis,
}

#[async_trait]
pub trait MessageRepositoryTrait {
    fn new(db_conn: Redis) -> Self;
    async fn create(&self, payload: MessageCreateDto) -> Result<Message, DbError>;
}

#[async_trait]
impl MessageRepositoryTrait for MessageRepository {
    fn new(db_conn: Redis) -> Self {
        Self { db_conn }
    }

    async fn create(&self, payload: MessageCreateDto) -> Result<Message, DbError> {
        let message = Message {
            id: Uuid::new_v4(),
            from: payload.from_,
            to: payload.to,
            message: payload.message,
            created_at: OffsetDateTime::now_utc(),
        };

        let data = serde_json::to_string(&message)
            .map_err(|e| DbError::SomethingWentWrong(e.to_string()))?;

        let mut conn = self.db_conn.get_connection();

        let _ = conn
            .xgroup_create_mkstream(Message::STREAM, Message::GROUP, "$")
            .await;

        // typed xadd returns the stream entry id; None only with NOMKSTREAM
        let _: Option<String> = conn
            .xadd(Message::STREAM, "*", &[("data", data.as_str())])
            .await
            .map_err(|e| DbError::SomethingWentWrong(e.to_string()))?;

        Ok(message)
    }
}
