use crate::{
    dto::message_dto::{MessageCreateDto, MessageReadDto},
    error::api_error::ApiError,
    state::message_state::MessageState,
};
use axum::{Json, extract::State, http::StatusCode};
use validator::Validate;

pub async fn message_handler(
    State(state): State<MessageState>,
    Json(payload): Json<MessageCreateDto>,
) -> Result<(StatusCode, Json<MessageReadDto>), ApiError> {
    if let Err(e) = payload.validate() {
        return Err(ApiError::ValidationError(e.to_string()));
    }

    let message = state.message_service.create_message(payload).await?;
    Ok((StatusCode::CREATED, Json(message)))
}
