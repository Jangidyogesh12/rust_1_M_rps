use crate::error::db_error::DbError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error(transparent)]
    DbError(#[from] DbError),
    #[error("{0}")]
    ValidationError(String),
}
