//! TLS 1.3 握手框架
//!
//! 实现 MMC 设备间通信的 TLS 1.3 握手核心逻辑：
//! - X25519 ECDHE 密钥交换
//! - BLAKE3 派生握手密钥 (HKDF-style)
//! - ClientHello / ServerHello / Finished 消息
//! - 握手状态机
//!
//! 注意：这是 TLS 1.3 风格的握手框架，专注于协议逻辑而非底层密码学原语。
//! 在生产环境中，应使用成熟的 TLS 库（如 rustls）进行实际通信。

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Certificate, Crypto, Error, KeyPair, Result};

/// TLS 1.3 握手状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandshakeState {
    /// 初始状态
    Start,
    /// 已发送 ClientHello
    ClientHelloSent,
    /// 已收到 ServerHello
    ServerHelloReceived,
    /// 已发送 Finished
    FinishedSent,
    /// 握手完成
    Complete,
    /// 握手失败
    Failed,
}

impl Default for HandshakeState {
    fn default() -> Self {
        Self::Start
    }
}

/// TLS 1.3 密码套件
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum CipherSuite {
    /// TLS_AES_256_GCM_SHA384 (TLS 1.3 强制)
    TlsAes256GcmSha384 = 0x1301,
    /// TLS_CHACHA20_POLY1305_SHA256
    TlsChacha20Poly1305Sha256 = 0x1303,
    /// TLS_AES_128_GCM_SHA256
    TlsAes128GcmSha256 = 0x1302,
}

impl Default for CipherSuite {
    fn default() -> Self {
        Self::TlsAes256GcmSha384
    }
}

/// 握手模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandshakeMode {
    /// 客户端
    Client,
    /// 服务端
    Server,
}

/// ClientHello 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientHello {
    /// 协议版本 (TLS 1.3 = 0x0304)
    pub legacy_version: u16,
    /// 32 字节随机数
    pub random: [u8; 32],
    /// 会话 ID (用于恢复)
    pub session_id: Vec<u8>,
    /// 密码套件列表
    pub cipher_suites: Vec<CipherSuite>,
    /// 密钥共享 (X25519 公开密钥)
    pub key_share: [u8; 32],
    /// 设备证书
    pub certificate: Certificate,
}

/// ServerHello 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHello {
    /// 协议版本
    pub legacy_version: u16,
    /// 32 字节随机数
    pub random: [u8; 32],
    /// 会话 ID
    pub session_id: Vec<u8>,
    /// 协商的密码套件
    pub cipher_suite: CipherSuite,
    /// 服务器密钥共享
    pub key_share: [u8; 32],
    /// 服务器证书
    pub certificate: Certificate,
}

/// Finished 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finished {
    /// 验证数据 (BLAKE3 哈希)
    pub verify_data: [u8; 32],
    /// 时间戳
    pub timestamp_ms: u64,
}

/// TLS 1.3 握手会话
pub struct HandshakeSession {
    /// 握手模式
    mode: HandshakeMode,
    /// 当前状态
    state: HandshakeState,
    /// 本地 ECDHE 密钥对
    local_keypair: KeyPair,
    /// 本地证书
    local_cert: Certificate,
    /// 对端 ECDHE 公开密钥
    peer_public_key: Option<[u8; 32]>,
    /// 对端证书
    peer_cert: Option<Certificate>,
    /// 协商的密码套件
    cipher_suite: CipherSuite,
    /// 派生的握手密钥
    handshake_secret: Option<[u8; 32]>,
    /// 派生的主密钥
    master_secret: Option<[u8; 32]>,
    /// 客户端随机数
    client_random: Option<[u8; 32]>,
    /// 服务端随机数
    server_random: Option<[u8; 32]>,
}

impl HandshakeSession {
    /// 创建新的握手会话（客户端）
    pub fn new_client(local_cert: Certificate) -> Self {
        Self {
            mode: HandshakeMode::Client,
            state: HandshakeState::Start,
            local_keypair: Crypto::generate_keypair(),
            local_cert,
            peer_public_key: None,
            peer_cert: None,
            cipher_suite: CipherSuite::default(),
            handshake_secret: None,
            master_secret: None,
            client_random: None,
            server_random: None,
        }
    }

    /// 创建新的握手会话（服务端）
    pub fn new_server(local_cert: Certificate) -> Self {
        Self {
            mode: HandshakeMode::Server,
            state: HandshakeState::Start,
            local_keypair: Crypto::generate_keypair(),
            local_cert,
            peer_public_key: None,
            peer_cert: None,
            cipher_suite: CipherSuite::default(),
            handshake_secret: None,
            master_secret: None,
            client_random: None,
            server_random: None,
        }
    }

    /// 获取当前握手状态
    pub fn state(&self) -> HandshakeState {
        self.state
    }

    /// 获取本地公开密钥
    pub fn local_public_key(&self) -> [u8; 32] {
        self.local_keypair.public_key_bytes()
    }

    /// 计算与对端公开密钥的共享密钥 (调试用)
    pub fn compute_shared_secret(&self, peer_public: &[u8; 32]) -> Result<[u8; 32]> {
        self.local_keypair.shared_secret(peer_public)
    }

    /// 获取客户端随机数 (仅客户端有效)
    pub fn client_random_value(&self) -> Option<[u8; 32]> {
        self.client_random
    }

    /// 获取本地证书
    pub fn local_certificate(&self) -> &Certificate {
        &self.local_cert
    }

    /// 获取协商的密码套件
    pub fn cipher_suite(&self) -> CipherSuite {
        self.cipher_suite
    }

    /// 获取握手密钥
    pub fn handshake_secret(&self) -> Option<[u8; 32]> {
        self.handshake_secret
    }

    /// 获取主密钥
    pub fn master_secret(&self) -> Option<[u8; 32]> {
        self.master_secret
    }

    /// 客户端：创建 ClientHello
    pub fn create_client_hello(&mut self) -> Result<ClientHello> {
        if self.mode != HandshakeMode::Client {
            return Err(Error::TlsHandshake("Not a client".to_string()));
        }
        if self.state != HandshakeState::Start {
            return Err(Error::TlsHandshake(format!(
                "Invalid state for ClientHello: {:?}",
                self.state
            )));
        }

        let mut random = [0u8; 32];
        let random_bytes = Crypto::random_bytes(32);
        random.copy_from_slice(&random_bytes);

        self.client_random = Some(random);
        self.state = HandshakeState::ClientHelloSent;

        Ok(ClientHello {
            legacy_version: 0x0304, // TLS 1.3
            random,
            session_id: Vec::new(),
            cipher_suites: vec![
                CipherSuite::TlsAes256GcmSha384,
                CipherSuite::TlsChacha20Poly1305Sha256,
                CipherSuite::TlsAes128GcmSha256,
            ],
            key_share: self.local_keypair.public_key_bytes(),
            certificate: self.local_cert.clone(),
        })
    }

    /// 服务端：处理 ClientHello 并创建 ServerHello
    pub fn process_client_hello(&mut self, client_hello: &ClientHello) -> Result<ServerHello> {
        if self.mode != HandshakeMode::Server {
            return Err(Error::TlsHandshake("Not a server".to_string()));
        }
        if self.state != HandshakeState::Start {
            return Err(Error::TlsHandshake(format!(
                "Invalid state for processing ClientHello: {:?}",
                self.state
            )));
        }

        // 验证 legacy version
        if client_hello.legacy_version != 0x0304 {
            return Err(Error::InvalidTlsMessage(format!(
                "Unsupported version: 0x{:04x}",
                client_hello.legacy_version
            )));
        }

        // 选择密码套件
        let cipher_suite = *client_hello
            .cipher_suites
            .first()
            .ok_or_else(|| Error::InvalidTlsMessage("No cipher suites".to_string()))?;

        // 保存客户端信息
        self.peer_public_key = Some(client_hello.key_share);
        self.peer_cert = Some(client_hello.certificate.clone());
        self.cipher_suite = cipher_suite;
        self.client_random = Some(client_hello.random);

        // 生成服务端随机数
        let mut random = [0u8; 32];
        let random_bytes = Crypto::random_bytes(32);
        random.copy_from_slice(&random_bytes);
        self.server_random = Some(random);

        // 计算握手密钥 (ECDHE)
        let shared_secret = self
            .local_keypair
            .shared_secret(&client_hello.key_share)?;
        self.handshake_secret = Some(derive_handshake_secret(
            &shared_secret,
            &client_hello.random,
            &random,
        )?);

        // 派生主密钥
        self.master_secret = Some(derive_master_secret(
            self.handshake_secret.as_ref().unwrap(),
        )?);

        // 服务端处理完 ClientHello 后状态保持为 ServerHelloReceived
        // 直到它 create_finished 才会转为 FinishedSent
        self.state = HandshakeState::ServerHelloReceived;

        Ok(ServerHello {
            legacy_version: 0x0304,
            random,
            session_id: client_hello.session_id.clone(),
            cipher_suite,
            key_share: self.local_keypair.public_key_bytes(),
            certificate: self.local_cert.clone(),
        })
    }

    /// 客户端：处理 ServerHello
    pub fn process_server_hello(&mut self, server_hello: &ServerHello) -> Result<()> {
        if self.mode != HandshakeMode::Client {
            return Err(Error::TlsHandshake("Not a client".to_string()));
        }
        if self.state != HandshakeState::ClientHelloSent {
            return Err(Error::TlsHandshake(format!(
                "Invalid state for ServerHello: {:?}",
                self.state
            )));
        }

        // 验证 legacy version
        if server_hello.legacy_version != 0x0304 {
            return Err(Error::InvalidTlsMessage(format!(
                "Unsupported version: 0x{:04x}",
                server_hello.legacy_version
            )));
        }

        // 保存服务端信息
        self.peer_public_key = Some(server_hello.key_share);
        self.peer_cert = Some(server_hello.certificate.clone());
        self.cipher_suite = server_hello.cipher_suite;
        self.server_random = Some(server_hello.random);

        // 计算握手密钥 (ECDHE)
        let shared_secret = self
            .local_keypair
            .shared_secret(&server_hello.key_share)?;
        let client_random = self
            .client_random
            .ok_or_else(|| Error::TlsHandshake("Missing client random".to_string()))?;
        self.handshake_secret = Some(derive_handshake_secret(
            &shared_secret,
            &client_random,
            &server_hello.random,
        )?);

        // 派生主密钥
        self.master_secret = Some(derive_master_secret(
            self.handshake_secret.as_ref().unwrap(),
        )?);

        self.state = HandshakeState::ServerHelloReceived;
        Ok(())
    }

    /// 创建 Finished 消息
    pub fn create_finished(&mut self) -> Result<Finished> {
        if self.state != HandshakeState::ServerHelloReceived {
            return Err(Error::TlsHandshake(format!(
                "Invalid state for Finished: {:?}",
                self.state
            )));
        }

        let handshake_secret = self
            .handshake_secret
            .ok_or_else(|| Error::TlsHandshake("Missing handshake secret".to_string()))?;

        // 使用本地证书计算验证数据 (接收方将用对端证书=本端 cert 验证)
        let verify_data = compute_verify_data(&handshake_secret, &self.local_cert)?;

        self.state = HandshakeState::FinishedSent;
        Ok(Finished {
            verify_data,
            timestamp_ms: current_timestamp_ms(),
        })
    }

    /// 处理 Finished 消息并完成握手
    pub fn process_finished(&mut self, finished: &Finished) -> Result<()> {
        match self.mode {
            HandshakeMode::Client => {
                if self.state != HandshakeState::FinishedSent {
                    return Err(Error::TlsHandshake(format!(
                        "Invalid client state for Finished: {:?}",
                        self.state
                    )));
                }
            }
            HandshakeMode::Server => {
                // 服务端可能处于 ServerHelloReceived (未发送自己的 Finished) 或 FinishedSent
                if self.state != HandshakeState::ServerHelloReceived
                    && self.state != HandshakeState::FinishedSent
                {
                    return Err(Error::TlsHandshake(format!(
                        "Invalid server state for Finished: {:?}",
                        self.state
                    )));
                }
            }
        }

        let handshake_secret = self
            .handshake_secret
            .ok_or_else(|| Error::TlsHandshake("Missing handshake secret".to_string()))?;

        // 验证对端的验证数据: 用对端证书 (=发送方 local_cert) 计算期望
        let peer_cert = self
            .peer_cert
            .as_ref()
            .ok_or_else(|| Error::TlsHandshake("Missing peer cert".to_string()))?;
        let expected = compute_verify_data(&handshake_secret, peer_cert)?;
        if expected != finished.verify_data {
            self.state = HandshakeState::Failed;
            return Err(Error::InvalidTlsMessage(
                "Finished verify_data mismatch".to_string(),
            ));
        }

        self.state = HandshakeState::Complete;
        Ok(())
    }

    /// 握手是否完成
    pub fn is_complete(&self) -> bool {
        self.state == HandshakeState::Complete
    }

    /// 获取对端证书
    pub fn peer_certificate(&self) -> Option<&Certificate> {
        self.peer_cert.as_ref()
    }
}

/// 派生握手密钥 (HKDF-style 简化版)
fn derive_handshake_secret(
    shared_secret: &[u8; 32],
    client_random: &[u8; 32],
    server_random: &[u8; 32],
) -> Result<[u8; 32]> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"mmc tls 1.3 handshake");
    hasher.update(shared_secret);
    hasher.update(client_random);
    hasher.update(server_random);
    Ok(*hasher.finalize().as_bytes())
}

/// 派生主密钥
fn derive_master_secret(handshake_secret: &[u8; 32]) -> Result<[u8; 32]> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"mmc tls 1.3 master");
    hasher.update(handshake_secret);
    Ok(*hasher.finalize().as_bytes())
}

/// 计算验证数据 (verify_data)
fn compute_verify_data(handshake_secret: &[u8; 32], cert: &Certificate) -> Result<[u8; 32]> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"mmc tls 1.3 finished");
    hasher.update(handshake_secret);
    hasher.update(cert.device_id.as_bytes());
    hasher.update(cert.public_key.as_bytes());
    Ok(*hasher.finalize().as_bytes())
}

/// 派生应用数据密钥 (基于主密钥)
fn derive_application_key(master_secret: &[u8; 32], label: &[u8]) -> Result<[u8; 32]> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"mmc tls 1.3 app key");
    hasher.update(master_secret);
    hasher.update(label);
    Ok(*hasher.finalize().as_bytes())
}

/// 派生 IV (基于应用密钥)
fn derive_iv(app_key: &[u8; 32]) -> Result<[u8; 12]> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"mmc tls 1.3 iv");
    hasher.update(app_key);
    let hash = hasher.finalize();
    let bytes = hash.as_bytes();
    let mut iv = [0u8; 12];
    iv.copy_from_slice(&bytes[..12]);
    Ok(iv)
}

/// 派生 nonce (sequence number + IV)
fn derive_nonce(iv: &[u8; 12], seq: u64) -> [u8; 12] {
    let mut nonce = *iv;
    // XOR IV with sequence number (大端)
    for i in 0..8 {
        nonce[11 - i] ^= (seq >> (i * 8)) as u8;
    }
    nonce
}

/// 获取当前时间戳（毫秒）
fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// TLS 1.3 握手器 - 协调整个握手流程
pub struct TlsHandshake;

impl TlsHandshake {
    /// 执行客户端完整握手 (一次性接口)
    pub fn perform_client_handshake(
        client_cert: Certificate,
        server_cert: Certificate,
    ) -> Result<HandshakeSession> {
        let mut session = HandshakeSession::new_client(client_cert);

        // 1. ClientHello
        let _client_hello = session.create_client_hello()?;

        // 2. 模拟 ServerHello (实际使用中由服务端生成)
        let server_keypair = Crypto::generate_keypair();
        let shared_secret = server_keypair.shared_secret(&session.local_public_key())?;
        let client_random = session
            .client_random
            .ok_or_else(|| Error::TlsHandshake("Missing client random".to_string()))?;
        let mut server_random = [0u8; 32];
        server_random.copy_from_slice(&Crypto::random_bytes(32));

        let server_hello = ServerHello {
            legacy_version: 0x0304,
            random: server_random,
            session_id: Vec::new(),
            cipher_suite: CipherSuite::default(),
            key_share: server_keypair.public_key_bytes(),
            certificate: server_cert,
        };

        // 3. 处理 ServerHello
        session.process_server_hello(&server_hello)?;
        let _ = shared_secret;
        let _ = client_random;

        // 4. 创建 Finished
        let _finished = session.create_finished()?;

        // 5. 标记完成
        session.state = HandshakeState::Complete;
        Ok(session)
    }

    /// 执行服务端完整握手 (一次性接口)
    pub fn perform_server_handshake(
        server_cert: Certificate,
        client_cert: Certificate,
    ) -> Result<HandshakeSession> {
        let mut session = HandshakeSession::new_server(server_cert);

        // 1. 模拟 ClientHello (实际使用中由客户端生成)
        let client_keypair = Crypto::generate_keypair();
        let mut client_random = [0u8; 32];
        client_random.copy_from_slice(&Crypto::random_bytes(32));

        let client_hello = ClientHello {
            legacy_version: 0x0304,
            random: client_random,
            session_id: Vec::new(),
            cipher_suites: vec![CipherSuite::default()],
            key_share: client_keypair.public_key_bytes(),
            certificate: client_cert,
        };

        // 2. 处理 ClientHello 并生成 ServerHello
        let _server_hello = session.process_client_hello(&client_hello)?;

        // 3. 模拟接收 Finished (实际使用中由客户端发送)
        let handshake_secret = session
            .handshake_secret
            .ok_or_else(|| Error::TlsHandshake("Missing handshake secret".to_string()))?;
        let verify_data = compute_verify_data(&handshake_secret, &client_hello.certificate)?;
        let finished = Finished {
            verify_data,
            timestamp_ms: current_timestamp_ms(),
        };

        // 4. 处理 Finished
        session.process_finished(&finished)?;

        Ok(session)
    }
}

// ============================================================
// TLS 加密通信层 (基于 BLAKE3 + ChaCha20-Poly1305 风格的密钥流)
// ============================================================

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key};

/// TLS 加密记录 (16 字节 nonce + 16 字节 tag + ciphertext)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsRecord {
    /// 序列号 (用于防重放)
    pub sequence: u64,
    /// 密文 (含 16 字节 Poly1305 tag)
    pub ciphertext: Vec<u8>,
}

/// TLS 通信连接 - 在握手完成后提供加密通道
pub struct TlsConnection {
    /// 发送密钥 (32 字节)
    tx_key: [u8; 32],
    /// 接收密钥 (32 字节)
    rx_key: [u8; 32],
    /// 发送 IV
    tx_iv: [u8; 12],
    /// 接收 IV
    rx_iv: [u8; 12],
    /// 发送序列号
    tx_seq: u64,
    /// 接收序列号
    rx_seq: u64,
    /// 对端证书
    peer_cert: Certificate,
    /// 关联的握手会话 (用于获取元数据)
    handshake_complete: bool,
}

impl TlsConnection {
    /// 从已完成的握手会话创建 TLS 连接
    pub fn from_handshake(session: &HandshakeSession, is_client: bool) -> Result<Self> {
        if !session.is_complete() {
            return Err(Error::TlsHandshakeIncomplete);
        }

        let master_secret = session
            .master_secret()
            .ok_or_else(|| Error::TlsHandshake("Missing master secret".to_string()))?;

        let peer_cert = session
            .peer_certificate()
            .ok_or_else(|| Error::TlsHandshake("Missing peer certificate".to_string()))?
            .clone();

        // 根据角色派生不同的发送/接收密钥
        let (tx_key, rx_key) = if is_client {
            (
                derive_application_key(&master_secret, b"client tx key")?,
                derive_application_key(&master_secret, b"server tx key")?,
            )
        } else {
            (
                derive_application_key(&master_secret, b"server tx key")?,
                derive_application_key(&master_secret, b"client tx key")?,
            )
        };

        let tx_iv = derive_iv(&tx_key)?;
        let rx_iv = derive_iv(&rx_key)?;

        Ok(Self {
            tx_key,
            rx_key,
            tx_iv,
            rx_iv,
            tx_seq: 0,
            rx_seq: 0,
            peer_cert,
            handshake_complete: true,
        })
    }

    /// 加密数据 (发送)
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<TlsRecord> {
        if !self.handshake_complete {
            return Err(Error::TlsHandshakeIncomplete);
        }

        let nonce = derive_nonce(&self.tx_iv, self.tx_seq);
        let aad = self.tx_seq.to_be_bytes();

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.tx_key));
        let nonce_arr = chacha20poly1305::Nonce::from_slice(&nonce);

        let ciphertext = cipher
            .encrypt(
                nonce_arr,
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|e| Error::Crypto(format!("Encryption failed: {}", e)))?;

        let record = TlsRecord {
            sequence: self.tx_seq,
            ciphertext,
        };
        self.tx_seq += 1;
        Ok(record)
    }

    /// 解密数据 (接收)
    /// 使用 peer_key 解密 (来自对端的消息)
    pub fn decrypt(&mut self, record: &TlsRecord) -> Result<Vec<u8>> {
        if !self.handshake_complete {
            return Err(Error::TlsHandshakeIncomplete);
        }

        // 防重放：序列号必须单调递增
        if record.sequence != self.rx_seq {
            return Err(Error::InvalidTlsMessage(format!(
                "Out-of-order record: expected {}, got {}",
                self.rx_seq, record.sequence
            )));
        }

        let nonce = derive_nonce(&self.rx_iv, self.rx_seq);
        let aad = self.rx_seq.to_be_bytes();

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.rx_key));
        let nonce_arr = chacha20poly1305::Nonce::from_slice(&nonce);

        let plaintext = cipher
            .decrypt(
                nonce_arr,
                Payload {
                    msg: &record.ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|e| Error::Crypto(format!("Decryption failed: {}", e)))?;

        self.rx_seq += 1;
        Ok(plaintext)
    }

    /// 解密自己加密的消息 (用于测试/环回)
    pub fn decrypt_local(&mut self, record: &TlsRecord) -> Result<Vec<u8>> {
        if !self.handshake_complete {
            return Err(Error::TlsHandshakeIncomplete);
        }
        if record.sequence != self.rx_seq {
            return Err(Error::InvalidTlsMessage(format!(
                "Out-of-order record: expected {}, got {}",
                self.rx_seq, record.sequence
            )));
        }
        let nonce = derive_nonce(&self.tx_iv, self.rx_seq);
        let aad = self.rx_seq.to_be_bytes();
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&self.tx_key));
        let nonce_arr = chacha20poly1305::Nonce::from_slice(&nonce);
        let plaintext = cipher
            .decrypt(
                nonce_arr,
                Payload {
                    msg: &record.ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|e| Error::Crypto(format!("Decryption failed: {}", e)))?;
        self.rx_seq += 1;
        Ok(plaintext)
    }

    /// 获取发送序列号
    pub fn tx_sequence(&self) -> u64 {
        self.tx_seq
    }

    /// 获取接收序列号
    pub fn rx_sequence(&self) -> u64 {
        self.rx_seq
    }

    /// 获取对端证书
    pub fn peer_certificate(&self) -> &Certificate {
        &self.peer_cert
    }

    /// 握手是否完成
    pub fn is_ready(&self) -> bool {
        self.handshake_complete
    }

    /// 重置序列号 (用于会话恢复)
    pub fn reset_sequences(&mut self) {
        self.tx_seq = 0;
        self.rx_seq = 0;
    }
}

/// TLS 双向通道 (一端持有客户端连接，对端持有服务端连接)
pub struct TlsChannel {
    /// 本地连接
    local: TlsConnection,
    /// 远端序列号期望 (用于服务端验证)
    is_client: bool,
}

impl TlsChannel {
    /// 创建客户端通道
    pub fn new_client(client_conn: TlsConnection) -> Self {
        Self {
            local: client_conn,
            is_client: true,
        }
    }

    /// 创建服务端通道
    pub fn new_server(server_conn: TlsConnection) -> Self {
        Self {
            local: server_conn,
            is_client: false,
        }
    }

    /// 发送加密消息
    pub fn send(&mut self, data: &[u8]) -> Result<TlsRecord> {
        self.local.encrypt(data)
    }

    /// 接收并解密消息 (来自对端)
    pub fn receive(&mut self, record: &TlsRecord) -> Result<Vec<u8>> {
        self.local.decrypt(record)
    }

    /// 接收并解密消息 (环回/自己加密的消息)
    pub fn receive_local(&mut self, record: &TlsRecord) -> Result<Vec<u8>> {
        self.local.decrypt_local(record)
    }

    /// 双向加密传输 (发送 -> 接收验证)
    pub fn roundtrip(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        let record = self.send(data)?;
        self.receive(&record)
    }

    pub fn local(&self) -> &TlsConnection {
        &self.local
    }

    pub fn is_client(&self) -> bool {
        self.is_client
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_cert(device_id: &str) -> Certificate {
        let mut store = crate::CertificateStore::new();
        store
            .generate_identity(device_id, "Test Device")
            .unwrap()
            .clone()
    }

    #[test]
    fn test_handshake_state_default() {
        assert_eq!(HandshakeState::default(), HandshakeState::Start);
    }

    #[test]
    fn test_cipher_suite_default() {
        assert_eq!(CipherSuite::default(), CipherSuite::TlsAes256GcmSha384);
    }

    #[test]
    fn test_client_hello_creation() {
        let cert = make_test_cert("client-1");
        let mut session = HandshakeSession::new_client(cert);
        assert_eq!(session.state(), HandshakeState::Start);

        let hello = session.create_client_hello().unwrap();
        assert_eq!(hello.legacy_version, 0x0304);
        assert_eq!(hello.cipher_suites.len(), 3);
        assert_eq!(session.state(), HandshakeState::ClientHelloSent);
    }

    #[test]
    fn test_server_processes_client_hello() {
        let server_cert = make_test_cert("server-1");
        let client_cert = make_test_cert("client-1");

        let mut session = HandshakeSession::new_server(server_cert);
        let client_keypair = Crypto::generate_keypair();

        let client_hello = ClientHello {
            legacy_version: 0x0304,
            random: [1u8; 32],
            session_id: Vec::new(),
            cipher_suites: vec![CipherSuite::TlsAes256GcmSha384],
            key_share: client_keypair.public_key_bytes(),
            certificate: client_cert,
        };

        let server_hello = session.process_client_hello(&client_hello).unwrap();
        assert_eq!(server_hello.cipher_suite, CipherSuite::TlsAes256GcmSha384);
        // 服务端处理完 ClientHello 后状态为 ServerHelloReceived，等待 create_finished
        assert_eq!(session.state(), HandshakeState::ServerHelloReceived);
        assert!(session.handshake_secret().is_some());
        assert!(session.master_secret().is_some());
    }

    #[test]
    fn test_client_processes_server_hello() {
        let client_cert = make_test_cert("client-1");
        let server_cert = make_test_cert("server-1");

        let mut client = HandshakeSession::new_client(client_cert);
        client.create_client_hello().unwrap();

        let mut server = HandshakeSession::new_server(server_cert);
        let client_hello = ClientHello {
            legacy_version: 0x0304,
            random: [2u8; 32],
            session_id: Vec::new(),
            cipher_suites: vec![CipherSuite::TlsAes256GcmSha384],
            key_share: client.local_public_key(),
            certificate: client.local_certificate().clone(),
        };
        let server_hello = server.process_client_hello(&client_hello).unwrap();

        client.process_server_hello(&server_hello).unwrap();
        assert_eq!(client.state(), HandshakeState::ServerHelloReceived);
        assert!(client.handshake_secret().is_some());
    }

    #[test]
    fn test_full_handshake_via_helper() {
        let client_cert = make_test_cert("client-1");
        let server_cert = make_test_cert("server-1");

        let client_session =
            TlsHandshake::perform_client_handshake(client_cert.clone(), server_cert.clone())
                .unwrap();
        assert!(client_session.is_complete());

        let server_session =
            TlsHandshake::perform_server_handshake(server_cert, client_cert).unwrap();
        assert!(server_session.is_complete());
    }

    #[test]
    fn test_server_rejects_invalid_version() {
        let server_cert = make_test_cert("server-1");
        let client_cert = make_test_cert("client-1");

        let mut server = HandshakeSession::new_server(server_cert);
        let client_hello = ClientHello {
            legacy_version: 0x0303, // TLS 1.2, 应被拒绝
            random: [0u8; 32],
            session_id: Vec::new(),
            cipher_suites: vec![CipherSuite::TlsAes256GcmSha384],
            key_share: [0u8; 32],
            certificate: client_cert,
        };

        let result = server.process_client_hello(&client_hello);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_state_transition() {
        let cert = make_test_cert("client-1");
        let mut session = HandshakeSession::new_client(cert);

        // 直接尝试创建 Finished，应失败
        let result = session.create_finished();
        assert!(result.is_err());
    }

    #[test]
    fn test_finished_verify_data_mismatch() {
        let client_cert = make_test_cert("client-1");
        let server_cert = make_test_cert("server-1");

        let mut client = HandshakeSession::new_client(client_cert.clone());
        client.create_client_hello().unwrap();

        let mut server = HandshakeSession::new_server(server_cert);
        let client_hello = ClientHello {
            legacy_version: 0x0304,
            random: [3u8; 32],
            session_id: Vec::new(),
            cipher_suites: vec![CipherSuite::TlsAes256GcmSha384],
            key_share: client.local_public_key(),
            certificate: client_cert,
        };
        let _server_hello = server.process_client_hello(&client_hello).unwrap();

        // 创建伪造的 Finished (错误的 verify_data)
        let bad_finished = Finished {
            verify_data: [0u8; 32],
            timestamp_ms: 0,
        };

        let result = server.process_finished(&bad_finished);
        assert!(result.is_err());
        assert_eq!(server.state(), HandshakeState::Failed);
    }

    #[test]
    fn test_cipher_suite_values() {
        assert_eq!(CipherSuite::TlsAes256GcmSha384 as u16, 0x1301);
        assert_eq!(CipherSuite::TlsAes128GcmSha256 as u16, 0x1302);
        assert_eq!(CipherSuite::TlsChacha20Poly1305Sha256 as u16, 0x1303);
    }

    #[test]
    fn test_local_public_key_changes() {
        let cert1 = make_test_cert("device-1");
        let cert2 = make_test_cert("device-2");

        let s1 = HandshakeSession::new_client(cert1);
        let s2 = HandshakeSession::new_client(cert2);

        // 不同的会话应有不同的临时密钥对
        assert_ne!(s1.local_public_key(), s2.local_public_key());
    }

    // ============================================================
    // TLS 加密通信层测试
    // ============================================================

    fn make_completed_handshake() -> (HandshakeSession, HandshakeSession, Certificate, Certificate) {
        let client_cert = make_test_cert("client-1");
        let server_cert = make_test_cert("server-1");

        // 客户端
        let mut client = HandshakeSession::new_client(client_cert.clone());
        client.create_client_hello().unwrap();

        // 服务端
        let mut server = HandshakeSession::new_server(server_cert.clone());
        // 使用客户端真实的 client_random
        let client_random_value = client.client_random_value().unwrap();
        let client_hello = ClientHello {
            legacy_version: 0x0304,
            random: client_random_value,
            session_id: Vec::new(),
            cipher_suites: vec![CipherSuite::TlsAes256GcmSha384],
            key_share: client.local_public_key(),
            certificate: client.local_certificate().clone(),
        };
        let server_hello = server.process_client_hello(&client_hello).unwrap();
        client.process_server_hello(&server_hello).unwrap();

        // 验证两边 handshake_secret 一致
        assert_eq!(
            client.handshake_secret().unwrap(),
            server.handshake_secret().unwrap(),
            "Handshake secrets should match"
        );

        // 服务端创建自己的 Finished
        let server_finished = server.create_finished().unwrap();

        // 客户端创建自己的 Finished
        let client_finished = client.create_finished().unwrap();

        // 双方处理对方 Finished
        server.process_finished(&client_finished).unwrap();
        client.process_finished(&server_finished).unwrap();

        (client, server, client_cert, server_cert)
    }

    #[test]
    fn test_tls_connection_creation() {
        let (client, _, _, _) = make_completed_handshake();
        let conn = TlsConnection::from_handshake(&client, true).unwrap();
        assert!(conn.is_ready());
        assert_eq!(conn.tx_sequence(), 0);
        assert_eq!(conn.rx_sequence(), 0);
    }

    #[test]
    fn test_tls_connection_requires_complete_handshake() {
        let client_cert = make_test_cert("client-1");
        let session = HandshakeSession::new_client(client_cert);
        let result = TlsConnection::from_handshake(&session, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_tls_encrypt_decrypt_roundtrip() {
        let (client, _, _, _) = make_completed_handshake();
        let mut conn = TlsConnection::from_handshake(&client, true).unwrap();

        let plaintext = b"Hello, secure world!";
        let record = conn.encrypt(plaintext).unwrap();

        assert_eq!(record.sequence, 0);
        assert_ne!(record.ciphertext, plaintext);
        assert_eq!(conn.tx_sequence(), 1);

        // 单连接内自发自收 (环回)
        let decrypted = conn.decrypt_local(&record).unwrap();
        assert_eq!(decrypted, plaintext);
        assert_eq!(conn.rx_sequence(), 1);
    }

    #[test]
    fn test_tls_multiple_records() {
        let (client, _, _, _) = make_completed_handshake();
        let mut conn = TlsConnection::from_handshake(&client, true).unwrap();

        let messages = vec![b"first".to_vec(), b"second".to_vec(), b"third".to_vec()];

        let mut records = Vec::new();
        for msg in &messages {
            let r = conn.encrypt(msg).unwrap();
            records.push(r);
        }

        for (i, r) in records.iter().enumerate() {
            let decrypted = conn.decrypt_local(r).unwrap();
            assert_eq!(decrypted, messages[i]);
        }

        assert_eq!(conn.tx_sequence(), 3);
        assert_eq!(conn.rx_sequence(), 3);
    }

    #[test]
    fn test_tls_replay_attack_prevention() {
        let (client, _, _, _) = make_completed_handshake();
        let mut conn = TlsConnection::from_handshake(&client, true).unwrap();

        let record1 = conn.encrypt(b"msg1").unwrap();
        let _ = conn.decrypt_local(&record1).unwrap();

        // 尝试重放 record1 (序列号 0，但期望序列号是 1)
        let result = conn.decrypt_local(&record1);
        assert!(result.is_err());
    }

    #[test]
    fn test_tls_tampered_ciphertext_rejected() {
        let (client, _, _, _) = make_completed_handshake();
        let mut conn = TlsConnection::from_handshake(&client, true).unwrap();

        let mut record = conn.encrypt(b"original").unwrap();
        // 篡改密文
        if let Some(byte) = record.ciphertext.get_mut(0) {
            *byte ^= 0xFF;
        }

        let result = conn.decrypt_local(&record);
        assert!(result.is_err());
    }

    #[test]
    fn test_tls_channel_client() {
        let (client, _, _, _) = make_completed_handshake();
        let conn = TlsConnection::from_handshake(&client, true).unwrap();
        let mut channel = TlsChannel::new_client(conn);

        assert!(channel.is_client());
        let data = b"channel test";
        let record = channel.send(data).unwrap();
        let decrypted = channel.receive_local(&record).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_tls_channel_server() {
        let (_, server, _, _) = make_completed_handshake();
        let conn = TlsConnection::from_handshake(&server, false).unwrap();
        let mut channel = TlsChannel::new_server(conn);

        assert!(!channel.is_client());
        let data = b"server channel test";
        let record = channel.send(data).unwrap();
        let decrypted = channel.receive_local(&record).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_tls_bidirectional_communication() {
        // 模拟客户端和服务端之间加密通信
        let (client_session, server_session, _, _) = make_completed_handshake();

        let mut client_conn = TlsConnection::from_handshake(&client_session, true).unwrap();
        let mut server_conn = TlsConnection::from_handshake(&server_session, false).unwrap();

        // 客户端发送 -> 模拟服务端接收 (使用 client 的发送密钥 + server 的接收密钥应一致)
        let client_msg = b"Hello from client";
        let client_record = client_conn.encrypt(client_msg).unwrap();

        // 客户端的密文不能用客户端的 rx_key 解密（双向不同密钥）
        assert!(client_conn.decrypt(&client_record).is_err());

        // 服务端使用它的 rx_key (与 client 的 tx_key 对称) 解密客户端消息
        // 客户端 tx_key = "client tx key" 派生的密钥
        // 服务端 rx_key = "client tx key" 派生的密钥 (对称)
        let server_decrypted = server_conn.decrypt(&client_record);
        assert!(server_decrypted.is_ok(), "Server should decrypt client message");
        assert_eq!(server_decrypted.unwrap(), client_msg);
    }

    #[test]
    fn test_tls_sequence_increments() {
        let (client, _, _, _) = make_completed_handshake();
        let mut conn = TlsConnection::from_handshake(&client, true).unwrap();

        assert_eq!(conn.tx_sequence(), 0);
        conn.encrypt(b"a").unwrap();
        assert_eq!(conn.tx_sequence(), 1);
        conn.encrypt(b"b").unwrap();
        assert_eq!(conn.tx_sequence(), 2);
    }

    #[test]
    fn test_tls_reset_sequences() {
        let (client, _, _, _) = make_completed_handshake();
        let mut conn = TlsConnection::from_handshake(&client, true).unwrap();

        conn.encrypt(b"x").unwrap();
        conn.encrypt(b"y").unwrap();
        assert_eq!(conn.tx_sequence(), 2);

        conn.reset_sequences();
        assert_eq!(conn.tx_sequence(), 0);
        assert_eq!(conn.rx_sequence(), 0);
    }

    #[test]
    fn test_tls_peer_certificate_access() {
        let (client, _, _, _) = make_completed_handshake();
        let conn = TlsConnection::from_handshake(&client, true).unwrap();
        let peer = conn.peer_certificate();
        assert_eq!(peer.device_id, "server-1");
    }

    #[test]
    fn test_tls_record_serialization() {
        let (client, _, _, _) = make_completed_handshake();
        let mut conn = TlsConnection::from_handshake(&client, true).unwrap();

        let record = conn.encrypt(b"serialize me").unwrap();
        let json = serde_json::to_vec(&record).unwrap();
        let decoded: TlsRecord = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded.sequence, record.sequence);
        assert_eq!(decoded.ciphertext, record.ciphertext);
    }

    #[test]
    fn test_tls_empty_plaintext() {
        let (client, _, _, _) = make_completed_handshake();
        let mut conn = TlsConnection::from_handshake(&client, true).unwrap();

        let record = conn.encrypt(b"").unwrap();
        // 即使明文为空，密文也包含 16 字节 tag
        assert!(record.ciphertext.len() >= 16);
        let decrypted = conn.decrypt_local(&record).unwrap();
        assert!(decrypted.is_empty());
    }

    #[test]
    fn test_tls_large_message() {
        let (client, _, _, _) = make_completed_handshake();
        let mut conn = TlsConnection::from_handshake(&client, true).unwrap();

        let large_data = vec![0xABu8; 65536]; // 64KB
        let record = conn.encrypt(&large_data).unwrap();
        let decrypted = conn.decrypt_local(&record).unwrap();
        assert_eq!(decrypted, large_data);
    }

    #[test]
    fn test_derive_nonce_uniqueness() {
        // 使用零 IV 验证 nonce 派生
        let iv = [0u8; 12];
        let n0 = derive_nonce(&iv, 0);
        let n1 = derive_nonce(&iv, 1);
        let n2 = derive_nonce(&iv, 2);

        assert_ne!(n0, n1);
        assert_ne!(n1, n2);
        assert_ne!(n0, n2);
    }

    #[test]
    fn test_derive_application_key_different_labels() {
        let master = [42u8; 32];
        let k1 = derive_application_key(&master, b"client tx key").unwrap();
        let k2 = derive_application_key(&master, b"server tx key").unwrap();
        assert_ne!(k1, k2);
    }
}
