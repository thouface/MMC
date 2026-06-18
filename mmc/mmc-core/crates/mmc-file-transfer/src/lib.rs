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
        let service = TransferService::new();
        let tasks = service.get_tasks().await;
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_transfer_progress_new() {
        let progress = TransferProgress::new("task-1".to_string(), 1000);
        assert_eq!(progress.percent(), 0.0);
        assert_eq!(progress.state, TransferState::Idle);
        assert_eq!(progress.task_id, "task-1");
        assert_eq!(progress.total_bytes, 1000);
        assert_eq!(progress.bytes_transferred, 0);
    }

    #[test]
    fn test_transfer_state_variants() {
        assert_eq!(TransferState::Idle as u8, 0);
        assert!(matches!(TransferState::Idle, TransferState::Idle));
        assert!(matches!(TransferState::Preparing, TransferState::Preparing));
        assert!(matches!(TransferState::Transferring, TransferState::Transferring));
        assert!(matches!(TransferState::Paused, TransferState::Paused));
        assert!(matches!(TransferState::Completed, TransferState::Completed));
        assert!(matches!(TransferState::Failed, TransferState::Failed));
        assert!(matches!(TransferState::Canceled, TransferState::Canceled));
    }

    #[test]
    fn test_transfer_state_default() {
        let state: TransferState = TransferState::default();
        assert_eq!(state, TransferState::Idle);
    }

    #[test]
    fn test_transfer_progress_update() {
        let mut progress = TransferProgress::new("t1".to_string(), 1000);
        progress.update(500, 1000);
        assert_eq!(progress.percent(), 50.0);
        assert_eq!(progress.bytes_transferred, 500);
        assert_eq!(progress.state, TransferState::Transferring);
        assert!(progress.speed_bps > 0);
        assert!(progress.remaining_ms > 0);
    }

    #[test]
    fn test_transfer_progress_complete() {
        let mut progress = TransferProgress::new("t1".to_string(), 500);
        progress.update(250, 250);
        assert_eq!(progress.percent(), 50.0);
        progress.complete();
        assert_eq!(progress.bytes_transferred, 500);
        assert_eq!(progress.state, TransferState::Completed);
        assert_eq!(progress.percent(), 100.0);
        assert_eq!(progress.speed_bps, 0);
        assert_eq!(progress.remaining_ms, 0);
    }

    #[test]
    fn test_transfer_progress_fail() {
        let mut progress = TransferProgress::new("t2".to_string(), 500);
        progress.update(250, 100);
        progress.fail();
        assert_eq!(progress.state, TransferState::Failed);
    }

    #[test]
    fn test_transfer_progress_zero_total() {
        let progress = TransferProgress::new("t".to_string(), 0);
        assert_eq!(progress.percent(), 0.0);
    }

    #[test]
    fn test_chunk_info_creation() {
        let info = ChunkInfo {
            index: 5,
            hash: [0u8; 32],
            size: 100,
        };
        assert_eq!(info.index, 5);
        assert_eq!(info.size, 100);
    }

    #[test]
    fn test_chunk_manifest_creation() {
        let manifest = ChunkManifest {
            file_id: "f-1".to_string(),
            file_name: "test.txt".to_string(),
            total_size: 500,
            chunk_size: 100,
            total_chunks: 5,
            chunks: vec![],
        };

        assert_eq!(manifest.file_id, "f-1");
        assert_eq!(manifest.file_name, "test.txt");
        assert_eq!(manifest.total_size, 500);
        assert_eq!(manifest.total_chunks, 5);
        assert_eq!(manifest.chunk_size, 100);
        assert!(manifest.chunks.is_empty());
    }
}
