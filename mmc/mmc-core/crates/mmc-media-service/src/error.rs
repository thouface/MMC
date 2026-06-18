//! Error types for media service

use thiserror::Error;

#[derive(Error, Debug)]
pub enum MediaError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Frame processing error: {0}")]
    FrameProcessing(String),

    #[error("Input dispatch error: {0}")]
    InputDispatch(String),

    #[error("Not initialized")]
    NotInitialized,

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, MediaError>;
