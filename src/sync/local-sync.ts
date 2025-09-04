/**
 * Local Encrypted Sync - Privacy-first synchronization without server dependency
 * Uses passphrase-based encryption for settings and history
 */

import * as fs from 'fs';
import * as path from 'path';
import * as crypto from 'crypto';
import { EventEmitter } from 'events';
import * as dgram from 'dgram';
import * as net from 'net';
import { promisify } from 'util';

const scrypt = promisify(crypto.scrypt);

export interface SyncConfig {
  mode: 'local-network' | 'file-based' | 'usb' | 'p2p';
  encryption: {
    algorithm: 'aes-256-gcm' | 'chacha20-poly1305';
    keyDerivation: 'argon2id' | 'scrypt' | 'pbkdf2';
    iterations?: number;
  };
  discovery?: {
    port: number;
    multicastAddress: string;
    timeout: number;
  };
  storage?: {
    path: string;
    watchForChanges: boolean;
  };
  dataTypes: {
    settings: boolean;
    history: boolean;
    workspaces: boolean;
    snippets: boolean;
  };
}

export interface SyncData {
  type: 'settings' | 'history' | 'workspaces' | 'snippets';
  timestamp: number;
  deviceId: string;
  data: any;
  checksum: string;
}

export interface KdfParams {
  algo: 'argon2id' | 'scrypt' | 'pbkdf2';
  salt: string; // base64
  iterations?: number;
  memory?: number;
  parallelism?: number;
}

export interface EncryptedPayload {
  algorithm: string;
  iv: string;
  authTag: string;
  ciphertext: string;
  timestamp: number;
  version: number;
  kdf: KdfParams;
}

export interface SyncPeer {
  id: string;
  name: string;
  address: string;
  port: number;
  lastSeen: number;
  publicKey?: string; // PEM
}

export class LocalSync extends EventEmitter {
  private config: SyncConfig;
  private passphrase: string | null = null;
  private derivedKey: Buffer | null = null;
  private kdfParams: KdfParams | null = null;
  private deviceId: string;
  private syncData: Map<string, SyncData> = new Map();
  private peers: Map<string, SyncPeer> = new Map();
  private discoverySocket: dgram.Socket | null = null;
  private syncServer: net.Server | null = null;
  private fileWatcher: fs.FSWatcher | null = null;
  private publicKeyPem: string | null = null;
  private privateKeyPem: string | null = null;

  constructor(config: Partial<SyncConfig> = {}) {
    super();
    this.config = {
      mode: 'local-network',
      encryption: {
        algorithm: 'aes-256-gcm',
        keyDerivation: 'argon2id',
        iterations: 100000,
      },
      discovery: {
        port: 42424,
        multicastAddress: '239.255.42.42',
        timeout: 30000,
      },
      dataTypes: {
        settings: true,
        history: true,
        workspaces: true,
        snippets: false,
      },
      ...config,
    };

    this.deviceId = this.generateDeviceId();
    this.loadOrCreateKeys();
    this.loadLocalData();
  }

  private generateDeviceId(): string {
    const hostname = require('os').hostname();
    const mac = this.getMacAddress();
    return crypto
      .createHash('sha256')
      .update(`${hostname}-${mac}`)
      .digest('hex')
      .substring(0, 16);
  }

  private getMacAddress(): string {
    const networkInterfaces = require('os').networkInterfaces();
    for (const name of Object.keys(networkInterfaces)) {
      for (const iface of networkInterfaces[name]) {
        if (!iface.internal && iface.mac !== '00:00:00:00:00:00') {
          return iface.mac;
        }
      }
    }
    return 'unknown';
  }

  // Passphrase and Key Management
  public async setPassphrase(passphrase: string): Promise<void> {
    this.passphrase = passphrase;
    this.derivedKey = await this.deriveKey(passphrase);
    this.emit('passphrase:set');
  }

  public async changePassphrase(oldPassphrase: string, newPassphrase: string): Promise<boolean> {
    // Verify old passphrase
    const oldKey = await this.deriveKey(oldPassphrase);

    // Try to decrypt existing data with old key
    const testData = this.syncData.values().next().value;
    if (testData) {
      try {
        // Test decryption with old key
        await this.decryptData(testData, oldKey);
      } catch {
        this.emit('error', { type: 'passphrase', message: 'Invalid old passphrase' });
        return false;
      }
    }

    // Re-encrypt all data with new passphrase
    const newKey = await this.deriveKey(newPassphrase);
    const reencryptedData = new Map<string, SyncData>();

    for (const [id, data] of this.syncData) {
      // Decrypt with old key
      const decrypted = await this.decryptData(data, oldKey);
      // Re-encrypt with new key
      const encrypted = await this.encryptData(decrypted, newKey);
      reencryptedData.set(id, encrypted);
    }

    this.syncData = reencryptedData;
    this.passphrase = newPassphrase;
    this.derivedKey = newKey;

    await this.saveLocalData();
    this.emit('passphrase:changed');
    return true;
  }

  private getStateDir(): string {
    const home = process.env.HOME || process.env.USERPROFILE || '';
    const dir = path.join(home, '.openagent', 'sync');
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
    return dir;
  }

  private getSaltPath(): string {
    return path.join(this.getStateDir(), 'kdf_salt.bin');
  }

  private getKeyPath(): string {
    return path.join(this.getStateDir(), 'keys.json');
  }

  private loadOrCreateKeys(): void {
    const keyPath = this.getKeyPath();
    try {
      if (fs.existsSync(keyPath)) {
        const { publicKeyPem, privateKeyPem } = JSON.parse(fs.readFileSync(keyPath, 'utf-8'));
        this.publicKeyPem = publicKeyPem;
        this.privateKeyPem = privateKeyPem;
      } else {
        const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
        this.publicKeyPem = publicKey.export({ type: 'spki', format: 'pem' }).toString();
        this.privateKeyPem = privateKey.export({ type: 'pkcs8', format: 'pem' }).toString();
        fs.writeFileSync(keyPath, JSON.stringify({ publicKeyPem: this.publicKeyPem, privateKeyPem: this.privateKeyPem }, null, 2));
      }
    } catch (e) {
      this.emit('error', { type: 'keys', error: e });
    }
  }

  private async getOrCreateSalt(): Promise<Buffer> {
    const saltPath = this.getSaltPath();
    if (fs.existsSync(saltPath)) {
      return fs.readFileSync(saltPath);
    }
    const salt = crypto.randomBytes(32);
    fs.writeFileSync(saltPath, salt);
    return salt;
  }

  private async deriveKey(passphrase: string): Promise<Buffer> {
    const salt = await this.getOrCreateSalt();

    let key: Buffer;
    switch (this.config.encryption.keyDerivation) {
      case 'argon2id':
        // Placeholder: fallback to scrypt until argon2 library is wired
      case 'scrypt':
        key = (await scrypt(passphrase, salt, 32)) as Buffer;
        break;
      case 'pbkdf2':
        key = crypto.pbkdf2Sync(
          passphrase,
          salt,
          this.config.encryption.iterations || 100000,
          32,
          'sha256'
        );
        break;
      default:
        throw new Error(`Unsupported key derivation: ${this.config.encryption.keyDerivation}`);
    }

    // Capture KDF params for payload embedding
    this.kdfParams = {
      algo: this.config.encryption.keyDerivation,
      salt: salt.toString('base64'),
      iterations: this.config.encryption.keyDerivation === 'pbkdf2' || this.config.encryption.keyDerivation === 'scrypt'
        ? (this.config.encryption.iterations || 100000) : undefined,
    };

    return key;
  }

  // Encryption/Decryption
  private async encryptData(data: any, key?: Buffer): Promise<EncryptedPayload> {
    const encryptionKey = key || this.derivedKey;
    if (!encryptionKey) {
      throw new Error('No encryption key available');
    }

    const algorithm = this.config.encryption.algorithm;
    const iv = crypto.randomBytes(16);

    let cipher: crypto.CipherGCM;

    switch (algorithm) {
      case 'aes-256-gcm':
        cipher = crypto.createCipheriv('aes-256-gcm', encryptionKey, iv) as crypto.CipherGCM;
        break;
      case 'chacha20-poly1305':
        cipher = crypto.createCipheriv('chacha20-poly1305', encryptionKey, iv) as crypto.CipherGCM;
        break;
      default:
        throw new Error(`Unsupported algorithm: ${algorithm}`);
    }

    const plaintext = JSON.stringify(data);
    const ciphertext = Buffer.concat([
      cipher.update(plaintext, 'utf8'),
      cipher.final(),
    ]);

    const authTag = cipher.getAuthTag();

    return {
      algorithm,
      iv: iv.toString('base64'),
      authTag: authTag.toString('base64'),
      ciphertext: ciphertext.toString('base64'),
      timestamp: Date.now(),
      version: 1,
      kdf: this.kdfParams || { algo: this.config.encryption.keyDerivation, salt: (await this.getOrCreateSalt()).toString('base64'), iterations: this.config.encryption.iterations },
    };
  }

  private async decryptData(payload: EncryptedPayload, key?: Buffer): Promise<any> {
    const decryptionKey = key || this.derivedKey;
    if (!decryptionKey) {
      throw new Error('No decryption key available');
    }

    const iv = Buffer.from(payload.iv, 'base64');
    const authTag = Buffer.from(payload.authTag, 'base64');
    const ciphertext = Buffer.from(payload.ciphertext, 'base64');

    // If payload carries KDF params, derive a per-payload key using passphrase
    let effectiveKey = decryptionKey;
    if (payload.kdf && this.passphrase) {
      const salt = Buffer.from(payload.kdf.salt, 'base64');
      switch (payload.kdf.algo) {
        case 'argon2id':
          // Fallback to scrypt until argon2 is wired
        case 'scrypt':
          effectiveKey = (await scrypt(this.passphrase, salt, 32)) as Buffer;
          break;
        case 'pbkdf2':
          effectiveKey = crypto.pbkdf2Sync(
            this.passphrase,
            salt,
            payload.kdf.iterations || 100000,
            32,
            'sha256'
          );
          break;
        default:
          throw new Error(`Unsupported KDF in payload: ${payload.kdf.algo}`);
      }
    }

    let decipher: crypto.DecipherGCM;

    switch (payload.algorithm) {
      case 'aes-256-gcm':
        decipher = crypto.createDecipheriv('aes-256-gcm', effectiveKey, iv) as crypto.DecipherGCM;
        break;
      case 'chacha20-poly1305':
        decipher = crypto.createDecipheriv('chacha20-poly1305', effectiveKey, iv) as crypto.DecipherGCM;
        break;
      default:
        throw new Error(`Unsupported algorithm: ${payload.algorithm}`);
    }

    decipher.setAuthTag(authTag);

    const plaintext = Buffer.concat([
      decipher.update(ciphertext),
      decipher.final(),
    ]);

    return JSON.parse(plaintext.toString('utf8'));
  }

  // Data Management
  public async addData(type: SyncData['type'], data: any): Promise<void> {
    if (!this.config.dataTypes[type]) {
      return; // This data type is not configured for sync
    }

    if (!this.derivedKey) {
      throw new Error('Passphrase must be set before adding data');
    }

    const syncData: SyncData = {
      type,
      timestamp: Date.now(),
      deviceId: this.deviceId,
      data,
      checksum: this.calculateChecksum(data),
    };

    const encrypted = await this.encryptData(syncData);
    const id = `${type}-${syncData.timestamp}-${this.deviceId}`;

    this.syncData.set(id, encrypted as any);
    await this.saveLocalData();

    this.emit('data:added', { type, id });

    // Propagate to peers if in network mode
    if (this.config.mode === 'local-network' || this.config.mode === 'p2p') {
      await this.broadcastToPeers(encrypted);
    }
  }

  private calculateChecksum(data: any): string {
    return crypto
      .createHash('sha256')
      .update(JSON.stringify(data))
      .digest('hex');
  }

  // Network Discovery and Sync
  public async startNetworkSync(): Promise<void> {
    if (this.config.mode !== 'local-network' && this.config.mode !== 'p2p') {
      return;
    }

    // Start discovery service
    await this.startDiscovery();

    // Start sync server
    await this.startSyncServer();

    this.emit('sync:started');
  }

  private async startDiscovery(): Promise<void> {
    const { port, multicastAddress } = this.config.discovery!;

    this.discoverySocket = dgram.createSocket({ type: 'udp4', reuseAddr: true });

    this.discoverySocket.on('message', (msg, rinfo) => {
      try {
        const announcement = JSON.parse(msg.toString());

        if (announcement.deviceId === this.deviceId) {
          return; // Ignore our own announcements
        }

        // Verify signature
        if (!announcement.sig || !announcement.publicKey) {
          return; // ignore unauthenticated
        }
        const verify = crypto.createVerify('SHA256');
        const signedFields = {
          deviceId: announcement.deviceId,
          name: announcement.name,
          syncPort: announcement.syncPort,
          timestamp: announcement.timestamp,
          version: announcement.version,
        };
        const payloadStr = JSON.stringify(signedFields);
        verify.update(payloadStr);
        verify.end();
        const isValid = verify.verify(announcement.publicKey, Buffer.from(announcement.sig, 'base64'));
        if (!isValid) return;

        const peer: SyncPeer = {
          id: announcement.deviceId,
          name: announcement.name,
          address: rinfo.address,
          port: announcement.syncPort,
          lastSeen: Date.now(),
          publicKey: announcement.publicKey,
        };

        this.peers.set(peer.id, peer);
        this.emit('peer:discovered', peer);

      } catch (error) {
        this.emit('error', { type: 'discovery', error });
      }
    });

    this.discoverySocket.bind(port, () => {
      this.discoverySocket!.addMembership(multicastAddress);

      // Announce ourselves periodically
      setInterval(() => {
        this.announcePresence();
      }, 5000);

      this.announcePresence();
    });

    // Clean up stale peers
    setInterval(() => {
      const now = Date.now();
      for (const [id, peer] of this.peers) {
        if (now - peer.lastSeen > this.config.discovery!.timeout) {
          this.peers.delete(id);
          this.emit('peer:lost', peer);
        }
      }
    }, 10000);
  }

  private announcePresence(): void {
    if (!this.discoverySocket) return;

    const base = {
      deviceId: this.deviceId,
      name: require('os').hostname(),
      syncPort: 42425,
      timestamp: Date.now(),
      version: 1,
    };

    // Sign announcement
    let sigB64 = '';
    if (this.privateKeyPem) {
      const sign = crypto.createSign('SHA256');
      sign.update(JSON.stringify(base));
      sign.end();
      const signature = sign.sign(this.privateKeyPem);
      sigB64 = signature.toString('base64');
    }

    const announcement = {
      ...base,
      publicKey: this.publicKeyPem,
      sig: sigB64,
    };

    const message = Buffer.from(JSON.stringify(announcement));

    this.discoverySocket.send(
      message,
      this.config.discovery!.port,
      this.config.discovery!.multicastAddress
    );
  }

  private async startSyncServer(): Promise<void> {
    this.syncServer = net.createServer((socket) => {
      socket.on('data', async (data) => {
        try {
          const request = JSON.parse(data.toString());

          switch (request.type) {
            case 'sync-request':
              await this.handleSyncRequest(socket, request);
              break;
            case 'data-push':
              await this.handleDataPush(socket, request);
              break;
            case 'ping':
              socket.write(JSON.stringify({ type: 'pong' }));
              break;
          }
        } catch (error) {
          this.emit('error', { type: 'sync-server', error });
        }
      });
    });

    this.syncServer.listen(42425, () => {
      this.emit('server:started', { port: 42425 });
    });
  }

  private async handleSyncRequest(socket: net.Socket, request: any): Promise<void> {
    // Send all our encrypted data to the requesting peer
    const base = {
      type: 'sync-response',
      deviceId: this.deviceId,
      data: Array.from(this.syncData.entries()),
      timestamp: Date.now(),
    };

    const signed = this.signMessage(base);
    socket.write(JSON.stringify(signed));
  }

  private async handleDataPush(socket: net.Socket, request: any): Promise<void> {
    // Receive and merge data from peer
    try {
      const { data, deviceId, sig, publicKey } = request;

      // Verify signature
      if (!this.verifyMessage(request, publicKey)) {
        socket.write(JSON.stringify({ type: 'ack', success: false, error: 'invalid signature' }));
        return;
      }

      for (const [id, encryptedData] of data) {
        // Only accept newer data
        const existing = this.syncData.get(id);
        if (!existing || encryptedData.timestamp > existing.timestamp) {
          this.syncData.set(id, encryptedData);
        }
      }

      await this.saveLocalData();

      socket.write(JSON.stringify({
        type: 'ack',
        success: true
      }));

      this.emit('sync:received', { from: deviceId, items: data.length });

    } catch (error) {
      socket.write(JSON.stringify({
        type: 'ack',
        success: false,
        error: error.message
      }));
    }
  }

  private async broadcastToPeers(data: EncryptedPayload): Promise<void> {
    const base = {
      type: 'data-push',
      deviceId: this.deviceId,
      data: [[`temp-${Date.now()}`, data]],
      timestamp: Date.now(),
    };
    const message = this.signMessage(base);

    for (const peer of this.peers.values()) {
      this.sendToPeer(peer, message).catch(error => {
        this.emit('error', { type: 'broadcast', peer: peer.id, error });
      });
    }
  }

  private async sendToPeer(peer: SyncPeer, message: any): Promise<void> {
    return new Promise((resolve, reject) => {
      const client = new net.Socket();

      client.connect(peer.port, peer.address, () => {
        client.write(JSON.stringify(message));
      });

      client.on('data', (data) => {
        try {
          const response = JSON.parse(data.toString());
          if (response.type === 'ack' && response.success) {
            resolve();
          } else {
            reject(new Error(response.error || 'Unknown error'));
          }
        } catch (error) {
          reject(error);
        } finally {
          client.destroy();
        }
      });

      client.on('error', reject);

      setTimeout(() => {
        client.destroy();
        reject(new Error('Timeout'));
      }, 5000);
    });
  }

  // File-based Sync
  public async startFileSync(syncPath: string): Promise<void> {
    if (this.config.mode !== 'file-based' && this.config.mode !== 'usb') {
      return;
    }

    this.config.storage = {
      path: syncPath,
      watchForChanges: true,
    };

    // Ensure sync directory exists
    if (!fs.existsSync(syncPath)) {
      fs.mkdirSync(syncPath, { recursive: true });
    }

    // Load existing sync data
    await this.loadFromSyncPath();

    // Watch for changes
    if (this.config.storage.watchForChanges) {
      this.fileWatcher = fs.watch(syncPath, async (eventType, filename) => {
        if (filename && filename.endsWith('.sync')) {
          await this.loadFromSyncPath();
          this.emit('sync:file-changed', { filename });
        }
      });
    }

    this.emit('sync:file-started', { path: syncPath });
  }

  private async loadFromSyncPath(): Promise<void> {
    const syncPath = this.config.storage!.path;
    const files = fs.readdirSync(syncPath);

    for (const file of files) {
      if (file.endsWith('.sync')) {
        try {
          const filePath = path.join(syncPath, file);
          const content = fs.readFileSync(filePath, 'utf-8');
          const encrypted = JSON.parse(content) as EncryptedPayload;

          // Verify we can decrypt it
          if (this.derivedKey) {
            await this.decryptData(encrypted);
            const id = file.replace('.sync', '');
            this.syncData.set(id, encrypted as any);
          }
        } catch (error) {
          this.emit('error', { type: 'file-load', file, error });
        }
      }
    }
  }

  private async saveToSyncPath(): Promise<void> {
    if (!this.config.storage) return;

    const syncPath = this.config.storage.path;

    for (const [id, data] of this.syncData) {
      const filePath = path.join(syncPath, `${id}.sync`);
      fs.writeFileSync(filePath, JSON.stringify(data, null, 2));
    }
  }

  // Local Data Persistence
  private loadLocalData(): void {
    const dataPath = this.getLocalDataPath();

    if (fs.existsSync(dataPath)) {
      try {
        const content = fs.readFileSync(dataPath, 'utf-8');
        const data = JSON.parse(content);

        for (const [id, encrypted] of Object.entries(data)) {
          this.syncData.set(id, encrypted as any);
        }
      } catch (error) {
        this.emit('error', { type: 'local-load', error });
      }
    }
  }

  private async saveLocalData(): Promise<void> {
    const dataPath = this.getLocalDataPath();
    const dataDir = path.dirname(dataPath);

    if (!fs.existsSync(dataDir)) {
      fs.mkdirSync(dataDir, { recursive: true });
    }

    const data = Object.fromEntries(this.syncData);
    fs.writeFileSync(dataPath, JSON.stringify(data, null, 2));
  }

  private getLocalDataPath(): string {
    const home = process.env.HOME || process.env.USERPROFILE || '';
    return path.join(home, '.openagent', 'sync', 'local-data.json');
  }

  // Data Retrieval
  public async getData(type: SyncData['type']): Promise<any[]> {
    if (!this.derivedKey) {
      throw new Error('Passphrase must be set before retrieving data');
    }

    const results: any[] = [];

    for (const [id, encrypted] of this.syncData) {
      if (id.startsWith(type)) {
        try {
          const decrypted = await this.decryptData(encrypted as any);
          if (decrypted.type === type) {
            results.push(decrypted.data);
          }
        } catch (error) {
          this.emit('error', { type: 'decrypt', id, error });
        }
      }
    }

    return results.sort((a, b) => b.timestamp - a.timestamp);
  }

  // Conflict Resolution
  public async resolveConflicts(strategy: 'newest' | 'merge' | 'manual'): Promise<void> {
    const conflicts = new Map<string, SyncData[]>();

    // Group data by type and key
    for (const [id, data] of this.syncData) {
      const [type] = id.split('-');
      if (!conflicts.has(type)) {
        conflicts.set(type, []);
      }
      conflicts.get(type)!.push(data as any);
    }

    // Resolve conflicts based on strategy
    for (const [type, items] of conflicts) {
      switch (strategy) {
        case 'newest':
          // Keep only the newest item
          const newest = items.sort((a, b) => b.timestamp - a.timestamp)[0];
          this.syncData.clear();
          this.syncData.set(`${type}-${newest.timestamp}-${newest.deviceId}`, newest as any);
          break;

        case 'merge':
          // Merge data from all devices
          // Implementation depends on data type
          this.emit('conflict:merge-required', { type, items });
          break;

        case 'manual':
          // Emit event for manual resolution
          this.emit('conflict:manual-required', { type, items });
          break;
      }
    }

    await this.saveLocalData();
  }

  // Export/Import
  public async exportBundle(outputPath: string): Promise<void> {
    if (!this.derivedKey) {
      throw new Error('Passphrase must be set before exporting');
    }

    const bundle = {
      version: 1,
      deviceId: this.deviceId,
      timestamp: Date.now(),
      data: Object.fromEntries(this.syncData),
    };

    // Encrypt the entire bundle
    const encrypted = await this.encryptData(bundle);
    fs.writeFileSync(outputPath, JSON.stringify(encrypted, null, 2));

    this.emit('bundle:exported', { path: outputPath });
  }

  public async importBundle(bundlePath: string, passphrase?: string): Promise<void> {
    const content = fs.readFileSync(bundlePath, 'utf-8');
    const encrypted = JSON.parse(content) as EncryptedPayload;

    // Use provided passphrase or current one
    const key = passphrase ? await this.deriveKey(passphrase) : this.derivedKey;

    if (!key) {
      throw new Error('Passphrase required for import');
    }

    const bundle = await this.decryptData(encrypted, key);

    // Merge imported data
    for (const [id, data] of Object.entries(bundle.data)) {
      this.syncData.set(id, data as any);
    }

    await this.saveLocalData();
    this.emit('bundle:imported', { path: bundlePath, items: Object.keys(bundle.data).length });
  }

  // Cleanup
  public async stop(): Promise<void> {
    if (this.discoverySocket) {
      this.discoverySocket.close();
      this.discoverySocket = null;
    }

    if (this.syncServer) {
      this.syncServer.close();
      this.syncServer = null;
    }

    if (this.fileWatcher) {
      this.fileWatcher.close();
      this.fileWatcher = null;
    }

    await this.saveLocalData();

    if (this.config.mode === 'file-based' || this.config.mode === 'usb') {
      await this.saveToSyncPath();
    }

    this.emit('sync:stopped');
  }

  // Status and Monitoring
  public getStatus(): any {
    return {
      deviceId: this.deviceId,
      mode: this.config.mode,
      isEncrypted: !!this.derivedKey,
      peers: Array.from(this.peers.values()),
      dataCount: this.syncData.size,
      publicKey: this.publicKeyPem,
      dataTypes: Object.keys(this.config.dataTypes).filter(k => this.config.dataTypes[k as keyof typeof this.config.dataTypes]),
    };
  }

  public getPeers(): SyncPeer[] {
    return Array.from(this.peers.values());
  }
  private signMessage<T extends Record<string, any>>(obj: T): T & { sig: string; publicKey: string | null } {
    if (!this.privateKeyPem || !this.publicKeyPem) {
      return { ...(obj as any), sig: '', publicKey: null };
    }
    const sign = crypto.createSign('SHA256');
    const payload = { ...obj };
    const payloadStr = JSON.stringify(payload);
    sign.update(payloadStr);
    sign.end();
    const signature = sign.sign(this.privateKeyPem).toString('base64');
    return { ...(obj as any), sig: signature, publicKey: this.publicKeyPem };
  }

  private verifyMessage<T extends Record<string, any>>(msg: T, pubKey?: string): boolean {
    try {
      const { sig, publicKey, ...rest } = msg as any;
      const key = pubKey || publicKey;
      if (!sig || !key) return false;
      const verify = crypto.createVerify('SHA256');
      const payloadStr = JSON.stringify(rest);
      verify.update(payloadStr);
      verify.end();
      return verify.verify(key, Buffer.from(sig, 'base64'));
    } catch {
      return false;
    }
  }
}

// Export for use
export const createLocalSync = (config?: Partial<SyncConfig>) => {
  return new LocalSync(config);
};
