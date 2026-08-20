use crate::{
    config::database::{Database, DatabaseTrait},
    dto::message_dto::MessageCreateDto,
    entity::message::Message,
    error::db_error::DbError,
};
use async_trait::async_trait;
use sqlx::Error as SqlxError;
use std::sync::Arc;

#[derive(Clone)]
pub struct MessageRepository {
    pub(crate) db_conn: Arc<Database>,
}

#[async_trait]
pub trait MessageRepositoryTrait {
    fn new(db_conn: &Arc<Database>) -> Self;
    async fn create(&self, payload: MessageCreateDto) -> Result<Message, DbError>;
}

#[async_trait]
impl MessageRepositoryTrait for MessageRepository {
    fn new(db_conn: &Arc<Database>) -> Self {
        Self {
            db_conn: Arc::clone(db_conn),
        }
    }
    async fn create(&self, payload: MessageCreateDto) -> Result<Message, DbError> {
        let message = sqlx::query_as::<_, Message>(
            r#"
                   INSERT INTO messages (id, "from", "to", message)
                   VALUES ($1, $2, $3, $4)
                   RETURNING id, "from", "to", message, created_at
            "#,
        )
        .bind(uuid::Uuid::new_v4())
        .bind(payload.from_)
        .bind(payload.to)
        .bind(payload.message)
        .fetch_one(self.db_conn.get_pool())
        .await
        .map_err(|e| match e {
            SqlxError::Database(e) => match e.code().as_deref() {
                Some("23505") => DbError::UniqueConstraintViolation(e.to_string()),
                _ => DbError::SomethingWentWrong(e.to_string()),
            },
            _ => DbError::SomethingWentWrong(e.to_string()),
        })?;

        Ok(message)
    }
}
