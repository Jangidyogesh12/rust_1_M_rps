use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use shared::error::api_error::ApiError;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ApiErrorResponse {
    message: Option<String>,
    #[serde(rename = "code")]
    status: u16,
}

impl ApiErrorResponse {
    fn new(status: u16, message: Option<String>) -> Self {
        Self { message, status }
    }
}

/// Bridge: shared defines the error data, server owns the HTTP mapping.
/// Enables `?` in handlers returning Result<_, ApiErrorResponse>.
impl From<ApiError> for ApiErrorResponse {
    fn from(error: ApiError) -> Self {
        match error {
            ApiError::DbError(e) => {
                Self::new(StatusCode::INTERNAL_SERVER_ERROR.as_u16(), Some(e.to_string()))
            }
            ApiError::ValidationError(message) => {
                Self::new(StatusCode::BAD_REQUEST.as_u16(), Some(message))
            }
        }
    }
}

impl IntoResponse for ApiErrorResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(self),
        )
            .into_response()
    }
}
