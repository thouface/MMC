//! File transfer module
//! Handles chunked file transmission with checksums and resume support

pub mod error;
pub mod transfer;

pub use error::{Error, Result};
pub use transfer::{
    ChunkInfo, ChunkManifest, TransferProgress, TransferService, TransferState,
    TransferTask,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transfer_service_creation() {
        let _service = TransferService::new();
        // Placeholder test
        assert!(true);
    }

    #[test]
    fn test_transfer_progress() {
        let progress = TransferProgress::new("task-1".to_string(), 1000);
        assert_eq!(progress.percent(), 0.0);
    }
}
