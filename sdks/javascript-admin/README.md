# @aegis-db/admin

Official server-side Admin SDK for **Aegis-DB**. This is the privileged SDK (similar to `firebase-admin`) for managing users, clusters, backups, vault secrets, security shield, and GDPR compliance from trusted server environments.

## Installation

```bash
npm install @aegis-db/admin
```

## Quick Start

```typescript
import { AegisAdmin } from '@aegis-db/admin';

const admin = new AegisAdmin({
  url: 'http://localhost:9090',
  username: 'admin',
  password: 'secret',
});

await admin.connect();

// List users
const { users } = await admin.auth.listUsers();

// Store a secret in the vault
await admin.vault.putSecret('stripe-key', 'sk_live_...');

// Block a malicious IP
await admin.shield.blockIP('10.0.0.1', 'brute force attempt');

// GDPR: delete a user's data
await admin.compliance.deleteDataSubject('user-123');

await admin.disconnect();
```

## Authentication

```typescript
// Username/password
const admin = new AegisAdmin({
  url: 'http://localhost:9090',
  username: 'admin',
  password: 'secret',
});

// API key
const admin = new AegisAdmin({
  url: 'http://localhost:9090',
  apiKey: 'your-api-key',
});

// Pre-existing token
const admin = new AegisAdmin({
  url: 'http://localhost:9090',
  token: 'existing-bearer-token',
});
```

## Services

### auth - User and Role Management

```typescript
await admin.auth.listUsers();
await admin.auth.createUser({ username: 'alice', password: 'pass123', roles: ['reader'] });
await admin.auth.updateUser('alice', { roles: ['reader', 'writer'] });
await admin.auth.deleteUser('alice');

await admin.auth.listRoles();
await admin.auth.createRole({ name: 'analyst', permissions: ['read', 'query'] });
await admin.auth.deleteRole('analyst');
```

### cluster - Cluster and Node Management

```typescript
await admin.cluster.getCluster();
await admin.cluster.listNodes();
await admin.cluster.restartNode('node-abc');
await admin.cluster.drainNode('node-abc');
await admin.cluster.getNodeLogs('node-abc');
await admin.cluster.removeNode('node-abc');

await admin.cluster.getStorage();
await admin.cluster.getStats();
await admin.cluster.getDatabase();
await admin.cluster.getAlerts();
await admin.cluster.getActivities();
await admin.cluster.getSettings();
await admin.cluster.updateSettings({ max_connections: 500 });
```

### backup - Backup and Restore

```typescript
await admin.backup.create({ name: 'daily-backup' });
await admin.backup.list();
await admin.backup.restore({ backup_id: 'bk-123' });
await admin.backup.delete('bk-123');
```

### vault - Secrets and Transit Encryption

```typescript
await admin.vault.getStatus();
await admin.vault.seal();
await admin.vault.unseal('unseal-key-share');

await admin.vault.listSecrets();
await admin.vault.putSecret('db-password', 'hunter2');
const secret = await admin.vault.getSecret('db-password');
await admin.vault.deleteSecret('db-password');

const { ciphertext } = await admin.vault.transitEncrypt('my-key', 'plaintext');
const { plaintext } = await admin.vault.transitDecrypt('my-key', ciphertext);
await admin.vault.createTransitKey('my-key', 'aes256-gcm');
await admin.vault.listTransitKeys();

await admin.vault.getAuditLog();
```

### shield - Security Shield

```typescript
await admin.shield.getStatus();
await admin.shield.getStats();
await admin.shield.getEvents();

await admin.shield.listBlocked();
await admin.shield.blockIP('10.0.0.1', 'malicious', 3600);
await admin.shield.unblockIP('10.0.0.1');

await admin.shield.listAllowlist();
await admin.shield.addToAllowlist('192.168.1.1', 'office');
await admin.shield.removeFromAllowlist('192.168.1.1');

await admin.shield.getPolicy();
await admin.shield.updatePolicy({ auto_block: true, auto_block_threshold: 5 });

await admin.shield.getIPReputation('10.0.0.1');
await admin.shield.getThreatFeed();
```

### compliance - GDPR, Consent, and Breach Management

```typescript
// Data subject rights
await admin.compliance.deleteDataSubject('user-123');
await admin.compliance.exportData({ subject_id: 'user-123', format: 'json' });

// Certificates
await admin.compliance.listCertificates();
await admin.compliance.getCertificate('cert-1');
await admin.compliance.verifyCertificate('cert-1');

// Audit
await admin.compliance.getAuditTrail('user-123');
await admin.compliance.verifyAuditLog();

// Consent
await admin.compliance.recordConsent({ subject_id: 'user-123', purpose: 'marketing', granted: true });
await admin.compliance.getConsentStats();
await admin.compliance.getSubjectConsent('user-123');
await admin.compliance.getConsentHistory('user-123');
await admin.compliance.exportConsent('user-123');
await admin.compliance.checkConsent('user-123', 'marketing');
await admin.compliance.revokeConsent('user-123', 'marketing');
await admin.compliance.deleteSubjectConsent('user-123');

// Do Not Sell (CCPA)
await admin.compliance.getDoNotSellList();

// Breach management
await admin.compliance.listBreaches();
await admin.compliance.getBreachStats();
await admin.compliance.getBreach('br-1');
await admin.compliance.acknowledgeBreach('br-1');
await admin.compliance.resolveBreach('br-1');
await admin.compliance.getBreachReport('br-1');
await admin.compliance.cleanupBreaches();

// Security events
await admin.compliance.getSecurityEvents();
```

## Error Handling

```typescript
import { AdminError, AuthenticationError, ConnectionError, NotFoundError, ForbiddenError } from '@aegis-db/admin';

try {
  await admin.vault.getSecret('missing-key');
} catch (error) {
  if (error instanceof NotFoundError) {
    console.log('Secret not found');
  } else if (error instanceof AuthenticationError) {
    console.log('Not authenticated');
  } else if (error instanceof ForbiddenError) {
    console.log('Insufficient permissions');
  } else if (error instanceof ConnectionError) {
    console.log('Server unreachable');
  } else if (error instanceof AdminError) {
    console.log(`Error ${error.statusCode}: ${error.message}`);
  }
}
```

## Configuration

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `url` | `string` | (required) | Server URL |
| `username` | `string` | - | Admin username |
| `password` | `string` | - | Admin password |
| `apiKey` | `string` | - | API key (alternative auth) |
| `token` | `string` | - | Pre-existing bearer token |
| `timeout` | `number` | `30000` | Request timeout (ms) |
| `retryAttempts` | `number` | `3` | Retry count for 5xx errors |
| `retryDelay` | `number` | `1000` | Base retry delay (ms) |

## Requirements

- Node.js >= 18.0.0 (uses native `fetch`)
- Aegis-DB server v0.2.0+

## License

MIT
