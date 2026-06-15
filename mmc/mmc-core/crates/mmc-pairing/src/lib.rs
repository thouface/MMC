//! Pairing module for device authentication
//! Handles key exchange and device pairing

pub mod error;

/// Pairing result
#[derive(Debug, Clone)]
pub enum PairingResult {
    Success {
        device_id: String,
        device_name: String,
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

/// Pairing state
#[derive(Debug, Clone)]
pub enum PairingState {
    Idle,
    WaitingForConfirmation(String),
    Connected,
    Failed(String),
}

/// Capabilities offered during pairing
#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub file_transfer: bool,
    pub screen_mirror: bool,
    pub remote_control: bool,
    pub clipboard_sync: bool,
}

/// Incoming pairing request
#[derive(Debug, Clone)]
pub struct IncomingRequest {
    pub pairing_id: String,
    pub device_id: String,
    pub device_name: String,
    pub public_key: Vec<u8>,
    pub capabilities: Capabilities,
}

/// Pairing service
pub struct PairingService {
    state: PairingState,
}

impl PairingService {
    pub fn new() -> Self {
        Self {
            state: PairingState::Idle,
        }
    }

    /// Get current state
    pub fn state(&self) -> &PairingState {
        &self.state
    }
}

impl Default for PairingService {
    fn default() -> Self {
        Self::new()
    }
}

pub use error::{Error, Result};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pairing_service_creation() {
        let service = PairingService::new();
        assert!(matches!(service.state(), PairingState::Idle));
    }
}
