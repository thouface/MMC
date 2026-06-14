//! Error types for file transfer module

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("File rejected: {0}")]
    Rejected(String),

    #[error("Checksum mismatch")]
    ChecksumMismatch,

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),
}

pub type Result<T> = std::result::Result<T, Error>;
