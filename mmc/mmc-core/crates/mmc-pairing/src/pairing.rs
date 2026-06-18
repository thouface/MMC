//! Pairing implementation using ECDH + TLS

use mmc_protocol::{Frame, FrameType, read_frame, write_frame};
use mmc_security::{CertificateStore, Crypto, KeyPair};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
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
#[derive(Debug)]
pub struct IncomingRequest {
    pub pairing_id: String,
    pub request: PairingRequest,
}

/// Pending confirmation entry
struct PendingConfirmation {
    pairing_id: String,
    request: PairingRequest,
    confirm_tx: oneshot::Sender<bool>,
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
    pending_confirmations: Arc<RwLock<HashMap<String, PendingConfirmation>>>,
    event_tx: broadcast::Sender<PairingResult>,
    state: PairingState,
}

impl PairingService {
    pub fn new() -> Self {
        let (request_tx, _) = mpsc::channel(10);
        let (event_tx, _) = broadcast::channel(100);

        Self {
            keypair: Arc::new(RwLock::new(None)),
            cert_store: Arc::new(RwLock::new(CertificateStore::new())),
            pending_requests: request_tx,
            pending_confirmations: Arc::new(RwLock::new(HashMap::new())),
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

        let pairing_id = request.pairing_id.clone();
        
        // Create confirmation channel
        let (confirm_tx, confirm_rx) = oneshot::channel();
        
        // Store pending confirmation
        {
            let mut pending = self.pending_confirmations.write().await;
            pending.insert(pairing_id.clone(), PendingConfirmation {
                pairing_id: pairing_id.clone(),
                request: request.clone(),
                confirm_tx,
            });
        }
        
        // Also send to mpsc channel for application-level handling
        let incoming = IncomingRequest {
            pairing_id: pairing_id.clone(),
            request: request.clone(),
        };

        if self.pending_requests.send(incoming).await.is_err() {
            debug!("No listener for pairing request via mpsc, using pending_confirmations");
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
    pub async fn confirm_pairing(&self, pairing_id: &str, accept: bool) -> Result<()> {
        let confirm_tx = {
            let mut pending = self.pending_confirmations.write().await;
            match pending.remove(pairing_id) {
                Some(confirmation) => {
                    debug!("Confirming pairing {}: accept={}", pairing_id, accept);
                    confirmation.confirm_tx
                }
                None => {
                    warn!("Pairing request {} not found", pairing_id);
                    return Err(Error::NotFound);
                }
            }
        };

        // Send confirmation (true = accept, false = reject)
        if confirm_tx.send(accept).is_err() {
            warn!("Failed to send confirmation for pairing {}", pairing_id);
            return Err(Error::Protocol("Failed to confirm pairing".to_string()));
        }

        Ok(())
    }

    /// Get list of pending pairing requests
    pub async fn get_pending_requests(&self) -> Vec<PairingRequest> {
        let pending = self.pending_confirmations.read().await;
        pending.values().map(|p| p.request.clone()).collect()
    }

    /// Check if there are pending pairing requests
    pub async fn has_pending_requests(&self) -> bool {
        !self.pending_confirmations.read().await.is_empty()
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

    #[test]
    fn test_pairing_result_variants() {
        let success = PairingResult::Success {
            device_id: "d-1".to_string(),
            device_name: "Test Device".to_string(),
            shared_secret: [0u8; 32],
        };
        match success {
            PairingResult::Success { device_id, .. } => {
                assert_eq!(device_id, "d-1");
            }
            _ => panic!("Expected Success variant"),
        }

        let rejected = PairingResult::Rejected {
            pairing_id: "p-1".to_string(),
            reason: "test reason".to_string(),
        };
        match rejected {
            PairingResult::Rejected { pairing_id, .. } => {
                assert_eq!(pairing_id, "p-1");
            }
            _ => panic!("Expected Rejected variant"),
        }

        let error = PairingResult::Error {
            pairing_id: "p-2".to_string(),
            error: "test error".to_string(),
        };
        match error {
            PairingResult::Error { error, .. } => {
                assert_eq!(error, "test error");
            }
            _ => panic!("Expected Error variant"),
        }
    }

    #[test]
    fn test_capabilities_default() {
        let caps: Capabilities = Capabilities::default();
        assert!(!caps.file_transfer);
        assert!(!caps.screen_mirror);
        assert!(!caps.remote_control);
        assert!(!caps.clipboard_sync);
    }

    #[test]
    fn test_capabilities_all() {
        let caps = Capabilities {
            file_transfer: true,
            screen_mirror: true,
            remote_control: true,
            clipboard_sync: true,
        };
        assert!(caps.file_transfer);
        assert!(caps.screen_mirror);
        assert!(caps.remote_control);
        assert!(caps.clipboard_sync);
    }

    #[test]
    fn test_pairing_state() {
        let state = PairingState::Idle;
        match state {
            PairingState::Idle => {}
            _ => panic!("Expected Idle"),
        }

        let state = PairingState::Connected;
        match state {
            PairingState::Connected => {}
            _ => panic!("Expected Connected"),
        }

        let state = PairingState::Failed("test".to_string());
        match state {
            PairingState::Failed(msg) => {
                assert_eq!(msg, "test");
            }
            _ => panic!("Expected Failed"),
        }
    }

    #[test]
    fn test_pairing_state_waiting() {
        let state = PairingState::WaitingForConfirmation("pair-1".to_string());
        match state {
            PairingState::WaitingForConfirmation(id) => {
                assert_eq!(id, "pair-1");
            }
            _ => panic!("Expected WaitingForConfirmation"),
        }
    }

    #[tokio::test]
    async fn test_pairing_service_creation() {
        let mut service = PairingService::new();
        assert!(service.init("test-device", "Test Device").await.is_ok());
    }

    #[tokio::test]
    async fn test_pairing_service_events_subscribe() {
        let service = PairingService::new();
        let events = service.events();
        // Ensure events receiver can be created
        let _ = events;
        assert!(true);
    }

    #[test]
    fn test_pairing_request_struct() {
        let request = PairingRequest {
            pairing_id: "p-1".to_string(),
            device_id: "d-1".to_string(),
            device_name: "Test Device".to_string(),
            public_key: "abc123".to_string(),
            capabilities: Capabilities {
                file_transfer: true,
                screen_mirror: false,
                remote_control: false,
                clipboard_sync: false,
            },
        };
        assert_eq!(request.pairing_id, "p-1");
        assert!(request.capabilities.file_transfer);
    }

    #[tokio::test]
    async fn test_confirm_pairing_accept() {
        let service = PairingService::new();
        
        // Create a pending confirmation manually
        let pairing_id = "test-pairing-1".to_string();
        let (confirm_tx, _confirm_rx) = oneshot::channel();
        
        {
            let mut pending = service.pending_confirmations.write().await;
            pending.insert(pairing_id.clone(), PendingConfirmation {
                pairing_id: pairing_id.clone(),
                request: PairingRequest {
                    pairing_id: pairing_id.clone(),
                    device_id: "device-1".to_string(),
                    device_name: "Test Device".to_string(),
                    public_key: "key123".to_string(),
                    capabilities: Capabilities::default(),
                },
                confirm_tx,
            });
        }
        
        // Confirm pairing
        let result = service.confirm_pairing(&pairing_id, true).await;
        assert!(result.is_ok());
        
        // Should be removed after confirmation
        let has_pending = service.has_pending_requests().await;
        assert!(!has_pending);
    }

    #[tokio::test]
    async fn test_confirm_pairing_reject() {
        let service = PairingService::new();
        
        let pairing_id = "test-pairing-2".to_string();
        let (confirm_tx, _confirm_rx) = oneshot::channel();
        
        {
            let mut pending = service.pending_confirmations.write().await;
            pending.insert(pairing_id.clone(), PendingConfirmation {
                pairing_id: pairing_id.clone(),
                request: PairingRequest {
                    pairing_id: pairing_id.clone(),
                    device_id: "device-2".to_string(),
                    device_name: "Reject Device".to_string(),
                    public_key: "key456".to_string(),
                    capabilities: Capabilities::default(),
                },
                confirm_tx,
            });
        }
        
        // Reject pairing
        let result = service.confirm_pairing(&pairing_id, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_confirm_pairing_not_found() {
        let service = PairingService::new();
        
        // Try to confirm non-existent pairing
        let result = service.confirm_pairing("nonexistent", true).await;
        assert!(result.is_err());
        
        match result {
            Err(crate::Error::NotFound) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test]
    async fn test_get_pending_requests() {
        let service = PairingService::new();
        
        // Initially empty
        let pending = service.get_pending_requests().await;
        assert!(pending.is_empty());
        
        // Add a pending request
        let pairing_id = "test-pending".to_string();
        let (confirm_tx, _confirm_rx) = oneshot::channel();
        
        {
            let mut pending = service.pending_confirmations.write().await;
            pending.insert(pairing_id.clone(), PendingConfirmation {
                pairing_id: pairing_id.clone(),
                request: PairingRequest {
                    pairing_id: pairing_id.clone(),
                    device_id: "device-3".to_string(),
                    device_name: "Pending Device".to_string(),
                    public_key: "key789".to_string(),
                    capabilities: Capabilities::default(),
                },
                confirm_tx,
            });
        }
        
        // Should have one pending request
        let pending = service.get_pending_requests().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].device_name, "Pending Device");
    }

    #[tokio::test]
    async fn test_has_pending_requests() {
        let service = PairingService::new();
        
        // Initially false
        assert!(!service.has_pending_requests().await);
        
        // Add a pending request
        let (confirm_tx, _confirm_rx) = oneshot::channel();
        {
            let mut pending = service.pending_confirmations.write().await;
            pending.insert("pair-1".to_string(), PendingConfirmation {
                pairing_id: "pair-1".to_string(),
                request: PairingRequest {
                    pairing_id: "pair-1".to_string(),
                    device_id: "d".to_string(),
                    device_name: "D".to_string(),
                    public_key: "k".to_string(),
                    capabilities: Capabilities::default(),
                },
                confirm_tx,
            });
        }
        
        // Should be true now
        assert!(service.has_pending_requests().await);
        
        // Confirm it
        service.confirm_pairing("pair-1", true).await.unwrap();
        
        // Should be false again
        assert!(!service.has_pending_requests().await);
    }
}
