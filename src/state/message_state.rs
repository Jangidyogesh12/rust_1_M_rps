use std::sync::Arc;

use crate::{config::database::Database, service::message_service::MessageService};

#[derive(Clone)]

pub struct MessageState {
    pub message_service: MessageService,
}

impl MessageState {
    pub fn new(db_conn: &Arc<Database>) -> Self {
        Self {
            message_service: MessageService::new(db_conn),
        }
    }
}
