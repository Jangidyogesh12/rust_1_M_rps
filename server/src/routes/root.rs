use axum::Router;
use sqlx::PgPool;

use crate::{
    routes::{hello, message},
    state::message_state::MessageState,
};
use shared::config::redis::Redis;

pub fn routes(db_conn: Redis, pg_pool: PgPool) -> Router {
    let merged_router = {
        let message_state = MessageState::new(db_conn, pg_pool);

        message::route()
            .with_state(message_state)
            .merge(hello::route())
    };

    merged_router
}
