use crate::{dto::message_dto::MessageCreateDto, state::message_state::MessageState};
use axum::{Json, extract::State, http::StatusCode};
use validator::Validate;

pub async fn message_handler(
    State(state): State<MessageState>,
    Json(payload): Json<MessageCreateDto>,
) -> Result<StatusCode, String> {
    if let Err(e) = payload.validate() {
        return Err((StatusCode::BAD_REQUEST, e.to_string()));
    }

    match state.message_service {}
}
