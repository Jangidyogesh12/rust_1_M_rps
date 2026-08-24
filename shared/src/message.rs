use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// Wire format for one message row.
/// Serialized to JSON by the server (XADD) and deserialized by sync (XREADGROUP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    pub id: Uuid,
    pub from: String,
    pub to: String,
    pub message: String,
    pub created_at: OffsetDateTime,
}

impl StreamMessage {
    pub const STREAM: &'static str = "messages:stream";
    pub const GROUP: &'static str = "workers";
    pub const DEAD: &'static str = "messages:dead";
}
