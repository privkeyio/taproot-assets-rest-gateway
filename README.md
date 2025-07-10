# Taproot Assets REST Gateway

A lightweight REST proxy that makes Lightning Labs' Taproot Assets daemon accessible to web applications by adding CORS support and simplifying authentication.

## Disclaimer

⚠️ **EXPERIMENTAL SOFTWARE**: This gateway has only been tested on regtest 
with Polar. It is NOT recommended for mainnet use. Use at your own risk!

This is community-developed software that interfaces with Lightning Labs' 
Taproot Assets daemon. It is not affiliated with or endorsed by Lightning Labs.

## The Problem

Lightning Labs' `tapd` REST API (port 8089) doesn't support CORS, making it impossible to use directly from web browsers. Additionally, managing macaroons and TLS certificates adds complexity for developers who just want to integrate Taproot Assets into their applications.

## The Solution

This gateway acts as a proxy between your web application and `tapd`, handling:

- **CORS headers** - Enables browser-based applications
- **Macaroon authentication** - No manual base64 encoding or header management
- **TLS complexity** - Configurable verification for development
- **Better error messages** - Meaningful errors instead of raw gRPC codes
- **Request tracking** - UUID for each request aids debugging

## Quick Start

```bash
# Clone and configure
git clone https://github.com/yourusername/taproot-assets-rest-gateway.git
cd taproot-assets-rest-gateway
cp .env.example .env

# Edit .env with your tapd details
# Run with Docker
docker-compose up -d

# Or run directly
cargo run --release
```

## Why Use This?

If you're building a web app that needs Taproot Assets, you have three options:

1. **Use tapd's gRPC API** - Requires gRPC-web, complex for browsers
2. **Use tapd's REST API directly** - No CORS support, won't work from browsers  
3. **Use this gateway** - Works immediately from any web app

## Usage Examples

### Curl: Mint New Asset

```bash
curl -X POST http://localhost:8080/v1/taproot-assets/assets \
  -H "Content-Type: application/json" \
  -d '{
    "asset": {
      "asset_type": "NORMAL",
      "name": "MyToken",
      "amount": "1000"
    },
    "short_response": true
  }'
```

### JavaScript/TypeScript

```javascript
// No authentication headers needed - the gateway handles it
const GATEWAY_URL = 'http://localhost:8080';

// List your assets
async function listAssets() {
  const response = await fetch(`${GATEWAY_URL}/v1/taproot-assets/assets`);
  const data = await response.json();
  return data.assets;
}

// Create a new address
async function createAddress(assetId, amount) {
  const response = await fetch(`${GATEWAY_URL}/v1/taproot-assets/addrs`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      asset_id: assetId,
      amt: amount.toString()
    })
  });
  return response.json();
}

// Send assets
async function sendAssets(toAddress, feeRate = 5) {
  const response = await fetch(`${GATEWAY_URL}/v1/taproot-assets/send`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      tap_addrs: [toAddress],
      fee_rate: feeRate
    })
  });
  return response.json();
}
```

### Python

```python
import requests

GATEWAY_URL = 'http://localhost:8080'

# No need to handle macaroons or TLS certificates
def list_assets():
    response = requests.get(f'{GATEWAY_URL}/v1/taproot-assets/assets')
    return response.json()['assets']

def mint_asset(name, amount):
    response = requests.post(
        f'{GATEWAY_URL}/v1/taproot-assets/assets',
        json={
            'asset': {
                'asset_type': 'NORMAL',
                'name': name,
                'amount': str(amount)
            },
            'short_response': True
        }
    )
    return response.json()

def get_balance():
    response = requests.get(f'{GATEWAY_URL}/v1/taproot-assets/assets/balance')
    return response.json()
```

### Go

```go
package main

import (
    "bytes"
    "encoding/json"
    "fmt"
    "net/http"
)

const gatewayURL = "http://localhost:8080"

type Asset struct {
    AssetID string `json:"asset_id"`
    Amount  string `json:"amount"`
}

func listAssets() ([]Asset, error) {
    resp, err := http.Get(gatewayURL + "/v1/taproot-assets/assets")
    if err != nil {
        return nil, err
    }
    defer resp.Body.Close()
    
    var result struct {
        Assets []Asset `json:"assets"`
    }
    
    if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
        return nil, err
    }
    
    return result.Assets, nil
}
```

### React Hook Example

```typescript
import { useState, useEffect } from 'react';

const GATEWAY_URL = process.env.REACT_APP_GATEWAY_URL || 'http://localhost:8080';

export function useTaprootAssets() {
  const [assets, setAssets] = useState([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetchAssets();
  }, []);

  const fetchAssets = async () => {
    try {
      const response = await fetch(`${GATEWAY_URL}/v1/taproot-assets/assets`);
      const data = await response.json();
      setAssets(data.assets || []);
    } catch (error) {
      console.error('Failed to fetch assets:', error);
    } finally {
      setLoading(false);
    }
  };

  const sendAssets = async (address: string, amount: string) => {
    const response = await fetch(`${GATEWAY_URL}/v1/taproot-assets/send`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        tap_addrs: [address],
        fee_rate: 5
      })
    });
    
    if (!response.ok) {
      throw new Error(`Send failed: ${response.statusText}`);
    }
    
    return response.json();
  };

  return { assets, loading, sendAssets, refetch: fetchAssets };
}
```

## .env Configuration

```env
# Required
TAPROOT_ASSETS_HOST=127.0.0.1:8289
TAPD_MACAROON_PATH=/path/to/tapd/admin.macaroon
LND_MACAROON_PATH=/path/to/lnd/admin.macaroon

# Security (use true in production)
TLS_VERIFY=false

# CORS - Add your app's URL
CORS_ORIGINS=http://localhost:3000,http://localhost:5173

# Optional
SERVER_ADDRESS=127.0.0.1:8080
REQUEST_TIMEOUT_SECS=30
RATE_LIMIT_PER_MINUTE=100
```

## Testing Requirements
- Bitcoin Core with RPC enabled (for integration tests)
- Set BITCOIN_RPC_USER and BITCOIN_RPC_PASS
- LND & tapd running
- Or use Polar for easier setup

## Features

### What Works Now
- ✅ Complete REST API coverage for tapd
- ✅ CORS support for web browsers
- ✅ Automatic macaroon authentication
- ✅ Request ID tracking
- ✅ Basic rate limiting
- ✅ Docker support
- ✅ Health check endpoints

### What's Missing
- 🚧 WebSocket support for real-time events (in progress)
- ❌ Response caching
- ❌ Metrics/monitoring endpoints
- ❌ Load balancing for multiple tapd instances
- ❌ Advanced rate limiting (per endpoint/user)

## Development Setup

1. **Install Polar** for local Lightning development
2. **Create a network** with at least one LND node
3. **Enable Taproot Assets** on the node
4. **Find your macaroons**:
   ```bash
   # Use the helper script
   ./scripts/find-macaroons.sh
   ```
5. **Configure `.env`** with the paths
6. **Run tests**: `cargo test`

## Architecture

```
Web App → REST Gateway → tapd gRPC/REST
         ↓
   [CORS Headers]
   [Macaroon Auth]
   [Rate Limiting]
   [Error Handling]
```

The gateway forwards requests to tapd's REST API (port 8089) while adding the necessary headers and authentication that web browsers require.

## Limitations

- Only exposes endpoints available in tapd's REST API
- Some advanced gRPC-only features not accessible
- Rate limiting is basic (per-IP only)
- No built-in caching or response optimization
- Requires local access to macaroon files

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for details.
This project needs:
- Production hardening
- Better error messages
- WebSocket support for events
- Caching layer
- More comprehensive tests

## License

MIT

## Comparison with Alternatives

| Feature | Direct gRPC | Direct REST | This Gateway |
|---------|------------|-------------|--------------|
| Browser Support | ❌ Complex setup | ❌ No CORS | ✅ Native |
| Authentication | Manual | Manual | ✅ Automatic |
| Error Messages | gRPC codes | gRPC codes | ✅ Friendly |
| Setup Complexity | High | Medium | ✅ Low |

## Security Considerations

- Never expose this gateway to the public internet without proper authentication
- The gateway has full access to your tapd node
- Use `TLS_VERIFY=true` in any production setting
- Secure your macaroon files with appropriate permissions
- Consider running behind a reverse proxy with additional security

## Not Production Ready

This is experimental software for developers who want to quickly prototype Taproot Assets integrations. For production use, consider:
- Adding authentication to the gateway itself
- Implementing proper monitoring and alerting
- Running multiple instances behind a load balancer
- Regular security audits
- Macaroon rotation strategies

---

Built by developers who just wanted to use Taproot Assets from a web app without the complexity.
