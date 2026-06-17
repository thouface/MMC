//! Error types for the clipboard module.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Transport error: {0}")]
    Transport(String),
    
    #[error("Not connected")]
    NotConnected,
    
    #[error("Content too large: {0} bytes (max: {1})")]
    ContentTooLarge(usize, usize),
    
    #[error("Invalid content: {0}")]
    InvalidContent(String),
    
    #[error("No content to sync")]
    NoContent,
    
    #[error("Clipboard is empty")]
    Empty,
    
    #[error("Sync error: {0}")]
    SyncFailed(String),
}

pub type Result<T> = std::result::Result<T, ClipboardError>;