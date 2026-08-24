use crate::service::message_service::MessageService;
use shared::config::redis::Redis;

#[derive(Clone)]

pub struct MessageState {
    pub message_service: MessageService,
}

impl MessageState {
    pub fn new(db_conn: Redis) -> Self {
        Self {
            message_service: MessageService::new(db_conn),
        }
    }
}
