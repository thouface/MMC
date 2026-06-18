//! Error types for pairing module

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Not initialized")]
    NotInitialized,

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Pairing rejected: {0}")]
    Rejected(String),

    #[error("Timeout")]
    Timeout,

    #[error("Security error: {0}")]
    Security(String),

    #[error("Not found")]
    NotFound,
}

pub type Result<T> = std::result::Result<T, Error>;
