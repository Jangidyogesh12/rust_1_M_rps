use crate::error::db_error::DbError;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error(transparent)]
    DbError(#[from] DbError),
    #[error("{0}")]
    ValidationError(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::DbError(error) => error.into_response(),
            ApiError::ValidationError(message) => {
                crate::response::api_response::ApiErrorResponse::send(
                    StatusCode::BAD_REQUEST.as_u16(),
                    Some(message),
                )
            }
        }
    }
}
