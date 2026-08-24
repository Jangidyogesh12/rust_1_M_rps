use crate::response::api_response::ApiErrorResponse;
use crate::{
    dto::message_dto::{MessageCreateDto, MessageReadDto},
    state::message_state::MessageState,
};
use axum::{Json, extract::State, http::StatusCode};
use shared::error::api_error::ApiError;
use validator::Validate;

pub async fn message_fast_handler(
    State(state): State<MessageState>,
    Json(payload): Json<MessageCreateDto>,
) -> Result<(StatusCode, Json<MessageReadDto>), ApiErrorResponse> {
    if let Err(e) = payload.validate() {
        return Err(ApiError::ValidationError(e.to_string()).into());
    }

    let message = state.message_service.create_message(payload).await?;
    Ok((StatusCode::CREATED, Json(message)))
}
