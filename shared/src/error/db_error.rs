use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("{0}")]
    SomethingWentWrong(String),
}
