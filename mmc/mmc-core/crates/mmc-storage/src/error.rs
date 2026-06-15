//! Error types for storage module

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to open database: {0}")]
    Open(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

pub type Result<T> = std::result::Result<T, Error>;
