//! Error types for core library

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Not initialized")]
    NotInitialized,

    #[error("Already initialized")]
    AlreadyInitialized,

    #[error("Initialization failed: {0}")]
    InitFailed(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Security error: {0}")]
    Security(String),

    #[error("Storage error: {0}")]
    StorageFailed(String),

    #[error("Transfer error: {0}")]
    TransferFailed(String),

    #[error("Discovery error: {0}")]
    DiscoveryFailed(String),

    #[error("Pairing error: {0}")]
    PairingFailed(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Timeout")]
    Timeout,

    #[error("Cancelled")]
    Cancelled,

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Io(e.to_string())
    }
}
