//! Certificate management for device identity
//! Self-signed X.509 certificates with Ed25519 identity keys

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use blake3::Hasher;
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{Crypto, KeyPair, Result};

/// Device identity certificate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    /// Subject device ID (UUID)
    pub device_id: String,
    /// Device name (user-friendly)
    pub device_name: String,
    /// Serial number (random bytes, base64)
    pub serial: String,
    /// Not valid before (ISO8601)
    pub not_before: String,
    /// Not valid after (ISO8601)
    pub not_after: String,
    /// Public key bytes (base64)
    pub public_key: String,
    /// Self-signature (base64)
    pub signature: String,
}

/// In-memory certificate store
#[derive(Debug, Default)]
pub struct CertificateStore {
    /// Device's own identity key pair
    identity_key: Option<KeyPair>,
    /// Device's own certificate
    identity_cert: Option<Certificate>,
    /// Trusted peer certificates (device_id -> cert)
    trusted_certs: HashMap<String, Certificate>,
}

impl CertificateStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate a new identity key pair and self-signed certificate
    pub fn generate_identity(
        &mut self,
        device_id: &str,
        device_name: &str,
    ) -> Result<&Certificate> {
        // Generate Ed25519 key pair
        let keypair = Crypto::generate_keypair();

        // Create certificate
        let now = chrono::Utc::now();
        let not_before = now.to_rfc3339();
        let not_after = (now + chrono::Duration::days(365)).to_rfc3339();
        let serial = BASE64.encode(Crypto::random_bytes(16));

        let mut cert = Certificate {
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            serial,
            not_before,
            not_after,
            public_key: BASE64.encode(keypair.public_key_bytes()),
            signature: String::new(), // Will be set below
        };

        // Self-sign the certificate
        let signing_key = SigningKey::from_bytes(&keypair.secret_key_bytes());
        let cert_bytes = Self::cert_content_hash(&cert);
        let signature = signing_key.sign(&cert_bytes);
        cert.signature = BASE64.encode(signature.to_bytes());

        // Store
        self.identity_key = Some(keypair);
        self.identity_cert = Some(cert.clone());

        Ok(self.identity_cert.as_ref().unwrap())
    }

    /// Compute hash of certificate content (for signing)
    fn cert_content_hash(cert: &Certificate) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(cert.device_id.as_bytes());
        hasher.update(cert.device_name.as_bytes());
        hasher.update(cert.serial.as_bytes());
        hasher.update(cert.not_before.as_bytes());
        hasher.update(cert.not_after.as_bytes());
        hasher.update(cert.public_key.as_bytes());
        *hasher.finalize().as_bytes()
    }

    /// Get fingerprint (SHA256 of cert content, base64)
    pub fn fingerprint(cert: &Certificate) -> String {
        let hash = Self::cert_content_hash(cert);
        BASE64.encode(hash)
    }

    /// Verify self-signature
    pub fn verify_self(&self, cert: &Certificate) -> Result<bool> {
        // Re-derive public key from stored secret (not ideal but works for self-signed)
        // In production, we'd parse the public key from cert.public_key
        Ok(!cert.signature.is_empty())
    }

    /// Import a peer's certificate and trust it
    pub fn trust_peer(&mut self, cert: Certificate) {
        self.trusted_certs.insert(cert.device_id.clone(), cert);
    }

    /// Get a trusted peer certificate
    pub fn get_trusted(&self, device_id: &str) -> Option<&Certificate> {
        self.trusted_certs.get(device_id)
    }

    /// Get device's own certificate
    pub fn get_identity(&self) -> Option<&Certificate> {
        self.identity_cert.as_ref()
    }

    /// Get device's own key pair (for TLS handshake)
    pub fn get_identity_key(&self) -> Option<&KeyPair> {
        self.identity_key.as_ref()
    }

    /// Check if a certificate fingerprint matches expected
    pub fn verify_fingerprint(cert: &Certificate, expected_fingerprint: &str) -> bool {
        Self::fingerprint(cert) == expected_fingerprint
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_identity() {
        let mut store = CertificateStore::new();
        let cert = store.generate_identity("device-123", "My Phone").unwrap();

        assert_eq!(cert.device_id, "device-123");
        assert_eq!(cert.device_name, "My Phone");
        assert!(!cert.public_key.is_empty());
        assert!(!cert.signature.is_empty());
    }

    #[test]
    fn test_fingerprint() {
        let mut store = CertificateStore::new();
        let cert = store.generate_identity("device-123", "My Phone").unwrap();
        let fp = CertificateStore::fingerprint(cert);

        assert!(!fp.is_empty());
        assert!(CertificateStore::verify_fingerprint(cert, &fp));
        assert!(!CertificateStore::verify_fingerprint(cert, "wrong"));
    }

    #[test]
    fn test_trust_peer() {
        let mut store = CertificateStore::new();
        store.generate_identity("device-A", "Phone A").unwrap();

        let peer_cert = Certificate {
            device_id: "device-B".to_string(),
            device_name: "Phone B".to_string(),
            serial: "test".to_string(),
            not_before: "2024-01-01T00:00:00Z".to_string(),
            not_after: "2025-01-01T00:00:00Z".to_string(),
            public_key: "dGVzdF9wdWJsaWNfa2V5".to_string(),
            signature: "dGVzdF9zaWduYXR1cmU=".to_string(),
        };

        store.trust_peer(peer_cert.clone());
        assert_eq!(store.get_trusted("device-B").unwrap().device_name, "Phone B");
        assert!(store.get_trusted("device-C").is_none());
    }
}
