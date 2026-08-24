use axum::{Router, routing::post};

use crate::{
    handler::message_handler::{message_direct_handler, message_fast_handler},
    state::message_state::MessageState,
};

pub fn route() -> Router<MessageState> {
    Router::new()
        .route("/message-fast", post(message_fast_handler))
        .route("/message", post(message_direct_handler))
}
