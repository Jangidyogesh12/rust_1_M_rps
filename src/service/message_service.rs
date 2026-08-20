use crate::config::database::Database;
use crate::dto::message_dto::{MessageCreateDto, MessageReadDto};
use crate::error::api_error::ApiError;
use crate::repository::message_repository::{MessageRepository, MessageRepositoryTrait};
use std::sync::Arc;

#[derive(Clone)]
pub struct MessageService {
    message_repo: MessageRepository,
}

impl MessageService {
    pub fn new(db_conn: &Arc<Database>) -> Self {
        Self {
            message_repo: MessageRepository::new(db_conn),
        }
    }

    pub async fn create_message(
        &self,
        payload: MessageCreateDto,
    ) -> Result<MessageReadDto, ApiError> {
        let message = self.message_repo.create(payload).await?;
        Ok(MessageReadDto::from(message))
    }
}
