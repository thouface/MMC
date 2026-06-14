//! Error types for discovery module

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("mDNS error: {0}")]
    Mdns(String),

    #[error("Service not started")]
    NotStarted,

    #[error("Invalid service info: {0}")]
    InvalidService(String),

    #[error("Service registration failed: {0}")]
    RegistrationFailed(String),
}

pub type Result<T> = std::result::Result<T, Error>;
