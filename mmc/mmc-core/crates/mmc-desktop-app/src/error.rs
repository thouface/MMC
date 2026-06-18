//! Error types for desktop application

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DesktopError {
    #[error("Clipboard error: {0}")]
    Clipboard(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Transfer error: {0}")]
    Transfer(String),

    #[error("Mirror error: {0}")]
    Mirror(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Discovery error: {0}")]
    Discovery(String),

    #[error("Pairing error: {0}")]
    Pairing(String),

    #[error("Not initialized")]
    NotInitialized,

    #[error("Invalid command: {0}")]
    InvalidCommand(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<mmc_clipboard::ClipboardError> for DesktopError {
    fn from(e: mmc_clipboard::ClipboardError) -> Self {
        DesktopError::Clipboard(e.to_string())
    }
}

impl From<mmc_file_transfer::error::Error> for DesktopError {
    fn from(e: mmc_file_transfer::error::Error) -> Self {
        DesktopError::Transfer(e.to_string())
    }
}

impl From<mmc_media_service::error::MediaError> for DesktopError {
    fn from(e: mmc_media_service::error::MediaError) -> Self {
        DesktopError::Mirror(e.to_string())
    }
}

impl From<mmc_storage::error::Error> for DesktopError {
    fn from(e: mmc_storage::error::Error) -> Self {
        DesktopError::Storage(e.to_string())
    }
}

impl From<mmc_discovery::error::Error> for DesktopError {
    fn from(e: mmc_discovery::error::Error) -> Self {
        DesktopError::Discovery(e.to_string())
    }
}

impl From<mmc_pairing::error::Error> for DesktopError {
    fn from(e: mmc_pairing::error::Error) -> Self {
        DesktopError::Pairing(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, DesktopError>;