use crate::dto::message_dto::{MessageCreateDto, MessageReadDto};
use crate::repository::message_repository::{MessageRepository, MessageRepositoryTrait};
use shared::config::redis::Redis;
use shared::error::api_error::ApiError;
use shared::message::StreamMessage as Message;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone)]
pub struct MessageService {
    message_repo: MessageRepository,
    pg_pool: PgPool,
}

impl MessageService {
    pub fn new(db_conn: Redis, pg_pool: PgPool) -> Self {
        Self {
            message_repo: MessageRepository::new(db_conn),
            pg_pool,
        }
    }

    pub async fn create_message(
        &self,
        payload: MessageCreateDto,
    ) -> Result<MessageReadDto, ApiError> {
        let message = self.message_repo.create(payload).await?;
        Ok(MessageReadDto::from(message))
    }

    pub async fn create_message_direct(
        &self,
        payload: MessageCreateDto,
    ) -> Result<MessageReadDto, ApiError> {
        let message = Message {
            id: Uuid::new_v4(),
            from: payload.from_,
            to: payload.to,
            message: payload.message,
            created_at: OffsetDateTime::now_utc(),
        };

        sqlx::query(
            r#"INSERT INTO messages (id, "from", "to", message, created_at)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(message.id)
        .bind(&message.from)
        .bind(&message.to)
        .bind(&message.message)
        .bind(message.created_at)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::DbError(shared::error::db_error::DbError::SomethingWentWrong(e.to_string())))?;

        Ok(MessageReadDto::from(message))
    }
}
