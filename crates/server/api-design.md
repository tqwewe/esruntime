# ESRuntime HTTP API

Base URL: `https://api.esruntime.io/v1`

## Authentication

All requests require an API key:
```
Authorization: Bearer <api_key>
```

---

## Commands

### Execute a Command

```
POST /commands/{command_name}
```

Execute a registered command handler.

**Request:**
```json
{
  "source_account": "alice",
  "dest_account": "bob",
  "amount": 50.0
}
```

**Response (success):**
```json
{
  "status": "ok",
  "events": [
    {
      "id": "evt_01H8X...",
      "type": "SentFunds",
      "data": {
        "account_id": "alice",
        "amount": 50.0,
        "recipient_id": "bob"
      },
      "timestamp": "2025-01-15T10:30:00Z"
    },
    {
      "id": "evt_01H8Y...",
      "type": "ReceivedFunds",
      "data": {
        "account_id": "bob",
        "amount": 50.0,
        "sender_id": "alice"
      },
      "timestamp": "2025-01-15T10:30:00Z"
    }
  ],
  "position": 12847
}
```

**Response (rejected):**
```json
{
  "status": "rejected",
  "code": "insufficient_funds",
  "message": "Insufficient funds: available 30.0, requested 50.0"
}
```

**Response (conflict - retry):**
```json
{
  "status": "conflict",
  "message": "Concurrent modification detected, please retry"
}
```

**Headers:**
| Header | Description |
|--------|-------------|
| `X-Idempotency-Key` | Optional. Ensures exactly-once execution. |
| `X-Retry-Count` | Response header indicating internal retry count. |

---

## Schema

### Get Current Schema

```
GET /schema
```

**Response:**
```json
{
  "version": "0.3.0",
  "schema": "event OpenedAccount {\n  @account_id: String\n  ...",
  "events": [
    {
      "name": "OpenedAccount",
      "domain_ids": ["account_id"],
      "fields": [
        {"name": "account_id", "type": "String", "domain_id": true},
        {"name": "initial_balance", "type": "Float", "domain_id": false}
      ]
    }
  ],
  "updated_at": "2025-01-10T08:00:00Z"
}
```

### Update Schema

```
PUT /schema
Content-Type: text/plain
```

**Request Body:** Raw ESDL content

**Response (success):**
```json
{
  "version": "0.4.0",
  "changes": {
    "added_events": ["OverdraftWarning"],
    "modified_events": [],
    "removed_events": []
  }
}
```

**Response (breaking change rejected):**
```json
{
  "status": "rejected",
  "breaking_changes": [
    {
      "event": "SentFunds",
      "change": "removed_field",
      "field": "recipient_id",
      "message": "Cannot remove field 'recipient_id' - 1,847 events exist with this field"
    }
  ],
  "hint": "Use --force flag via CLI to override (creates new schema version)"
}
```

### Validate Schema (dry run)

```
POST /schema/validate
Content-Type: text/plain
```

Same as PUT but doesn't persist. Returns what would change.

---

## Command Handlers

### List Handlers

```
GET /handlers
```

**Response:**
```json
{
  "handlers": [
    {
      "name": "transfer_funds",
      "version": "1.2.0",
      "event_types": ["OpenedAccount", "SentFunds", "ReceivedFunds"],
      "uploaded_at": "2025-01-14T12:00:00Z",
      "executions_24h": 8472
    },
    {
      "name": "open_account",
      "version": "1.0.0",
      "event_types": [],
      "uploaded_at": "2025-01-10T08:00:00Z",
      "executions_24h": 156
    }
  ]
}
```

### Get Handler Details

```
GET /handlers/{command_name}
```

**Response:**
```json
{
  "name": "transfer_funds",
  "version": "1.2.0",
  "event_types": ["OpenedAccount", "SentFunds", "ReceivedFunds"],
  "domain_id_fields": ["account_id"],
  "uploaded_at": "2025-01-14T12:00:00Z",
  "wasm_size_bytes": 245760,
  "sha256": "a1b2c3..."
}
```

### Upload Handler

```
PUT /handlers/{command_name}
Content-Type: application/wasm
```

**Request Body:** Raw WASM binary

**Headers:**
| Header | Description |
|--------|-------------|
| `X-Handler-Version` | Semver version string (required) |

**Response:**
```json
{
  "name": "transfer_funds",
  "version": "1.2.0",
  "event_types": ["OpenedAccount", "SentFunds", "ReceivedFunds"],
  "validation": {
    "status": "ok",
    "warnings": [
      "Handler queries ReceivedFunds but never matches on sender_id"
    ]
  }
}
```

**Validation errors:**
```json
{
  "status": "rejected",
  "errors": [
    {
      "code": "unknown_event_type",
      "message": "Handler emits 'FundsTransferred' which is not in the schema"
    }
  ]
}
```

### Delete Handler

```
DELETE /handlers/{command_name}
```

---

## Events (Admin/Debug)

### Query Events

```
POST /events/query
```

**Request:**
```json
{
  "event_types": ["SentFunds", "ReceivedFunds"],
  "domain_ids": {
    "account_id": ["alice", "bob"]
  },
  "after_position": 0,
  "limit": 100
}
```

**Response:**
```json
{
  "events": [
    {
      "id": "evt_01H8X...",
      "type": "SentFunds",
      "data": { ... },
      "domain_ids": {"account_id": "alice", "recipient_id": "bob"},
      "position": 12845,
      "timestamp": "2025-01-15T10:30:00Z"
    }
  ],
  "next_position": 12847,
  "has_more": true
}
```

### Get Event by ID

```
GET /events/{event_id}
```

---

## Health & Metrics

### Health Check

```
GET /health
```

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "umadb": "connected"
}
```

### Metrics

```
GET /metrics
```

Prometheus format for scraping.

---

## Error Responses

All errors follow this format:

```json
{
  "error": {
    "code": "handler_not_found",
    "message": "No handler registered with name 'withdraw_funds'",
    "request_id": "req_01H8X..."
  }
}
```

**HTTP Status Codes:**

| Code | Meaning |
|------|---------|
| 200 | Success |
| 400 | Invalid request (bad JSON, validation error) |
| 401 | Missing or invalid API key |
| 404 | Handler or event not found |
| 409 | Conflict (concurrent modification, retry) |
| 422 | Command rejected (business rule violation) |
| 500 | Internal error |

---

## SDK Usage Example

```typescript
import { ESRuntime } from '@esruntime/sdk';

const client = new ESRuntime({
  apiKey: process.env.ESRUNTIME_API_KEY,
  baseUrl: 'https://api.esruntime.io/v1'
});

// Execute a command
const result = await client.execute('transfer_funds', {
  source_account: 'alice',
  dest_account: 'bob',
  amount: 50.0
});

if (result.status === 'ok') {
  console.log(`Transferred! Events:`, result.events);
} else if (result.status === 'rejected') {
  console.log(`Rejected: ${result.message}`);
} else if (result.status === 'conflict') {
  // Automatic retry handled by SDK, or manual retry
}
```
