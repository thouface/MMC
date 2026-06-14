//! Security module for MMC
//! Handles TLS, key exchange, certificate management, and encryption

pub mod cert;
pub mod crypto;
pub mod error;

pub use cert::{Certificate, CertificateStore};
pub use crypto::{Crypto, KeyPair};
pub use error::{Error, Result};
