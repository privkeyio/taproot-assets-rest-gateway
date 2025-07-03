# Taproot Assets API Documentation

## Overview

The Taproot Assets API Proxy provides a RESTful interface to interact with the Taproot Assets daemon. All endpoints follow REST conventions and return JSON responses.

## Base URL

```
http://localhost:8080/v1/taproot-assets
```

## Authentication

The proxy handles macaroon authentication internally. Ensure your proxy is configured with the correct macaroon paths.

## Common Response Format

### Success Response
```json
{
  "field1": "value1",
  "field2": "value2"
}
```

### Error Response
```json
{
  "error": "Error description",
  "type": "ErrorType"
}
```

## Endpoints

### System Information

#### Get Info
Returns information about the Taproot Assets daemon.

```http
GET /getinfo
```

**Response:**
```json
{
  "version": "0.3.0",
  "network": "regtest",
  "block_height": 150,
  "block_hash": "..."
}
```

### Asset Management

#### List Assets
Returns all assets managed by the daemon.

```http
GET /assets
```

**Response:**
```json
{
  "assets": [
    {
      "asset_id": "...",
      "asset_type": "NORMAL",
      "amount": "1000",
      "asset_genesis": {
        "name": "MyToken",
        "meta_data": "..."
      }
    }
  ],
  "unconfirmed_transfers": "0",
  "unconfirmed_mints": "0"
}
```

#### Mint Asset
Creates a new asset mint batch.

```http
POST /assets
```

**Request Body:**
```json
{
  "asset": {
    "asset_type": "NORMAL",
    "name": "MyToken",
    "amount": "1000"
  },
  "short_response": true
}
```

**Response:**
```json
{
  "pending_batch": {
    "batch_key": "...",
    "state": "BATCH_STATE_PENDING"
  }
}
```

#### Get Asset Balance
Returns the total balance of all assets.

```http
GET /assets/balance
```

**Response:**
```json
{
  "asset_balances": [
    {
      "asset_id": "...",
      "balance": "1000"
    }
  ]
}
```

### Address Management

#### List Addresses
Returns all Taproot Asset addresses.

```http
GET /addrs
```

**Response:**
```json
{
  "addrs": [
    {
      "encoded": "taprt1...",
      "asset_id": "...",
      "amount": "100"
    }
  ]
}
```

#### Create Address
Creates a new Taproot Asset address.

```http
POST /addrs
```

**Request Body:**
```json
{
  "asset_id": "...",
  "amt": "100"
}
```

**Response:**
```json
{
  "encoded": "taprt1...",
  "asset_id": "...",
  "amount": "100"
}
```

#### Decode Address
Decodes a Taproot Asset address.

```http
POST /addrs/decode
```

**Request Body:**
```json
{
  "addr": "taprt1..."
}
```

### Asset Transfers

#### Send Assets
Sends assets to one or more Taproot Asset addresses.

```http
POST /send
```

**Request Body:**
```json
{
  "tap_addrs": ["taprt1..."],
  "fee_rate": 10
}
```

**Response:**
```json
{
  "transfer_txid": "...",
  "anchor_output_index": 0,
  "transfer": {
    "transfer_timestamp": "1234567890",
    "new_outputs": [...]
  }
}
```

### Minting Process

#### Fund Batch
Funds a pending mint batch.

```http
POST /assets/mint/fund
```

**Request Body:**
```json
{
  "short_response": true,
  "fee_rate": 10
}
```

#### Finalize Batch
Finalizes and broadcasts a funded mint batch.

```http
POST /assets/mint/finalize
```

**Request Body:**
```json
{
  "short_response": true,
  "fee_rate": 10
}
```

#### Cancel Batch
Cancels a pending mint batch.

```http
POST /assets/mint/cancel
```

### Universe Sync

#### List Universe Roots
Returns known universe roots.

```http
GET /universe/roots
```

**Response:**
```json
{
  "universe_roots": [
    {
      "asset_id": "...",
      "root_sum": "..."
    }
  ]
}
```

#### Sync Universe
Synchronizes with a universe server.

```http
POST /universe/sync
```

**Request Body:**
```json
{
  "universe_host": "universe.example.com:10029",
  "sync_mode": "SYNC_ISSUANCE_ONLY"
}
```

### Proofs

#### Export Proof
Exports a proof for a specific asset.

```http
POST /proofs/export
```

**Request Body:**
```json
{
  "asset_id": "...",
  "script_key": "...",
  "outpoint": {
    "txid": "...",
    "output_index": 0
  }
}
```

#### Verify Proof
Verifies an asset proof.

```http
POST /proofs/verify
```

**Request Body:**
```json
{
  "raw_proof_file": "...",
  "genesis_point": "..."
}
```

### Health Checks

#### Health
Basic health check endpoint.

```http
GET /health
```

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

#### Readiness
Checks if the service is ready to handle requests.

```http
GET /readiness
```

**Response:**
```json
{
  "status": "ready",
  "services": {
    "taproot_assets": "up"
  }
}
```

## Error Codes

| Status Code | Description |
|-------------|-------------|
| 200 | Success |
| 400 | Bad Request - Invalid parameters |
| 404 | Not Found - Resource not found |
| 500 | Internal Server Error |
| 502 | Bad Gateway - Cannot connect to tapd |
| 504 | Gateway Timeout - Request timeout |

## Rate Limiting

Currently, no rate limiting is implemented. This may change in future versions.

## Pagination

Some endpoints support pagination using query parameters:
- `offset`: Starting index (default: 0)
- `limit`: Maximum number of results (default: 100)

## WebSocket Support (Coming Soon)

Future versions will support WebSocket connections for real-time updates.

## Examples

### Complete Asset Minting Flow

```bash
# 1. Create a mint batch
curl -X POST http://localhost:8080/v1/taproot-assets/assets \
  -H "Content-Type: application/json" \
  -d '{
    "asset": {
      "asset_type": "NORMAL",
      "name": "TestToken",
      "amount": "1000"
    },
    "short_response": true
  }'

# 2. Fund the batch
curl -X POST http://localhost:8080/v1/taproot-assets/assets/mint/fund \
  -H "Content-Type: application/json" \
  -d '{"short_response": true, "fee_rate": 10}'

# 3. Finalize the batch
curl -X POST http://localhost:8080/v1/taproot-assets/assets/mint/finalize \
  -H "Content-Type: application/json" \
  -d '{"short_response": true, "fee_rate": 10}'

# 4. List your assets
curl http://localhost:8080/v1/taproot-assets/assets
```

### Send Assets

```bash
# 1. Create an address to receive assets
curl -X POST http://localhost:8080/v1/taproot-assets/addrs \
  -H "Content-Type: application/json" \
  -d '{
    "asset_id": "your-asset-id",
    "amt": "100"
  }'

# 2. Send assets to the address
curl -X POST http://localhost:8080/v1/taproot-assets/send \
  -H "Content-Type: application/json" \
  -d '{
    "tap_addrs": ["taprt1..."],
    "fee_rate": 10
  }'
```
