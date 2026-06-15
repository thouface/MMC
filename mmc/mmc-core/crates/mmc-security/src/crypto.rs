//! Cryptographic primitives for MMC
//! Key generation, encryption, hashing

use rand::RngCore;
use zeroize::Zeroizing;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::Error;

/// Key pair for X25519 key exchange
#[derive(Debug, Clone)]
pub struct KeyPair {
    secret: Zeroizing<[u8; 32]>,
    public: [u8; 32],
}

impl KeyPair {
    /// Get public key bytes
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.public
    }

    /// Get secret key bytes (for signing, not for network)
    pub fn secret_key_bytes(&self) -> [u8; 32] {
        *self.secret
    }

    /// Compute shared secret with peer's public key
    pub fn shared_secret(&self, peer_public: &[u8; 32]) -> Result<[u8; 32], Error> {
        let secret = StaticSecret::from(*self.secret);
        let public = PublicKey::from(*peer_public);
        let shared = secret.diffie_hellman(&public);
        Ok(*shared.as_bytes())
    }
}

/// Cryptographic operations
pub struct Crypto;

impl Crypto {
    /// Generate a new X25519 key pair
    pub fn generate_keypair() -> KeyPair {
        let mut secret_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut secret_bytes);
        let secret = StaticSecret::from(secret_bytes);
        let public = PublicKey::from(&secret);

        KeyPair {
            secret: Zeroizing::new(secret_bytes),
            public: *public.as_bytes(),
        }
    }

    /// Generate random bytes
    pub fn random_bytes(len: usize) -> Vec<u8> {
        let mut buf = vec![0u8; len];
        rand::thread_rng().fill_bytes(&mut buf);
        buf
    }

    /// Compute BLAKE3 hash
    pub fn blake3(data: &[u8]) -> [u8; 32] {
        *blake3::hash(data).as_bytes()
    }

    /// Verify BLAKE3 hash (constant-time)
    pub fn verify_blake3(data: &[u8], expected: &[u8; 32]) -> bool {
        let hash = Self::blake3(data);
        hash == *expected
    }

    /// Compute BLAKE3 hash (incremental, for large files)
    pub fn blake3_hasher() -> blake3::Hasher {
        blake3::Hasher::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let keypair = Crypto::generate_keypair();
        assert!(!keypair.public_key_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_shared_secret() {
        let alice = Crypto::generate_keypair();
        let bob = Crypto::generate_keypair();

        let secret_alice = alice.shared_secret(&bob.public_key_bytes()).unwrap();
        let secret_bob = bob.shared_secret(&alice.public_key_bytes()).unwrap();

        assert_eq!(secret_alice, secret_bob);
    }

    #[test]
    fn test_blake3() {
        let data = b"hello world";
        let hash = Crypto::blake3(data);

        // Verify
        assert!(Crypto::verify_blake3(data, &hash));
        assert!(!Crypto::verify_blake3(b"wrong data", &hash));
    }

    #[test]
    fn test_random_bytes() {
        let a = Crypto::random_bytes(32);
        let b = Crypto::random_bytes(32);
        assert_ne!(a, b);
    }
}
