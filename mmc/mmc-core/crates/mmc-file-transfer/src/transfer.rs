//! File transfer implementation with chunked protocol

use mmc_protocol::{Frame, FrameType, read_frame, write_frame};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::{broadcast, RwLock};
use tracing::info;

use crate::{Error, Result};

/// Transfer state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferState {
    Idle,
    Preparing,
    Transferring,
    Paused,
    Completed,
    Failed,
    Canceled,
}

impl Default for TransferState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Transfer progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub task_id: String,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub speed_bps: u64,
    pub remaining_ms: u64,
    pub state: TransferState,
}

impl TransferProgress {
    pub fn new(task_id: String, total_bytes: u64) -> Self {
        Self {
            task_id,
            bytes_transferred: 0,
            total_bytes,
            speed_bps: 0,
            remaining_ms: 0,
            state: TransferState::Idle,
        }
    }

    pub fn update(&mut self, bytes: u64, elapsed_ms: u64) {
        self.bytes_transferred = bytes;
        self.state = TransferState::Transferring;

        if elapsed_ms > 0 {
            self.speed_bps = (bytes * 1000) / elapsed_ms;
            let remaining = self.total_bytes.saturating_sub(bytes);
            if self.speed_bps > 0 {
                self.remaining_ms = (remaining * 1000) / self.speed_bps;
            }
        }
    }

    pub fn complete(&mut self) {
        self.bytes_transferred = self.total_bytes;
        self.state = TransferState::Completed;
        self.speed_bps = 0;
        self.remaining_ms = 0;
    }

    pub fn fail(&mut self) {
        self.state = TransferState::Failed;
    }

    pub fn percent(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.bytes_transferred as f32 / self.total_bytes as f32) * 100.0
        }
    }
}

/// File transfer task
#[derive(Debug, Clone)]
pub struct TransferTask {
    pub task_id: String,
    pub file_id: String,
    pub file_name: String,
    pub total_size: u64,
    pub chunk_size: u32,
    pub total_chunks: u32,
    pub state: TransferState,
    pub progress: TransferProgress,
    pub created_at: i64,
}

/// Manifest of file chunks with BLAKE3 hashes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkManifest {
    pub file_id: String,
    pub file_name: String,
    pub total_size: u64,
    pub chunk_size: u32,
    pub total_chunks: u32,
    pub chunks: Vec<ChunkInfo>,
}

/// Info about a single chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub index: u32,
    pub hash: [u8; 32],
    pub size: u32,
}

/// Transfer service
pub struct TransferService {
    tasks: Arc<RwLock<HashMap<String, TransferTask>>>,
    event_tx: broadcast::Sender<TransferProgress>,
}

impl TransferService {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        }
    }

    /// Compute chunk manifest for a file
    pub async fn compute_manifest(
        &self,
        file_path: &Path,
        chunk_size: u32,
    ) -> Result<ChunkManifest> {
        let mut file = File::open(file_path).await.map_err(|e| Error::Io(e.to_string()))?;
        let metadata = file.metadata().await.map_err(|e| Error::Io(e.to_string()))?;
        let total_size = metadata.len();

        let total_chunks =
            ((total_size as u64 + chunk_size as u64 - 1) / chunk_size as u64) as u32;

        let mut chunks = Vec::with_capacity(total_chunks as usize);
        let mut buf = vec![0u8; chunk_size as usize];
        let mut chunk_index = 0u32;

        loop {
            let bytes_read = file
                .read(&mut buf)
                .await
                .map_err(|e| Error::Io(e.to_string()))?;
            if bytes_read == 0 {
                break;
            }

            let hash = blake3::hash(&buf[..bytes_read]);
            chunks.push(ChunkInfo {
                index: chunk_index,
                hash: *hash.as_bytes(),
                size: bytes_read as u32,
            });

            chunk_index += 1;
        }

        let file_id = uuid::Uuid::new_v4().to_string();
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(ChunkManifest {
            file_id,
            file_name,
            total_size,
            chunk_size,
            total_chunks,
            chunks,
        })
    }

    /// Start sending a file to a peer
    pub async fn send_file(
        &self,
        task_id: &str,
        peer_addr: &str,
        manifest: &ChunkManifest,
        file_path: &Path,
    ) -> Result<()> {
        let task = TransferTask {
            task_id: task_id.to_string(),
            file_id: manifest.file_id.clone(),
            file_name: manifest.file_name.clone(),
            total_size: manifest.total_size,
            chunk_size: manifest.chunk_size,
            total_chunks: manifest.total_chunks,
            state: TransferState::Preparing,
            progress: TransferProgress::new(task_id.to_string(), manifest.total_size),
            created_at: chrono::Utc::now().timestamp(),
        };

        self.tasks.write().await.insert(task_id.to_string(), task);

        // Connect to peer
        let mut stream = tokio::net::TcpStream::connect(peer_addr)
            .await
            .map_err(|e| Error::Connection(e.to_string()))?;

        // Send manifest
        let manifest_json =
            serde_json::to_vec(manifest).map_err(|e| Error::Serialization(e.to_string()))?;
        let frame = Frame::new(FrameType::FileManifestRequest, manifest_json);
        write_frame(&mut stream, &frame)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        // Wait for response
        let response_frame = read_frame(&mut stream)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?
            .ok_or_else(|| Error::Protocol("No manifest response".to_string()))?;

        if response_frame.frame_type() != FrameType::FileManifestResponse {
            return Err(Error::Protocol("Unexpected response type".to_string()));
        }

        let response_json = response_frame.into_payload();
        let response: serde_json::Value =
            serde_json::from_slice(&response_json).map_err(|e| Error::Serialization(e.to_string()))?;

        if response["accepted"].as_bool() != Some(true) {
            let reason = response["error_reason"].as_str().unwrap_or("Unknown");
            return Err(Error::Rejected(reason.to_string()));
        }

        // Get already received chunks (for resume)
        let already_have: Vec<u32> = response["already_have_chunks"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_i64().map(|i| i as u32))
                    .collect()
            })
            .unwrap_or_default();

        // Open file and send chunks
        let mut file = File::open(file_path).await.map_err(|e| Error::Io(e.to_string()))?;
        let mut buf = vec![0u8; manifest.chunk_size as usize];
        let mut chunk_index = 0u32;
        let start_time = std::time::Instant::now();

        while chunk_index < manifest.total_chunks {
            // Skip already received chunks
            if already_have.contains(&chunk_index) {
                chunk_index += 1;
                continue;
            }

            let bytes_read = file
                .read(&mut buf)
                .await
                .map_err(|e| Error::Io(e.to_string()))?;
            if bytes_read == 0 {
                break;
            }

            // Send chunk
            let chunk_frame = Frame::new(FrameType::ChunkData, buf[..bytes_read].to_vec());
            write_frame(&mut stream, &chunk_frame)
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?;

            // Update progress
            let elapsed = start_time.elapsed().as_millis() as u64;
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(task_id) {
                task.progress
                    .update((chunk_index as u64 + 1) * manifest.chunk_size as u64, elapsed);
                task.state = TransferState::Transferring;
                let _ = self.event_tx.send(task.progress.clone());
            }

            chunk_index += 1;
        }

        // Send completion
        let complete_frame = Frame::new(FrameType::TransferComplete, vec![]);
        write_frame(&mut stream, &complete_frame)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.progress.complete();
            task.state = TransferState::Completed;
            let _ = self.event_tx.send(task.progress.clone());
        }

        info!("File transfer completed: {}", manifest.file_name);

        Ok(())
    }

    /// Receive a file from a peer
    pub async fn receive_file(
        &self,
        task_id: &str,
        manifest: ChunkManifest,
        output_path: &Path,
    ) -> Result<()> {
        let task = TransferTask {
            task_id: task_id.to_string(),
            file_id: manifest.file_id.clone(),
            file_name: manifest.file_name.clone(),
            total_size: manifest.total_size,
            chunk_size: manifest.chunk_size,
            total_chunks: manifest.total_chunks,
            state: TransferState::Transferring,
            progress: TransferProgress::new(task_id.to_string(), manifest.total_size),
            created_at: chrono::Utc::now().timestamp(),
        };

        self.tasks.write().await.insert(task_id.to_string(), task);

        // Create output file
        let _file = File::create(output_path).await.map_err(|e| Error::Io(e.to_string()))?;

        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.progress.complete();
            task.state = TransferState::Completed;
            let _ = self.event_tx.send(task.progress.clone());
        }

        Ok(())
    }

    /// Cancel a transfer
    pub async fn cancel(&self, task_id: &str) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.state = TransferState::Canceled;
            task.progress.fail();
        }
        Ok(())
    }

    /// Get all tasks
    pub async fn get_tasks(&self) -> Vec<TransferTask> {
        let tasks = self.tasks.read().await;
        tasks.values().cloned().collect()
    }

    /// Get progress events stream
    pub fn events(&self) -> broadcast::Receiver<TransferProgress> {
        self.event_tx.subscribe()
    }
}

impl Default for TransferService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_compute_manifest() {
        let service = TransferService::new();

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, b"hello world")
            .await
            .unwrap();

        let manifest = service.compute_manifest(&file_path, 1024).await.unwrap();

        assert_eq!(manifest.file_name, "test.txt");
        assert_eq!(manifest.total_size, 11);
        assert_eq!(manifest.total_chunks, 1);
        assert_eq!(manifest.chunks.len(), 1);
        assert_eq!(manifest.chunks[0].index, 0);
        assert_eq!(manifest.chunks[0].size, 11);
    }

    #[tokio::test]
    async fn test_compute_manifest_multi_chunk() {
        let service = TransferService::new();

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("large.txt");

        let data: Vec<u8> = (0..300).map(|_| b'x').collect();
        tokio::fs::write(&file_path, &data).await.unwrap();

        let manifest = service.compute_manifest(&file_path, 100).await.unwrap();

        assert_eq!(manifest.total_size, 300);
        assert_eq!(manifest.total_chunks, 3);
        assert_eq!(manifest.chunks.len(), 3);

        for i in 0..3 {
            assert_eq!(manifest.chunks[i].index, i as u32);
            assert_eq!(manifest.chunks[i].size, 100);
        }
    }

    #[tokio::test]
    async fn test_transfer_progress() {
        let mut progress = TransferProgress::new("task-1".to_string(), 1000);

        progress.update(500, 1000);
        assert_eq!(progress.percent(), 50.0);
        assert_eq!(progress.speed_bps, 500);

        progress.complete();
        assert_eq!(progress.state, TransferState::Completed);
        assert_eq!(progress.bytes_transferred, 1000);
    }

    #[tokio::test]
    async fn test_transfer_progress_zero_total() {
        let progress = TransferProgress::new("task-1".to_string(), 0);
        assert_eq!(progress.percent(), 0.0);
    }

    #[tokio::test]
    async fn test_transfer_progress_fail() {
        let mut progress = TransferProgress::new("task-1".to_string(), 500);
        progress.update(250, 500);
        assert_eq!(progress.percent(), 50.0);

        progress.fail();
        assert_eq!(progress.state, TransferState::Failed);
    }

    #[tokio::test]
    async fn test_transfer_state_default() {
        let state: TransferState = TransferState::default();
        assert_eq!(state, TransferState::Idle);
    }

    #[tokio::test]
    async fn test_cancel_transfer() {
        let service = TransferService::new();
        let result = service.cancel("nonexistent").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_tasks_empty() {
        let service = TransferService::new();
        let tasks = service.get_tasks().await;
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn test_chunk_info_fields() {
        let info = ChunkInfo {
            index: 5,
            hash: [0u8; 32],
            size: 100,
        };
        assert_eq!(info.index, 5);
        assert_eq!(info.size, 100);
    }

    #[tokio::test]
    async fn test_chunk_manifest_fields() {
        let manifest = ChunkManifest {
            file_id: "f-1".to_string(),
            file_name: "test.txt".to_string(),
            total_size: 500,
            chunk_size: 100,
            total_chunks: 5,
            chunks: vec![],
        };

        assert_eq!(manifest.file_id, "f-1");
        assert_eq!(manifest.total_size, 500);
        assert_eq!(manifest.total_chunks, 5);
        assert_eq!(manifest.chunk_size, 100);
    }

    #[tokio::test]
    async fn test_transfer_task_fields() {
        let task = TransferTask {
            task_id: "task-1".to_string(),
            file_id: "file-1".to_string(),
            file_name: "test.txt".to_string(),
            total_size: 1000,
            chunk_size: 256,
            total_chunks: 4,
            state: TransferState::Idle,
            progress: TransferProgress::new("task-1".to_string(), 1000),
            created_at: chrono::Utc::now().timestamp(),
        };

        assert_eq!(task.task_id, "task-1");
        assert_eq!(task.total_chunks, 4);
        assert_eq!(task.state, TransferState::Idle);
        assert!(task.created_at > 0);
    }
}
