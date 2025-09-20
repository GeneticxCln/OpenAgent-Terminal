//! Secure sync implementation with per-install encryption
//!
//! This module provides secure synchronization with:
//! - Per-install random salt generation
//! - Proper KDF parameters stored with payload
//! - Authenticated peer discovery
//! - Signed and encrypted sync messages

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

use ring::rand::{SecureRandom, SystemRandom};
use ring::{aead, hkdf, pbkdf2};
use ring::{digest, signature, signature::KeyPair};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{SyncConfig, SyncError, SyncProvider, SyncScope, SyncStatus};

/// KDF parameters for key derivation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    /// Algorithm identifier (e.g., "PBKDF2-SHA256", "Argon2id")
    pub algorithm: String,
    /// Random salt unique to this installation
    pub salt: Vec<u8>,
    /// Number of iterations (for PBKDF2) or memory cost (for Argon2)
    pub iterations: u32,
    /// Memory cost in KB (for Argon2)
    pub memory_cost: Option<u32>,
    /// Parallelism factor (for Argon2)
    pub parallelism: Option<u32>,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            algorithm: "PBKDF2-SHA256".to_string(),
            salt: Vec::new(),
            iterations: 100_000, // OWASP recommended minimum for PBKDF2-SHA256
            memory_cost: None,
            parallelism: None,
        }
    }
}

/// Installation-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationMetadata {
    /// Unique installation ID
    pub installation_id: String,
    /// Creation timestamp
    pub created_at: u64,
    /// KDF parameters for this installation
    pub kdf_params: KdfParams,
    /// Version of the sync protocol
    pub sync_version: String,
    /// Public key for peer authentication (placeholder)
    pub public_key: Option<Vec<u8>>,
}

/// Encrypted sync payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    /// KDF parameters used for encryption
    pub kdf_params: KdfParams,
    /// Encrypted data
    pub ciphertext: Vec<u8>,
    /// Authentication tag
    pub auth_tag: Vec<u8>,
    /// Nonce/IV for encryption
    pub nonce: Vec<u8>,
    /// Installation ID of sender
    pub sender_id: String,
    /// Timestamp of encryption
    pub timestamp: u64,
    /// Sync scope this payload contains
    pub scope: String,
}

/// Peer discovery information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer installation ID
    pub installation_id: String,
    /// Display name for this peer
    pub display_name: String,
    /// Last seen timestamp
    pub last_seen: u64,
    /// Public key for authentication (raw Ed25519 bytes)
    pub public_key: Vec<u8>,
    /// Sync capabilities
    pub capabilities: Vec<String>,
}

/// A single key record for rotation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyHistoryEntry {
    pub public_key: Vec<u8>,
    pub key_fingerprint: String,
    pub valid_from: u64,
    pub valid_to: u64,
}

/// Persistent peer record with trust metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRecord {
    pub info: PeerInfo,
    pub key_fingerprint: String,
    pub revoked: bool,
    pub key_history: Vec<KeyHistoryEntry>,
}

/// Versioned trust store signed by the local installation key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustStore {
    pub version: u32,
    pub peers: HashMap<String, PeerRecord>,
    pub signed_by: String,
    pub signed_at: u64,
    pub signature: Vec<u8>,
}

/// Signable view of the trust store for canonical signing
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustStoreSignable {
    version: u32,
    signed_by: String,
    signed_at: u64,
    peers: Vec<(String, PeerRecord)>,
}

/// A handshake challenge sent to a peer for authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeChallenge {
    pub from_installation_id: String,
    pub challenge: Vec<u8>, // 32 random bytes
    pub timestamp: u64,
}

/// A handshake response proving control of the private key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub responder_installation_id: String,
    pub challenge: Vec<u8>,
    pub signature: Vec<u8>, // Ed25519 signature over domain|from|to|challenge
}

/// Secure sync provider with per-install encryption
pub struct SecureSyncProvider {
    /// Base directory for sync data
    base_dir: PathBuf,
    /// Installation metadata
    metadata: InstallationMetadata,
    /// Secure random number generator
    rng: SystemRandom,
    /// Versioned trust store of peers
    trust_store: TrustStore,
    /// Ed25519 keypair for signing (held in memory)
    key_pair: signature::Ed25519KeyPair,
    /// Optional env var name for encryption key at rest
    encryption_key_env: Option<String>,
}

impl SecureSyncProvider {
    /// Create a new secure sync provider
    pub fn new(config: &SyncConfig) -> Result<Self, SyncError> {
        let base_dir = Self::get_base_dir(config)?;

        // Ensure the base directory exists
        fs::create_dir_all(&base_dir)?;

        let rng = SystemRandom::new();
        let (metadata, key_pair) = Self::load_or_create_metadata_and_keys(&base_dir, &rng)?;
        let trust_store = Self::load_trust_store(&base_dir, &metadata, &key_pair)?;

        Ok(Self {
            base_dir,
            metadata,
            rng,
            trust_store,
            key_pair,
            encryption_key_env: config.encryption_key_env.clone(),
        })
    }

    /// Get the base directory for sync data
    fn get_base_dir(config: &SyncConfig) -> Result<PathBuf, SyncError> {
        let base_dir = if let Some(ref dir) = config.data_dir {
            dir.clone()
        } else {
            let state_dir =
                std::env::var("XDG_STATE_HOME").map(PathBuf::from).unwrap_or_else(|_| {
                    let home = std::env::var("HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(|_| PathBuf::from("."));
                    home.join(".local").join("state")
                });
            state_dir.join("openagent-terminal").join("secure-sync")
        };

        Ok(base_dir)
    }

    /// Load existing metadata and keys or create new installation
    fn load_or_create_metadata_and_keys(
        base_dir: &Path,
        rng: &SystemRandom,
    ) -> Result<(InstallationMetadata, signature::Ed25519KeyPair), SyncError> {
        let metadata_file = base_dir.join("installation.json");

        // Ensure keys directory exists
        let keys_dir = base_dir.join("keys");
        fs::create_dir_all(&keys_dir)?;

        let private_key_path = keys_dir.join("ed25519_private.pk8");
        let public_key_path = keys_dir.join("ed25519_public.bin");

        let mut metadata: InstallationMetadata;

        if metadata_file.exists() {
            let content = fs::read_to_string(&metadata_file)?;
            metadata = serde_json::from_str(&content)
                .map_err(|e| SyncError::Other(format!("Failed to parse metadata: {}", e)))?;

            // Validate metadata
            if metadata.kdf_params.salt.is_empty() {
                return Err(SyncError::Misconfigured("Invalid KDF salt in metadata"));
            }
        } else {
            // Create new installation metadata with random salt
            let installation_id = Uuid::new_v4().to_string();

            // Generate random salt
            let mut salt = vec![0u8; 32];
            rng.fill(&mut salt)
                .map_err(|_| SyncError::Other("Failed to generate random salt".to_string()))?;

            let kdf_params = KdfParams {
                algorithm: "PBKDF2-SHA256".to_string(),
                salt,
                iterations: 100_000,
                memory_cost: None,
                parallelism: None,
            };

            metadata = InstallationMetadata {
                installation_id,
                created_at: Self::current_timestamp(),
                kdf_params,
                sync_version: "1.0.0".to_string(),
                public_key: None,
            };
        }

        // Load or generate keypair
        let key_pair = if private_key_path.exists() {
            let pkcs8_bytes = fs::read(&private_key_path)?;
            let kp = signature::Ed25519KeyPair::from_pkcs8(&pkcs8_bytes)
                .map_err(|_| SyncError::Other("Failed to parse private key".to_string()))?;

            // Ensure public key file exists and metadata has public key
            let public_key = kp.public_key().as_ref().to_vec();
            if !public_key_path.exists() {
                fs::write(&public_key_path, &public_key)?;
            }
            if metadata.public_key.as_deref() != Some(public_key.as_slice()) {
                metadata.public_key = Some(public_key);
            }

            kp
        } else {
            // Generate new keypair
            let pkcs8 = signature::Ed25519KeyPair::generate_pkcs8(rng)
                .map_err(|_| SyncError::Other("Failed to generate keypair".to_string()))?;
            // Persist private key with restrictive permissions
            Self::write_private_key_secure(&private_key_path, pkcs8.as_ref())?;

            let kp = signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
                .map_err(|_| SyncError::Other("Failed to parse generated keypair".to_string()))?;

            // Persist public key
            let public_key = kp.public_key().as_ref().to_vec();
            fs::write(&public_key_path, &public_key)?;
            metadata.public_key = Some(public_key);

            kp
        };

        // Save metadata (in case we updated public_key or created new)
        let json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| SyncError::Other(format!("Failed to serialize metadata: {}", e)))?;
        fs::write(&metadata_file, json)?;

        Ok((metadata, key_pair))
    }

    /// Load the versioned trust store, migrating from legacy peers.json if needed
    fn load_trust_store(
        base_dir: &Path,
        metadata: &InstallationMetadata,
        key_pair: &signature::Ed25519KeyPair,
    ) -> Result<TrustStore, SyncError> {
        let peers_file = base_dir.join("peers.json");

        if peers_file.exists() {
            let content = fs::read_to_string(&peers_file)?;

            // Try current TrustStore format
            if let Ok(store) = serde_json::from_str::<TrustStore>(&content) {
                // Verify signature if signed_by matches this installation
                if store.signed_by == metadata.installation_id {
                    let bytes = Self::canonical_trust_store_bytes(
                        store.version,
                        &store.signed_by,
                        store.signed_at,
                        &store.peers,
                    )?;
                    let verifier = signature::UnparsedPublicKey::new(
                        &signature::ED25519,
                        metadata.public_key.as_ref().ok_or_else(|| {
                            SyncError::Other("Missing public key in metadata".to_string())
                        })?,
                    );
                    if verifier.verify(&bytes, &store.signature).is_err() {
                        return Err(SyncError::Other("Trust store signature invalid".to_string()));
                    }
                } else {
                    warn!("Trust store signed_by does not match this installation; skipping signature verification");
                }
                return Ok(store);
            }

            // Try legacy format and migrate
            if let Ok(legacy_peers) = serde_json::from_str::<HashMap<String, PeerInfo>>(&content) {
                let mut peers: HashMap<String, PeerRecord> = HashMap::new();
                for (id, info) in legacy_peers {
                    let fingerprint = Self::fingerprint_key(&info.public_key);
                    let record = PeerRecord {
                        info,
                        key_fingerprint: fingerprint.clone(),
                        revoked: false,
                        key_history: Vec::new(),
                    };
                    peers.insert(id, record);
                }
                let mut store = TrustStore {
                    version: 1,
                    peers,
                    signed_by: metadata.installation_id.clone(),
                    signed_at: Self::current_timestamp(),
                    signature: Vec::new(),
                };
                // Sign and save migrated store
                store.signature = Self::sign_trust_store(&store, key_pair)?;
                let json = serde_json::to_string_pretty(&store).map_err(|e| {
                    SyncError::Other(format!("Failed to serialize trust store: {}", e))
                })?;
                fs::write(&peers_file, json)?;
                return Ok(store);
            }

            // Unknown format
            return Err(SyncError::Other("Failed to parse trust store".to_string()));
        }

        // New store
        let mut store = TrustStore {
            version: 1,
            peers: HashMap::new(),
            signed_by: metadata.installation_id.clone(),
            signed_at: Self::current_timestamp(),
            signature: Vec::new(),
        };
        store.signature = Self::sign_trust_store(&store, key_pair)?;
        let json = serde_json::to_string_pretty(&store)
            .map_err(|e| SyncError::Other(format!("Failed to serialize trust store: {}", e)))?;
        fs::write(peers_file, json)?;
        Ok(store)
    }

    /// Save trust store to disk with fresh signature
    fn save_trust_store(&mut self) -> Result<(), SyncError> {
        self.trust_store.signed_by = self.metadata.installation_id.clone();
        self.trust_store.signed_at = Self::current_timestamp();
        self.trust_store.signature = Self::sign_trust_store(&self.trust_store, &self.key_pair)?;

        let peers_file = self.base_dir.join("peers.json");
        let json = serde_json::to_string_pretty(&self.trust_store)
            .map_err(|e| SyncError::Other(format!("Failed to serialize trust store: {}", e)))?;
        fs::write(peers_file, json)?;
        Ok(())
    }

    /// Compute canonical bytes for signing the trust store
    fn canonical_trust_store_bytes(
        version: u32,
        signed_by: &str,
        signed_at: u64,
        peers: &HashMap<String, PeerRecord>,
    ) -> Result<Vec<u8>, SyncError> {
        let mut items: Vec<(String, PeerRecord)> =
            peers.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        items.sort_by(|a, b| a.0.cmp(&b.0));
        let signable = TrustStoreSignable {
            version,
            signed_by: signed_by.to_string(),
            signed_at,
            peers: items,
        };
        let mut data = serde_json::to_vec(&signable).map_err(|e| {
            SyncError::Other(format!("Failed to serialize trust store (signable): {}", e))
        })?;
        // Domain separation prefix
        let mut prefixed = b"openagent-terminal.secure-sync.truststore.v1|".to_vec();
        prefixed.append(&mut data);
        Ok(prefixed)
    }

    /// Sign the trust store
    fn sign_trust_store(
        store: &TrustStore,
        key_pair: &signature::Ed25519KeyPair,
    ) -> Result<Vec<u8>, SyncError> {
        let bytes = Self::canonical_trust_store_bytes(
            store.version,
            &store.signed_by,
            store.signed_at,
            &store.peers,
        )?;
        Ok(key_pair.sign(&bytes).as_ref().to_vec())
    }

    /// Compute a hex fingerprint of a public key (SHA-256)
    fn fingerprint_key(public_key: &[u8]) -> String {
        let digest = digest::digest(&digest::SHA256, public_key);
        let bytes = digest.as_ref();
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            use std::fmt::Write as _;
            let _ = write!(&mut s, "{:02x}", b);
        }
        s
    }

    /// Derive encryption key from password using installation-specific KDF params
    fn derive_key(&self, password: &[u8]) -> Result<[u8; 32], SyncError> {
        let mut key = [0u8; 32];

        match self.metadata.kdf_params.algorithm.as_str() {
            "PBKDF2-SHA256" => {
                pbkdf2::derive(
                    pbkdf2::PBKDF2_HMAC_SHA256,
                    std::num::NonZeroU32::new(self.metadata.kdf_params.iterations)
                        .ok_or(SyncError::Misconfigured("Invalid iteration count"))?,
                    &self.metadata.kdf_params.salt,
                    password,
                    &mut key,
                );
                Ok(key)
            }
            _ => Err(SyncError::Other(format!(
                "Unsupported KDF algorithm: {}",
                self.metadata.kdf_params.algorithm
            ))),
        }
    }

    /// Write private key to disk with restrictive permissions
    fn write_private_key_secure(path: &Path, bytes: &[u8]) -> Result<(), SyncError> {
        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(path)?;
            use std::io::Write as _;
            file.write_all(bytes)?;
            file.sync_all()?;
            Ok(())
        }
        #[cfg(not(unix))]
        {
            fs::write(path, bytes)?;
            return Ok(());
        }
    }

    /// Build the domain-separated bytes for signing/verification
    fn signing_bytes(domain: &str, from_id: &str, to_id: &str, challenge: &[u8]) -> Vec<u8> {
        let mut data =
            Vec::with_capacity(domain.len() + from_id.len() + to_id.len() + challenge.len() + 3);
        data.extend_from_slice(domain.as_bytes());
        data.push(b'|');
        data.extend_from_slice(from_id.as_bytes());
        data.push(b'|');
        data.extend_from_slice(to_id.as_bytes());
        data.push(b'|');
        data.extend_from_slice(challenge);
        data
    }

    /// Create an authentication challenge for a peer
    pub fn create_handshake_challenge(
        &self,
        _to_installation_id: &str,
    ) -> Result<HandshakeChallenge, SyncError> {
        let mut challenge = vec![0u8; 32];
        self.rng
            .fill(&mut challenge)
            .map_err(|_| SyncError::Other("Failed to generate challenge".to_string()))?;
        Ok(HandshakeChallenge {
            from_installation_id: self.metadata.installation_id.clone(),
            challenge,
            timestamp: Self::current_timestamp(),
        })
    }

    /// Create a signature over the given challenge for handshake response
    pub fn respond_to_handshake(
        &self,
        from_installation_id: &str,
        challenge: &[u8],
    ) -> Result<HandshakeResponse, SyncError> {
        const DOMAIN: &str = "openagent-terminal.secure-sync.handshake.v1";
        let bytes = Self::signing_bytes(
            DOMAIN,
            &self.metadata.installation_id,
            from_installation_id,
            challenge,
        );
        let sig = self.key_pair.sign(&bytes);
        Ok(HandshakeResponse {
            responder_installation_id: self.metadata.installation_id.clone(),
            challenge: challenge.to_vec(),
            signature: sig.as_ref().to_vec(),
        })
    }

    /// Verify a handshake response from a peer
    pub fn verify_handshake_response(
        &self,
        peer: &PeerInfo,
        from_installation_id: &str,
        response: &HandshakeResponse,
    ) -> Result<bool, SyncError> {
        const DOMAIN: &str = "openagent-terminal.secure-sync.handshake.v1";
        if response.challenge.is_empty() {
            return Ok(false);
        }
        // response.responder_installation_id must match the peer record
        if response.responder_installation_id != peer.installation_id {
            return Ok(false);
        }
        let bytes = Self::signing_bytes(
            DOMAIN,
            &peer.installation_id,
            from_installation_id,
            &response.challenge,
        );
        let verifier = signature::UnparsedPublicKey::new(&signature::ED25519, &peer.public_key);
        let ok = verifier.verify(&bytes, &response.signature).is_ok();
        Ok(ok)
    }

    /// Sign an arbitrary message with domain separation
    pub fn sign_message(&self, domain: &str, message: &[u8]) -> Vec<u8> {
        let mut data = Vec::with_capacity(domain.len() + 1 + message.len());
        data.extend_from_slice(domain.as_bytes());
        data.push(b'|');
        data.extend_from_slice(message);
        self.key_pair.sign(&data).as_ref().to_vec()
    }

    /// Verify a signature over an arbitrary message with domain separation
    pub fn verify_signature(
        public_key: &[u8],
        domain: &str,
        message: &[u8],
        signature_bytes: &[u8],
    ) -> bool {
        let mut data = Vec::with_capacity(domain.len() + 1 + message.len());
        data.extend_from_slice(domain.as_bytes());
        data.push(b'|');
        data.extend_from_slice(message);
        let verifier = signature::UnparsedPublicKey::new(&signature::ED25519, public_key);
        verifier.verify(&data, signature_bytes).is_ok()
    }

    /// Encrypt data for sync
    fn encrypt_data(
        &self,
        data: &[u8],
        password: &[u8],
        scope: SyncScope,
    ) -> Result<EncryptedPayload, SyncError> {
        let key = self.derive_key(password)?;

        // Use HKDF to derive encryption key from master key
        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, &[]);
        let prk = salt.extract(&key);
        let okm = prk
            .expand(&[b"sync-encryption"], hkdf::HKDF_SHA256)
            .map_err(|_| SyncError::Other("HKDF expand failed".to_string()))?;

        let mut encryption_key = [0u8; 32];
        okm.fill(&mut encryption_key)
            .map_err(|_| SyncError::Other("HKDF fill failed".to_string()))?;

        // Generate nonce
        let mut nonce = [0u8; 12];
        self.rng
            .fill(&mut nonce)
            .map_err(|_| SyncError::Other("Failed to generate nonce".to_string()))?;

        // Encrypt using AES-256-GCM (ring 0.17 API)
        let unbound = aead::UnboundKey::new(&aead::AES_256_GCM, &encryption_key)
            .map_err(|_| SyncError::Other("Failed to create AEAD key".to_string()))?;
        let key = aead::LessSafeKey::new(unbound);

        let mut ciphertext_and_tag = data.to_vec();
        key.seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::empty(),
            &mut ciphertext_and_tag,
        )
        .map_err(|_| SyncError::Other("Encryption failed".to_string()))?;

        // Split ciphertext and tag
        let ciphertext_len = ciphertext_and_tag.len() - aead::AES_256_GCM.tag_len();
        let ciphertext = ciphertext_and_tag[..ciphertext_len].to_vec();
        let auth_tag = ciphertext_and_tag[ciphertext_len..].to_vec();

        Ok(EncryptedPayload {
            kdf_params: self.metadata.kdf_params.clone(),
            ciphertext,
            auth_tag,
            nonce: nonce.to_vec(),
            sender_id: self.metadata.installation_id.clone(),
            timestamp: Self::current_timestamp(),
            scope: format!("{:?}", scope),
        })
    }

    /// Decrypt sync data
    fn decrypt_data(
        &self,
        payload: &EncryptedPayload,
        password: &[u8],
    ) -> Result<Vec<u8>, SyncError> {
        // Validate KDF params compatibility
        if payload.kdf_params.algorithm != self.metadata.kdf_params.algorithm {
            return Err(SyncError::Other("KDF algorithm mismatch".to_string()));
        }

        // Derive key using payload's KDF params
        let mut key = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(payload.kdf_params.iterations)
                .ok_or(SyncError::Misconfigured("Invalid iteration count"))?,
            &payload.kdf_params.salt,
            password,
            &mut key,
        );

        // Use HKDF to derive decryption key
        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, &[]);
        let prk = salt.extract(&key);
        let okm = prk
            .expand(&[b"sync-encryption"], hkdf::HKDF_SHA256)
            .map_err(|_| SyncError::Other("HKDF expand failed".to_string()))?;

        let mut decryption_key = [0u8; 32];
        okm.fill(&mut decryption_key)
            .map_err(|_| SyncError::Other("HKDF fill failed".to_string()))?;

        // Prepare nonce
        if payload.nonce.len() != 12 {
            return Err(SyncError::Other("Invalid nonce length".to_string()));
        }
        let mut nonce_array = [0u8; 12];
        nonce_array.copy_from_slice(&payload.nonce);

        // Decrypt using AES-256-GCM (ring 0.17 API)
        let unbound = aead::UnboundKey::new(&aead::AES_256_GCM, &decryption_key)
            .map_err(|_| SyncError::Other("Failed to create AEAD key".to_string()))?;
        let key = aead::LessSafeKey::new(unbound);

        // Combine ciphertext and tag
        let mut ciphertext_and_tag = payload.ciphertext.clone();
        ciphertext_and_tag.extend_from_slice(&payload.auth_tag);

        let plaintext = key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce_array),
                aead::Aad::empty(),
                &mut ciphertext_and_tag,
            )
            .map_err(|_| SyncError::Other("Decryption failed".to_string()))?;

        Ok(plaintext.to_vec())
    }

    /// Get the current Unix timestamp
    fn current_timestamp() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    }

    /// Get the encrypted data directory for a scope
    fn encrypted_data_dir(&self, scope: SyncScope) -> PathBuf {
        self.base_dir.join("encrypted").join(match scope {
            SyncScope::Settings => "settings",
            SyncScope::History => "history",
        })
    }

    /// Get sync status file path
    fn status_file(&self) -> PathBuf {
        self.base_dir.join("sync_status.json")
    }

    /// Read sync status
    fn read_status(&self) -> Result<SyncStatus, SyncError> {
        let status_path = self.status_file();

        if !status_path.exists() {
            return Ok(SyncStatus::default());
        }

        let content = fs::read_to_string(&status_path)?;
        let status: SyncStatus = serde_json::from_str(&content)
            .map_err(|e| SyncError::Other(format!("Failed to parse status: {}", e)))?;

        Ok(status)
    }

    /// Write sync status
    fn write_status(&self, status: &SyncStatus) -> Result<(), SyncError> {
        let status_path = self.status_file();
        let json = serde_json::to_string_pretty(status)
            .map_err(|e| SyncError::Other(format!("Failed to serialize status: {}", e)))?;
        fs::write(status_path, json)?;
        Ok(())
    }

    /// Get source directory for a sync scope
    fn source_dir(&self, scope: SyncScope) -> PathBuf {
        match scope {
            SyncScope::Settings => {
                let config_dir =
                    std::env::var("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|_| {
                        let home = std::env::var("HOME")
                            .map(PathBuf::from)
                            .unwrap_or_else(|_| PathBuf::from("."));
                        home.join(".config")
                    });
                config_dir.join("openagent-terminal")
            }
            SyncScope::History => {
                let data_dir =
                    std::env::var("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|_| {
                        let home = std::env::var("HOME")
                            .map(PathBuf::from)
                            .unwrap_or_else(|_| PathBuf::from("."));
                        home.join(".local").join("share")
                    });
                data_dir.join("openagent-terminal")
            }
        }
    }

    /// Add a known peer for authenticated sync (trusted by default)
    pub fn add_peer(&mut self, mut peer: PeerInfo) -> Result<(), SyncError> {
        // Update last_seen on add
        peer.last_seen = Self::current_timestamp();
        let fingerprint = Self::fingerprint_key(&peer.public_key);
        let record = PeerRecord {
            info: peer,
            key_fingerprint: fingerprint,
            revoked: false,
            key_history: Vec::new(),
        };
        self.trust_store.peers.insert(record.info.installation_id.clone(), record);
        self.save_trust_store()
    }

    /// Remove a peer entirely from the trust store
    pub fn remove_peer(&mut self, installation_id: &str) -> Result<bool, SyncError> {
        let removed = self.trust_store.peers.remove(installation_id).is_some();
        if removed {
            self.save_trust_store()?;
        }
        Ok(removed)
    }

    /// Revoke a peer (kept in trust store but marked untrusted)
    pub fn revoke_peer(&mut self, installation_id: &str) -> Result<bool, SyncError> {
        if let Some(record) = self.trust_store.peers.get_mut(installation_id) {
            record.revoked = true;
            self.save_trust_store()?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Rotate a peer's public key (append old to history)
    pub fn rotate_peer_key(
        &mut self,
        installation_id: &str,
        new_public_key: Vec<u8>,
    ) -> Result<bool, SyncError> {
        if let Some(record) = self.trust_store.peers.get_mut(installation_id) {
            let now = Self::current_timestamp();
            // Push current key to history
            let old_entry = KeyHistoryEntry {
                public_key: record.info.public_key.clone(),
                key_fingerprint: record.key_fingerprint.clone(),
                valid_from: 0, // unknown, legacy
                valid_to: now,
            };
            record.key_history.push(old_entry);
            // Update to new key
            record.info.public_key = new_public_key;
            record.key_fingerprint = Self::fingerprint_key(&record.info.public_key);
            record.info.last_seen = now;
            self.save_trust_store()?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Get a peer by installation id (only non-revoked)
    pub fn get_peer(&self, installation_id: &str) -> Option<&PeerInfo> {
        self.trust_store.peers.get(installation_id).filter(|r| !r.revoked).map(|r| &r.info)
    }

    /// List all known, non-revoked peers
    pub fn list_peers(&self) -> Vec<&PeerInfo> {
        self.trust_store.peers.values().filter(|r| !r.revoked).map(|r| &r.info).collect()
    }

    /// Get full peer record (including revoked and history)
    pub fn get_peer_record(&self, installation_id: &str) -> Option<&PeerRecord> {
        self.trust_store.peers.get(installation_id)
    }

    /// List all peer records (including revoked)
    pub fn list_peer_records(&self) -> Vec<&PeerRecord> {
        self.trust_store.peers.values().collect()
    }

    /// Get installation metadata
    pub fn installation_metadata(&self) -> &InstallationMetadata {
        &self.metadata
    }
}

impl SyncProvider for SecureSyncProvider {
    fn name(&self) -> &'static str {
        "secure"
    }

    fn status(&self) -> Result<SyncStatus, SyncError> {
        self.read_status()
    }

    fn push(&self, scope: SyncScope) -> Result<(), SyncError> {
        // Get password from environment via configured var or default
        let env_var = self
            .encryption_key_env
            .clone()
            .unwrap_or_else(|| "OPENAGENT_SYNC_PASSWORD".to_string());
        let password = std::env::var(&env_var)
            .map_err(|_| SyncError::Misconfigured("Sync encryption password not set in env"))?;

        let source_dir = self.source_dir(scope);
        if !source_dir.exists() {
            return Ok(()); // Nothing to sync
        }

        // Collect all files to sync
        let mut data = Vec::new();
        Self::collect_files_recursive(&source_dir, &mut data)?;

        // Serialize file data
        let serialized_data = serde_json::to_vec(&data)
            .map_err(|e| SyncError::Other(format!("Failed to serialize data: {}", e)))?;

        // Encrypt the data
        let encrypted_payload = self.encrypt_data(&serialized_data, password.as_bytes(), scope)?;

        // Save encrypted payload
        let encrypted_dir = self.encrypted_data_dir(scope);
        fs::create_dir_all(&encrypted_dir)?;

        let payload_file = encrypted_dir.join(format!("{}.json", Self::current_timestamp()));
        let payload_json = serde_json::to_string_pretty(&encrypted_payload)
            .map_err(|e| SyncError::Other(format!("Failed to serialize payload: {}", e)))?;
        fs::write(payload_file, payload_json)?;

        // Update status
        let mut status = self.read_status().unwrap_or_default();
        status.last_push = Some(Self::current_timestamp());
        status.pending = false;
        self.write_status(&status)?;

        Ok(())
    }

    fn pull(&self, scope: SyncScope) -> Result<(), SyncError> {
        let env_var = self
            .encryption_key_env
            .clone()
            .unwrap_or_else(|| "OPENAGENT_SYNC_PASSWORD".to_string());
        let password = std::env::var(&env_var)
            .map_err(|_| SyncError::Misconfigured("Sync encryption password not set in env"))?;

        let encrypted_dir = self.encrypted_data_dir(scope);
        if !encrypted_dir.exists() {
            return Err(SyncError::Other("No encrypted data found".to_string()));
        }

        // Find the latest encrypted payload
        let mut entries: Vec<_> = fs::read_dir(&encrypted_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .collect();

        entries.sort_by_key(|e| e.file_name());

        let latest_file =
            entries.last().ok_or_else(|| SyncError::Other("No payload files found".to_string()))?;

        // Load and decrypt payload
        let payload_content = fs::read_to_string(latest_file.path())?;
        let encrypted_payload: EncryptedPayload = serde_json::from_str(&payload_content)
            .map_err(|e| SyncError::Other(format!("Failed to parse payload: {}", e)))?;

        let decrypted_data = self.decrypt_data(&encrypted_payload, password.as_bytes())?;

        // Deserialize file data
        let file_data: Vec<(PathBuf, Vec<u8>)> = serde_json::from_slice(&decrypted_data)
            .map_err(|e| SyncError::Other(format!("Failed to deserialize data: {}", e)))?;

        // Restore files
        let dest_dir = self.source_dir(scope);
        for (relative_path, content) in file_data {
            let full_path = dest_dir.join(&relative_path);

            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(full_path, content)?;
        }

        // Update status
        let mut status = match self.read_status() {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to read secure sync status (pull); defaulting: {:?}", e);
                SyncStatus::default()
            }
        };
        status.last_pull = Some(Self::current_timestamp());
        self.write_status(&status)?;

        Ok(())
    }
}

impl SecureSyncProvider {
    /// Collect files recursively into a flat structure
    fn collect_files_recursive(
        dir: &Path,
        files: &mut Vec<(PathBuf, Vec<u8>)>,
    ) -> Result<(), SyncError> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                Self::collect_files_recursive(&path, files)?;
            } else {
                let relative_path = path
                    .strip_prefix(dir)
                    .map_err(|_| SyncError::Other("Failed to compute relative path".to_string()))?
                    .to_path_buf();

                let content = fs::read(&path)?;
                files.push((relative_path, content));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_installation_metadata_and_keys_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            provider: "secure".to_string(),
            data_dir: Some(temp_dir.path().to_path_buf()),
            endpoint_env: None,
            encryption_key_env: Some("OPENAGENT_SYNC_PASSWORD".to_string()),
        };

        let provider = SecureSyncProvider::new(&config).unwrap();
        let metadata = provider.installation_metadata();

        assert!(!metadata.installation_id.is_empty());
        assert!(!metadata.kdf_params.salt.is_empty());
        assert_eq!(metadata.kdf_params.algorithm, "PBKDF2-SHA256");
        assert_eq!(metadata.kdf_params.iterations, 100_000);
        // public key should be present
        assert!(metadata.public_key.as_ref().map(|v| !v.is_empty()).unwrap_or(false));

        // Keys should exist on disk
        let base_dir = provider.base_dir.clone();
        let priv_path = base_dir.join("keys").join("ed25519_private.pk8");
        let pub_path = base_dir.join("keys").join("ed25519_public.bin");
        assert!(priv_path.exists(), "private key should exist");
        assert!(pub_path.exists(), "public key should exist");
    }

    #[test]
    fn test_encryption_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            provider: "secure".to_string(),
            data_dir: Some(temp_dir.path().to_path_buf()),
            endpoint_env: None,
            encryption_key_env: Some("OPENAGENT_SYNC_PASSWORD".to_string()),
        };

        let provider = SecureSyncProvider::new(&config).unwrap();
        let test_data = b"Hello, secure sync!";
        let password = b"test-password-123";

        let encrypted = provider.encrypt_data(test_data, password, SyncScope::Settings).unwrap();
        let decrypted = provider.decrypt_data(&encrypted, password).unwrap();

        assert_eq!(test_data, &decrypted[..]);
    }

    #[test]
    fn test_sign_and_verify_message() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            provider: "secure".to_string(),
            data_dir: Some(temp_dir.path().to_path_buf()),
            endpoint_env: None,
            encryption_key_env: None,
        };
        let provider = SecureSyncProvider::new(&config).unwrap();
        let domain = "openagent-terminal.secure-sync.test";
        let message = b"test-message";
        let sig = provider.sign_message(domain, message);
        let public_key = provider.installation_metadata().public_key.clone().unwrap();
        let ok = SecureSyncProvider::verify_signature(&public_key, domain, message, &sig);
        assert!(ok, "signature should verify");
    }

    #[test]
    fn test_peer_handshake_flow() {
        let temp_dir_a = TempDir::new().unwrap();
        let temp_dir_b = TempDir::new().unwrap();

        let config_a = SyncConfig {
            provider: "secure".to_string(),
            data_dir: Some(temp_dir_a.path().to_path_buf()),
            endpoint_env: None,
            encryption_key_env: None,
        };
        let config_b = SyncConfig {
            provider: "secure".to_string(),
            data_dir: Some(temp_dir_b.path().to_path_buf()),
            endpoint_env: None,
            encryption_key_env: None,
        };

        let provider_a = SecureSyncProvider::new(&config_a).unwrap();
        let provider_b = SecureSyncProvider::new(&config_b).unwrap();

        let peer_b = PeerInfo {
            installation_id: provider_b.metadata.installation_id.clone(),
            display_name: "peer-b".to_string(),
            last_seen: 0,
            public_key: provider_b.metadata.public_key.clone().unwrap(),
            capabilities: vec![],
        };

        // A -> B: challenge
        let challenge = provider_a.create_handshake_challenge(&peer_b.installation_id).unwrap();

        // B -> A: response
        let response = provider_b
            .respond_to_handshake(&provider_a.metadata.installation_id, &challenge.challenge)
            .unwrap();

        // A verifies B's response
        let ok = provider_a
            .verify_handshake_response(&peer_b, &provider_a.metadata.installation_id, &response)
            .unwrap();
        assert!(ok, "handshake verification should succeed");
    }

    #[test]
    fn test_trust_store_add_rotate_revoke() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            provider: "secure".to_string(),
            data_dir: Some(temp_dir.path().to_path_buf()),
            endpoint_env: None,
            encryption_key_env: None,
        };
        let mut provider = SecureSyncProvider::new(&config).unwrap();

        // Create a dummy peer
        let peer = PeerInfo {
            installation_id: "peer-1".to_string(),
            display_name: "Peer One".to_string(),
            last_seen: 0,
            public_key: vec![1, 2, 3, 4],
            capabilities: vec![],
        };

        provider.add_peer(peer).unwrap();
        assert_eq!(provider.list_peers().len(), 1);

        // Rotate key
        let rotated = provider.rotate_peer_key("peer-1", vec![9, 9, 9, 9]).unwrap();
        assert!(rotated);
        let listed = provider.list_peers();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].public_key, vec![9, 9, 9, 9]);

        // Revoke
        let revoked = provider.revoke_peer("peer-1").unwrap();
        assert!(revoked);
        assert_eq!(provider.list_peers().len(), 0);
    }
}
