use axum::{routing::post, Router};

use crate::{handler::message_handler::message_handler, state::message_state::MessageState};

pub fn route() -> Router<MessageState> {
    Router::new().route("/message", post(message_handler))
}
