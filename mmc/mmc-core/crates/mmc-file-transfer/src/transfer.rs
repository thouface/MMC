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

    /// Receive chunk data and write to output file
    pub async fn receive_chunk(&self, task_id: &str, chunk_data: &[u8]) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.progress.update(
                task.progress.bytes_transferred + chunk_data.len() as u64,
                std::time::Instant::now().elapsed().as_millis() as u64,
            );
            let _ = self.event_tx.send(task.progress.clone());
        }
        Ok(())
    }

    /// Finish receiving a file
    pub async fn finish_receive(&self, task_id: &str) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.progress.complete();
            task.state = TransferState::Completed;
            let _ = self.event_tx.send(task.progress.clone());
        }
        Ok(())
    }

    /// Receive a file from a peer via an established stream.
    /// Returns BLAKE3 hash of the received file contents for optional verification.
    pub async fn receive_file(
        &self,
        task_id: &str,
        manifest: ChunkManifest,
        output_path: &Path,
        stream: Option<tokio::net::TcpStream>,
    ) -> Result<[u8; 32]> {
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

        // If no stream is provided, just create the empty output file (passive mode)
        let mut stream = match stream {
            Some(s) => s,
            None => {
                let _file = File::create(output_path).await.map_err(|e| Error::Io(e.to_string()))?;
                let mut tasks = self.tasks.write().await;
                if let Some(task) = tasks.get_mut(task_id) {
                    task.progress.complete();
                    task.state = TransferState::Completed;
                    let _ = self.event_tx.send(task.progress.clone());
                }
                return Ok([0u8; 32]);
            }
        };

        use mmc_protocol::{read_frame, FrameType};
        let mut file = File::create(output_path).await.map_err(|e| Error::Io(e.to_string()))?;
        let mut hasher = blake3::Hasher::new();
        let mut bytes_received: u64 = 0;
        let start_time = std::time::Instant::now();

        // Read chunks from stream until TransferComplete
        let mut _chunk_index = 0u32;
        loop {
            let frame = read_frame(&mut stream)
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?
                .ok_or_else(|| Error::Protocol("Unexpected end of stream".to_string()))?;

            if frame.frame_type == FrameType::TransferComplete {
                break;
            }

            if frame.frame_type != FrameType::ChunkData {
                return Err(Error::Protocol(format!(
                    "Unexpected frame type: {:?}",
                    frame.frame_type
                )));
            }

            // Write to file and hash
            tokio::io::AsyncWriteExt::write_all(&mut file, &frame.payload)
                .await
                .map_err(|e| Error::Io(e.to_string()))?;
            hasher.update(&frame.payload);
            bytes_received += frame.payload.len() as u64;

            // Update task progress
            {
                let mut tasks = self.tasks.write().await;
                if let Some(task) = tasks.get_mut(task_id) {
                    task.progress.update(
                        bytes_received,
                        start_time.elapsed().as_millis() as u64,
                    );
                    let _ = self.event_tx.send(task.progress.clone());
                }
            }

            _chunk_index += 1;
        }

        tokio::io::AsyncWriteExt::flush(&mut file)
            .await
            .map_err(|e| Error::Io(e.to_string()))?;

        let hash = *hasher.finalize().as_bytes();

        // Mark complete
        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(task_id) {
                task.progress.complete();
                task.state = TransferState::Completed;
                let _ = self.event_tx.send(task.progress.clone());
            }
        }

        info!("File received: {} ({} bytes)", manifest.file_name, bytes_received);

        Ok(hash)
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

    // ========== End-to-End File Transfer Tests ==========

    /// Simulate sending a file to a peer using our Frame protocol on a real tokio TCP pair.
    /// Verifies that the received bytes match the original file bytes exactly.
    #[tokio::test]
    async fn test_e2e_file_transfer_tcp_pair() {
        use mmc_protocol::{Frame, FrameType, read_frame, write_frame};

        let dir = tempdir().unwrap();
        let input_path = dir.path().join("source.bin");
        let output_path = dir.path().join("received.bin");

        // Prepare source file with deterministic mixed content (for good hashing coverage)
        let mut source_bytes: Vec<u8> = Vec::new();
        for i in 0..2000 {
            source_bytes.push((i * 7 + 3) as u8);
        }
        tokio::fs::write(&input_path, &source_bytes).await.unwrap();

        let chunk_size = 512u32;
        let service = TransferService::new();
        let manifest = service.compute_manifest(&input_path, chunk_size).await.unwrap();
        assert_eq!(manifest.total_size, source_bytes.len() as u64);
        assert!(manifest.total_chunks > 1);

        // Establish an in-memory duplex pair for the protocol exchange
        let (mut sender_stream, mut receiver_stream) = tokio::io::duplex(4096);

        // --- Sender task: Send manifest, then chunks, then TransferComplete ---
        let manifest_clone = manifest.clone();
        let input_path_clone = input_path.clone();
        let sender_handle = tokio::spawn(async move {
            // Send manifest as JSON in a FileManifestRequest frame
            let manifest_json = serde_json::to_vec(&manifest_clone).unwrap();
            write_frame(
                &mut sender_stream,
                &Frame::new(FrameType::FileManifestRequest, manifest_json),
            )
            .await
            .unwrap();

            // Read chunks from file and send
            let mut file = File::open(&input_path_clone).await.unwrap();
            let mut buf = vec![0u8; chunk_size as usize];
            loop {
                let bytes_read = file.read(&mut buf).await.unwrap();
                if bytes_read == 0 {
                    break;
                }
                write_frame(
                    &mut sender_stream,
                    &Frame::new(FrameType::ChunkData, buf[..bytes_read].to_vec()),
                )
                .await
                .unwrap();
            }

            // Signal transfer complete
            write_frame(
                &mut sender_stream,
                &Frame::new(FrameType::TransferComplete, vec![]),
            )
            .await
            .unwrap();
        });

        // --- Receiver task: Read manifest, then chunks, then verify ---
        let receiver_handle = tokio::spawn(async move {
            // Expect manifest frame
            let manifest_frame = read_frame(&mut receiver_stream)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(manifest_frame.frame_type, FrameType::FileManifestRequest);
            let received_manifest: ChunkManifest =
                serde_json::from_slice(&manifest_frame.payload).unwrap();
            assert_eq!(received_manifest.total_size, manifest.total_size);
            assert_eq!(received_manifest.chunk_size, manifest.chunk_size);

            // Read chunk data until TransferComplete
            let mut received_bytes: Vec<u8> = Vec::with_capacity(received_manifest.total_size as usize);
            loop {
                let frame = read_frame(&mut receiver_stream)
                    .await
                    .unwrap()
                    .unwrap();

                if frame.frame_type == FrameType::TransferComplete {
                    break;
                }
                assert_eq!(frame.frame_type, FrameType::ChunkData);
                received_bytes.extend_from_slice(&frame.payload);
            }

            received_bytes
        });

        sender_handle.await.unwrap();
        let received_bytes = receiver_handle.await.unwrap();

        // Write received file to disk (as the real flow would do)
        tokio::fs::write(&output_path, &received_bytes).await.unwrap();

        // Verify the received file matches the source
        let output_bytes = tokio::fs::read(&output_path).await.unwrap();
        assert_eq!(output_bytes.len(), source_bytes.len());
        assert_eq!(output_bytes, source_bytes);
    }

    /// Small file (single chunk) end-to-end transfer test
    #[tokio::test]
    async fn test_e2e_small_file_single_chunk() {
        use mmc_protocol::{Frame, FrameType, read_frame, write_frame};

        let dir = tempdir().unwrap();
        let input_path = dir.path().join("small.txt");
        let output_path = dir.path().join("recv.txt");

        let text = b"Hello, MMC file transfer!";
        tokio::fs::write(&input_path, text).await.unwrap();

        let service = TransferService::new();
        let manifest = service.compute_manifest(&input_path, 1024).await.unwrap();
        assert_eq!(manifest.total_chunks, 1);

        let (mut sender, mut receiver) = tokio::io::duplex(4096);

        // Send
        tokio::spawn(async move {
            let manifest_json = serde_json::to_vec(&manifest).unwrap();
            write_frame(
                &mut sender,
                &Frame::new(FrameType::FileManifestRequest, manifest_json),
            )
            .await
            .unwrap();

            let mut file = File::open(&input_path).await.unwrap();
            let mut buf = vec![0u8; 1024];
            loop {
                let bytes_read = file.read(&mut buf).await.unwrap();
                if bytes_read == 0 { break; }
                write_frame(
                    &mut sender,
                    &Frame::new(FrameType::ChunkData, buf[..bytes_read].to_vec()),
                )
                .await
                .unwrap();
            }
            write_frame(&mut sender, &Frame::new(FrameType::TransferComplete, vec![]))
                .await
                .unwrap();
        });

        // Receive
        let manifest_frame = read_frame(&mut receiver).await.unwrap().unwrap();
        assert_eq!(manifest_frame.frame_type, FrameType::FileManifestRequest);
        let received_manifest: ChunkManifest = serde_json::from_slice(&manifest_frame.payload).unwrap();
        assert_eq!(received_manifest.total_chunks, 1);

        let mut data = Vec::new();
        loop {
            let frame = read_frame(&mut receiver).await.unwrap().unwrap();
            if frame.frame_type == FrameType::TransferComplete { break; }
            data.extend_from_slice(&frame.payload);
        }

        tokio::fs::write(&output_path, &data).await.unwrap();
        let received = tokio::fs::read(&output_path).await.unwrap();
        assert_eq!(received, text.to_vec());
    }

    /// Verify manifest chunk hashes match actual file content
    #[tokio::test]
    async fn test_e2e_manifest_hash_verification() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("content.bin");

        // File content - 1500 bytes
        let content: Vec<u8> = (0..1500).map(|i| (i * 5 + 11) as u8).collect();
        tokio::fs::write(&file_path, &content).await.unwrap();

        let service = TransferService::new();
        let manifest = service.compute_manifest(&file_path, 500).await.unwrap();

        // Verify: chunks cover the entire file
        assert_eq!(manifest.total_chunks, 3);
        assert_eq!(manifest.total_size, 1500);

        // Verify: each chunk's BLAKE3 hash matches the hashes we compute independently
        for chunk_info in &manifest.chunks {
            let start = (chunk_info.index * 500) as usize;
            let end = (start + chunk_info.size as usize).min(1500);
            let expected_hash = blake3::hash(&content[start..end]);
            assert_eq!(chunk_info.hash, *expected_hash.as_bytes());
        }
    }

    /// Test progress event stream during a transfer
    #[tokio::test]
    async fn test_e2e_progress_events() {
        let service = TransferService::new();

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("progress_test.bin");
        let content: Vec<u8> = (0..2000).map(|i| i as u8).collect();
        tokio::fs::write(&file_path, &content).await.unwrap();

        let manifest = service.compute_manifest(&file_path, 500).await.unwrap();

        // First register the task via receive_file in passive mode (no stream)
        let output_path = dir.path().join("recv.bin");
        service
            .receive_file("progress-task", manifest.clone(), &output_path, None)
            .await
            .unwrap();

        // Subscribe after task registration to ensure broadcast captures our interest
        let mut events = service.events();

        // Simulate receiving chunks
        service.receive_chunk("progress-task", &content[..500]).await.unwrap();
        service.receive_chunk("progress-task", &content[500..1000]).await.unwrap();
        service.receive_chunk("progress-task", &content[1000..1500]).await.unwrap();
        service.receive_chunk("progress-task", &content[1500..2000]).await.unwrap();
        service.finish_receive("progress-task").await.unwrap();

        // Drain any broadcast events (non-blocking)
        let mut count = 0;
        let mut saw_completed = false;
        while let Ok(ev) = events.try_recv() {
            if ev.task_id == "progress-task" {
                count += 1;
                if ev.state == TransferState::Completed {
                    saw_completed = true;
                    assert_eq!(ev.bytes_transferred, ev.total_bytes);
                }
            }
        }
        // Count should at least include the final Complete event
        assert!(saw_completed || count >= 1, "Expected progress/completion events");
    }

    /// End-to-End test using TransferService::receive_file with a stream
    /// (Caller reads the manifest first, then passes stream to receive_file)
    #[tokio::test]
    async fn test_e2e_transfer_service_full_flow() {
        let dir = tempdir().unwrap();
        let input_path = dir.path().join("send.bin");
        let output_path = dir.path().join("recv.bin");

        // 50KB of pseudo-random bytes
        let mut source_bytes: Vec<u8> = Vec::with_capacity(50_000);
        for i in 0..50_000u64 {
            source_bytes.push(((i * 13 + 17) & 0xFF) as u8);
        }
        tokio::fs::write(&input_path, &source_bytes).await.unwrap();

        let chunk_size = 2048u32;
        let service_send = Arc::new(TransferService::new());
        let service_recv = Arc::new(TransferService::new());

        // Set up listener on a random port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let input_path_clone = input_path.clone();

        // Compute manifest
        let manifest = service_send
            .compute_manifest(&input_path, chunk_size)
            .await
            .unwrap();

        // Sender: connect and send (manifest + chunks + complete)
        let send_handle = tokio::spawn(async move {
            use mmc_protocol::{Frame, FrameType, write_frame};
            let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();

            // Send manifest
            let manifest_json = serde_json::to_vec(&manifest).unwrap();
            write_frame(
                &mut stream,
                &Frame::new(FrameType::FileManifestRequest, manifest_json),
            )
            .await
            .unwrap();

            // Send all chunks
            let mut file = File::open(&input_path_clone).await.unwrap();
            let mut buf = vec![0u8; chunk_size as usize];
            loop {
                let bytes_read = file.read(&mut buf).await.unwrap();
                if bytes_read == 0 { break; }
                write_frame(
                    &mut stream,
                    &Frame::new(FrameType::ChunkData, buf[..bytes_read].to_vec()),
                )
                .await
                .unwrap();
            }

            // Signal complete
            write_frame(
                &mut stream,
                &Frame::new(FrameType::TransferComplete, vec![]),
            )
            .await
            .unwrap();
        });

        // Receiver: accept, read manifest separately, then pass remaining stream to receive_file
        let recv_service = service_recv.clone();
        let output_path_clone = output_path.clone();
        let recv_handle = tokio::spawn(async move {
            use mmc_protocol::{FrameType, read_frame};
            let (mut stream, _) = listener.accept().await.unwrap();

            // 1. Read and parse manifest ourselves (caller responsibility)
            let manifest_frame = read_frame(&mut stream).await.unwrap().unwrap();
            assert_eq!(manifest_frame.frame_type, FrameType::FileManifestRequest);
            let received_manifest: ChunkManifest =
                serde_json::from_slice(&manifest_frame.payload).unwrap();

            // 2. Now pass the already-connected stream to receive_file for chunk reading
            recv_service
                .receive_file(
                    "task-e2e-full",
                    received_manifest,
                    &output_path_clone,
                    Some(stream),
                )
                .await
                .unwrap()
        });

        send_handle.await.unwrap();
        let _ = recv_handle.await.unwrap();

        let received_bytes = tokio::fs::read(&output_path).await.unwrap();
        assert_eq!(received_bytes.len(), source_bytes.len());
        assert_eq!(received_bytes, source_bytes);
    }
}
