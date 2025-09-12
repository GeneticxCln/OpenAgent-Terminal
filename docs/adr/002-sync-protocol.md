# ADR 002: Sync Protocol Architecture

## Status
Proposed

## Date
2024-08-30

## Context

OpenAgent Terminal needs to provide synchronization capabilities for user settings, history, and configurations across multiple devices. The key requirements are:

1. **End-to-end encryption**: User data must be encrypted before leaving the device
2. **Provider flexibility**: Support multiple storage backends
3. **Conflict resolution**: Handle concurrent modifications gracefully
4. **Offline support**: Work without network connectivity
5. **Privacy preservation**: No vendor lock-in or data mining

## Decision

We will implement a **provider-based sync architecture** with the following components:

### 1. Sync Provider Trait

```rust
pub trait SyncProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn push(&self, data: EncryptedData) -> Result<SyncToken, SyncError>;
    fn pull(&self, token: SyncToken) -> Result<EncryptedData, SyncError>;
    fn resolve_conflict(&self, local: &EncryptedData, remote: &EncryptedData) -> ConflictResolution;
}
```

### 2. Encryption Layer

Use `age` encryption for all sync data:

```rust
pub struct EncryptedData {
    pub encrypted_payload: Vec<u8>,
    pub nonce: Vec<u8>,
    pub timestamp: u64,
    pub device_id: String,
}
```

**Why age encryption?**
- Modern, secure encryption
- Simple API
- Small library size
- Active maintenance
- Passphrase and public key support

### 3. Provider Implementations

#### Local Filesystem (MVP)
```rust
pub struct FilesystemProvider {
    sync_dir: PathBuf,
    encryption_key: SecretString,
}
```
- For testing and local network sync
- No external dependencies
- Simple implementation

#### Git-based Sync
```rust
pub struct GitProvider {
    repository: Repository,
    branch: String,
    encryption_key: SecretString,
}
```
- Uses existing Git infrastructure
- Version history built-in
- Works with any Git host

#### Cloud Providers (Future)
- S3-compatible storage
- WebDAV
- Google Drive / Dropbox (via APIs)

### 4. Sync Data Structure

```rust
pub struct SyncManifest {
    pub version: u32,
    pub device_id: String,
    pub timestamp: u64,
    pub entries: Vec<SyncEntry>,
}

pub struct SyncEntry {
    pub key: String,  // e.g., "config", "history", "ai_cache"
    pub hash: String,
    pub size: u64,
    pub modified: u64,
}
```

### 5. Conflict Resolution Strategy

```rust
pub enum ConflictResolution {
    UseLocal,
    UseRemote,
    Merge(MergeStrategy),
    Manual(Vec<Diff>),
}

pub enum MergeStrategy {
    LastWriteWins,
    FirstWriteWins,
    Union,  // For lists like history
    Intersection,  // For security settings
}
```

## Consequences

### Positive

1. **Privacy Protected**: End-to-end encryption ensures data privacy
2. **Flexibility**: Multiple backend options for different use cases
3. **User Control**: Users own their data and choose storage
4. **Offline First**: Local changes queue for later sync
5. **Incremental Sync**: Only changed data is transmitted
6. **Audit Trail**: Sync history for debugging

### Negative

1. **Complexity**: Encryption and conflict resolution add complexity
2. **Key Management**: Users must manage encryption keys
3. **Storage Costs**: Cloud providers may charge for storage
4. **Network Usage**: Initial sync may be bandwidth-intensive

### Neutral

1. **Performance**: Encryption has minimal overhead
2. **Dependencies**: Each provider has different requirements
3. **Testing**: Requires comprehensive sync testing

## Implementation Plan

### Phase 1: Foundation
1. Define sync traits and data structures
2. Implement encryption layer with age
3. Create filesystem provider for testing
4. Add basic conflict detection

### Phase 2: Git Provider
1. Implement Git-based sync
2. Add merge strategies
3. Create sync UI in terminal
4. Add key derivation from passphrase

### Phase 3: Cloud Providers
1. S3-compatible storage
2. WebDAV support
3. OAuth for cloud services
4. Compression for large data

### Phase 4: Advanced Features
1. Selective sync (choose what to sync)
2. Multi-device presence
3. Sync status indicators
4. Bandwidth throttling

## Security Considerations

### MVP Auth and Key Management (current implementation)

While the long-term plan specifies age-based encryption and Argon2 KDF, the current secure sync provider (MVP) implements:

- Authentication: Ed25519 keypairs (per installation)
  - Keys are auto-generated on first use and persisted to STATE/openagent-terminal/secure-sync/keys/
    - ed25519_private.pk8 (0600 on Unix)
    - ed25519_public.bin
  - The public key is also stored in installation.json.
  - Peer public keys are stored in peers.json (trust store) with installation_id, display_name, last_seen, and capabilities.

- Handshake: challenge/response with domain separation
  - Domain string: "openagent-terminal.secure-sync.handshake.v1"
  - Signature covers: domain | from_installation_id | to_installation_id | challenge_bytes
  - Use random 32-byte challenges and enforce freshness at the transport layer to mitigate replay.

- Encryption of payloads: AES-256-GCM
  - Key derived from a runtime password using PBKDF2-HMAC-SHA256 and per-install random salt, then expanded with HKDF-SHA256 for AEAD.
  - KDF parameters (algorithm, salt, iterations) are stored in installation metadata and embedded with payloads as needed for decryption.
  - The runtime password is supplied via an environment variable. The variable name is [sync].encryption_key_env; if unset, OPENAGENT_SYNC_PASSWORD is used.

- Storage locations (STATE base)
  - $XDG_STATE_HOME/openagent-terminal/secure-sync, or ~/.local/state/openagent-terminal/secure-sync
  - installation.json, peers.json, keys/, and encrypted/ directories as described in docs/sync.md.

These choices keep the MVP functional and secure while allowing us to switch to age/Argon2 in a future phase without breaking the public API.

### Encryption
- All data encrypted with age before sync
- Keys derived from user passphrase using Argon2
- Option for hardware key support (YubiKey)

### Key Storage
```rust
// Never store keys in config files
// Use system keyring where available
pub enum KeyStorage {
    SystemKeyring,  // Preferred
    EnvironmentVariable,  // Fallback
    UserPrompt,  // Most secure
}
```

### Data Sanitization
- Strip sensitive environment variables
- Exclude temporary files
- Mask credentials in history

## Configuration

```toml
[sync]
enabled = false  # Opt-in
provider = "git"

[sync.encryption]
method = "age"
key_source = "keyring"  # or "prompt", "env"

[sync.git]
repository = "git@github.com:user/terminal-sync.git"
branch = "main"
auto_sync = true
sync_interval = 300  # seconds

[sync.filters]
include = ["config", "history", "ai_cache"]
exclude = ["*.tmp", "*.log"]
max_history_items = 10000
```

## Alternatives Considered

### 1. Centralized Sync Service
**Rejected**: Violates privacy principles, creates vendor lock-in

### 2. Peer-to-Peer Sync
**Deferred**: Complex NAT traversal, planned for future

### 3. Blockchain Storage
**Rejected**: Excessive complexity, poor performance

### 4. Plain Text Sync
**Rejected**: Security risk, no privacy protection

### 5. Built-in Cloud Service
**Rejected**: Maintenance burden, privacy concerns

## Migration Path

For users coming from other terminals:

1. **Import existing configs**: Convert and encrypt
2. **History migration**: Parse and sync bash/zsh history
3. **Gradual adoption**: Sync individual components
4. **Export capability**: Allow data export in standard formats

## Testing Strategy

### Unit Tests
- Encryption/decryption roundtrip
- Conflict detection algorithms
- Provider implementations

### Integration Tests
- Multi-device sync scenarios
- Network failure handling
- Concurrent modification handling

### End-to-End Tests
- Full sync workflow
- Key rotation
- Provider switching

## Future Enhancements

1. **Collaborative Features**: Share configurations with team
2. **Sync Profiles**: Different sync settings per environment
3. **Compression**: Reduce bandwidth usage
4. **Delta Sync**: Only sync changes, not full files
5. **Sync Analytics**: Usage patterns and optimization

## References

- [age Encryption](https://age-encryption.org/)
- [Conflict-free Replicated Data Types](https://crdt.tech/)
- [Git Sync Patterns](https://www.git-scm.com/book/en/v2/Git-Tools-Submodules)
- [Security Best Practices](https://owasp.org/www-project-application-security-verification-standard/)

## Sign-off

- Architecture Team: Pending
- Security Team: Pending
- Product Team: Pending

---

*This ADR documents the synchronization architecture for OpenAgent Terminal, prioritizing privacy and user control.*

*Last Modified: 2024-08-30*
*Author: OpenAgent Terminal Team*
