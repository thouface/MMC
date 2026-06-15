//! Pairing module for device authentication
//! Handles key exchange and device pairing

pub mod error;
pub mod pairing;

pub use pairing::{
    Capabilities, IncomingRequest, PairingResult, PairingService, PairingState,
};
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
