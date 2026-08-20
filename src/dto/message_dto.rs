use crate::entity::message::Message;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Deserialize, Validate)]
pub struct MessageCreateDto {
    #[serde(rename = "from")]
    pub from_: String,
    pub to: String,
    #[validate(length(min = 1, max = 1000, message = "Message must be 1-1000 chars"))]
    pub message: String,
}

#[derive(Clone, Serialize)]
pub struct MessageReadDto {
    pub id: Uuid,
    pub from: String,
    pub to: String,
    pub message: String,
    pub created_at: String,
}

impl From<Message> for MessageReadDto {
    fn from(m: Message) -> Self {
        Self {
            id: m.id,
            from: m.from,
            to: m.to,
            message: m.message,
            created_at: m.created_at.format(&Rfc3339).unwrap_or_default(),
        }
    }
}
