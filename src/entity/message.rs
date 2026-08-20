use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, sqlx::FromRow)]
pub struct Message {
    pub id: Uuid,
    pub from: String,
    pub to: String,
    pub message: String,
    pub created_at: OffsetDateTime,
}
