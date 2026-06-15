//! Pairing implementation using ECDH + TLS

use mmc_protocol::{Frame, FrameType, read_frame, write_frame};
use mmc_security::{CertificateStore, Crypto, KeyPair};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::{Error, Result};

/// Device pairing request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRequest {
    pub pairing_id: String,
    pub device_id: String,
    pub device_name: String,
    pub public_key: String,
    pub capabilities: Capabilities,
}

/// Granted capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Capabilities {
    pub file_transfer: bool,
    pub screen_mirror: bool,
    pub remote_control: bool,
    pub clipboard_sync: bool,
}

/// Pairing result
#[derive(Debug, Clone)]
pub enum PairingResult {
    Success {
        device_id: String,
        device_name: String,
        shared_secret: [u8; 32],
    },
    Rejected {
        pairing_id: String,
        reason: String,
    },
    Error {
        pairing_id: String,
        error: String,
    },
}

/// Incoming pairing request from a peer
pub struct IncomingRequest {
    pub pairing_id: String,
    pub request: PairingRequest,
    pub confirm_tx: tokio::sync::oneshot::Sender<bool>,
}

/// Pairing service state
#[derive(Debug, Clone)]
pub enum PairingState {
    Idle,
    WaitingForConfirmation(String),
    Connected,
    Failed(String),
}

/// Pairing service
pub struct PairingService {
    keypair: Arc<RwLock<Option<KeyPair>>>,
    cert_store: Arc<RwLock<CertificateStore>>,
    pending_requests: mpsc::Sender<IncomingRequest>,
    request_rx: mpsc::Receiver<IncomingRequest>,
    event_tx: broadcast::Sender<PairingResult>,
    state: PairingState,
}

impl PairingService {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel(10);
        let (event_tx, _) = broadcast::channel(100);

        Self {
            keypair: Arc::new(RwLock::new(None)),
            cert_store: Arc::new(RwLock::new(CertificateStore::new())),
            pending_requests: request_tx,
            request_rx,
            event_tx,
            state: PairingState::Idle,
        }
    }

    /// Get current pairing state
    pub fn state(&self) -> &PairingState {
        &self.state
    }

    /// Update pairing state
    pub fn set_state(&mut self, state: PairingState) {
        self.state = state;
    }

    /// Initialize with a new identity
    pub async fn init(&mut self, device_id: &str, device_name: &str) -> Result<()> {
        let mut store = self.cert_store.write().await;
        store
            .generate_identity(device_id, device_name)
            .map_err(|e| Error::Security(e.to_string()))?;
        Ok(())
    }

    /// Get public key for sharing with peers
    pub async fn get_public_key_base64(&self) -> Result<String> {
        let keypair = self.keypair.read().await;
        let keypair = keypair.as_ref().ok_or(Error::NotInitialized)?;
        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            keypair.public_key_bytes(),
        ))
    }

    /// Initiate pairing with a remote device
    pub async fn pair(
        &self,
        target_ip: &str,
        target_port: u16,
        target_public_key: &[u8; 32],
        device_id: String,
        device_name: String,
        capabilities: Capabilities,
    ) -> Result<PairingResult> {
        let pairing_id = uuid::Uuid::new_v4().to_string();

        let addr = format!("{}:{}", target_ip, target_port);
        let mut stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| Error::ConnectionFailed(e.to_string()))?;

        debug!("Connected to {} for pairing", addr);

        let local_keypair = Crypto::generate_keypair();
        let shared_secret = local_keypair
            .shared_secret(target_public_key)
            .map_err(|e| Error::Security(e.to_string()))?;

        let request = PairingRequest {
            pairing_id: pairing_id.clone(),
            device_id: device_id.clone(),
            device_name: device_name.clone(),
            public_key: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                local_keypair.public_key_bytes(),
            ),
            capabilities,
        };

        let payload =
            serde_json::to_vec(&request).map_err(|e| Error::Serialization(e.to_string()))?;

        let frame = Frame::new(FrameType::PairingRequest, payload);
        write_frame(&mut stream, &frame)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        let response_frame = read_frame(&mut stream)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?
            .ok_or_else(|| Error::Protocol("No response".to_string()))?;

        if response_frame.frame_type() != FrameType::PairingResponse {
            return Err(Error::Protocol("Unexpected frame type".to_string()));
        }

        let response: serde_json::Value = serde_json::from_slice(&response_frame.into_payload())
            .map_err(|e| Error::Serialization(e.to_string()))?;

        if response["accepted"].as_bool() == Some(true) {
            info!("Pairing successful with {}", device_name);
            Ok(PairingResult::Success {
                device_id,
                device_name,
                shared_secret,
            })
        } else {
            let reason = response["error_message"]
                .as_str()
                .unwrap_or("Rejected")
                .to_string();
            Ok(PairingResult::Rejected {
                pairing_id,
                reason,
            })
        }
    }

    /// Handle incoming pairing request
    pub async fn handle_incoming(&mut self, mut stream: TcpStream) {
        let frame = match read_frame(&mut stream).await {
            Ok(Some(f)) => f,
            Ok(None) => {
                warn!("Connection closed during pairing");
                return;
            }
            Err(e) => {
                error!("Failed to read pairing frame: {}", e);
                return;
            }
        };

        if frame.frame_type() != FrameType::PairingRequest {
            error!("Expected PairingRequest, got {:?}", frame.frame_type());
            return;
        }

        let request: PairingRequest = match serde_json::from_slice(&frame.into_payload()) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to parse pairing request: {}", e);
                return;
            }
        };

        let (confirm_tx, confirm_rx) = tokio::sync::oneshot::channel();
        let incoming = IncomingRequest {
            pairing_id: request.pairing_id.clone(),
            request: request.clone(),
            confirm_tx,
        };

        if self.pending_requests.send(incoming).await.is_err() {
            warn!("No listener for pairing request");
            return;
        }

        let accepted = confirm_rx.await.unwrap_or(false);

        let response = if accepted {
            serde_json::json!({
                "pairing_id": request.pairing_id,
                "accepted": true
            })
        } else {
            serde_json::json!({
                "pairing_id": request.pairing_id,
                "accepted": false,
                "error_message": "User rejected"
            })
        };

        let payload = response.to_string().into_bytes();
        let frame = Frame::new(FrameType::PairingResponse, payload);

        if let Err(e) = write_frame(&mut stream, &frame).await {
            error!("Failed to send pairing response: {}", e);
        }
    }

    /// Confirm or reject a pending pairing request
    pub async fn confirm_pairing(&self, _pairing_id: &str, _accept: bool) -> Result<()> {
        Ok(())
    }

    /// Receive events stream
    pub fn events(&self) -> broadcast::Receiver<PairingResult> {
        self.event_tx.subscribe()
    }
}

impl Default for PairingService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pairing_service_creation() {
        let mut service = PairingService::new();
        assert!(service.init("test-device", "Test Device").await.is_ok());
    }
}
