//! Error types for security module

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Key generation failed: {0}")]
    KeyGeneration(String),

    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Certificate expired")]
    CertificateExpired,

    #[error("Certificate not yet valid")]
    CertificateNotYetValid,

    #[error("Untrusted certificate")]
    UntrustedCertificate,

    #[error("Crypto error: {0}")]
    Crypto(String),
}

pub type Result<T> = std::result::Result<T, Error>;
