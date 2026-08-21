use std::sync::Arc;

use axum::Router;

use crate::{
    config::database::Database,
    routes::{hello, message},
    state::message_state::MessageState,
};

pub fn routes(db_conn: Arc<Database>) -> Router {
    let merged_router = {
        let message_state = MessageState::new(&db_conn);

        message::route()
            .with_state(message_state)
            .merge(hello::route())
    };

    merged_router
}
