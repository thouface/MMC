//! Security module for MMC
//! Handles TLS, key exchange, certificate management, and encryption

pub mod cert;
pub mod crypto;
pub mod error;
pub mod tls;

pub use cert::{Certificate, CertificateStore};
pub use crypto::{Crypto, KeyPair};
pub use error::{Error, Result};
pub use tls::{
    CipherSuite, ClientHello, Finished, HandshakeMode, HandshakeSession, HandshakeState,
    ServerHello, TlsHandshake,
};
