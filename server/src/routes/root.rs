use axum::Router;

use crate::{
    routes::{hello, message},
    state::message_state::MessageState,
};
use shared::config::redis::Redis;

pub fn routes(db_conn: Redis) -> Router {
    let merged_router = {
        let message_state = MessageState::new(db_conn);

        message::route()
            .with_state(message_state)
            .merge(hello::route())
    };

    merged_router
}
