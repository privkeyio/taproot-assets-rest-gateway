# Taproot Assets REST Gateway Tests

This directory contains integration tests for the REST gateway.

## Test Setup

### Prerequisites

1. **Polar Lightning Network**: Install from https://lightningpolar.com/
2. **Bitcoin Core RPC Access**: Tests need to mine blocks
3. **Running tapd instance**: With Taproot Assets enabled

### Configuration

Create a `.env` file in the root directory with test configuration:

```env
# Your standard configuration
TAPROOT_ASSETS_HOST=127.0.0.1:8289
TAPD_MACAROON_PATH=/path/to/tapd/admin.macaroon
LND_MACAROON_PATH=/path/to/lnd/admin.macaroon
TLS_VERIFY=false  # For Polar development

# Test-specific configuration
BITCOIN_RPC_URL=http://127.0.0.1:18443
BITCOIN_RPC_USER=polaruser
BITCOIN_RPC_PASS=polarpass
LND_URL=https://127.0.0.1:8080
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test module
cargo test api::assets

# Run with debug logging
RUST_LOG=debug cargo test -- --nocapture

# Run tests serially (recommended for integration tests)
cargo test -- --test-threads=1
```

## Test Structure

- `setup.rs` - Common test setup and utilities
- `test_utils.rs` - Helper functions for tests
- Integration tests in `tests/` directory test actual API endpoints

## Writing Tests

Tests should:
1. Use the common setup from `setup.rs`
2. Clean up after themselves (cancel mints, etc.)
3. Be independent of other tests
4. Handle async operations properly

Example test:
```rust
#[tokio::test]
async fn test_list_assets() {
    let (client, base_url, macaroon) = setup::setup().await;
    
    // Your test logic here
    let assets = api::assets::list_assets(&client, &base_url, &macaroon).await;
    assert!(assets.is_ok());
}
```

## Troubleshooting

### Tests Hanging
- Ensure Polar is running with auto-mining enabled
- Check that tapd is responsive
- Verify RPC credentials are correct

### Connection Errors
- Set `TLS_VERIFY=false` for local development
- Check firewall settings
- Verify all services are on the same network

### Asset Not Found
- Tests may need to wait for block confirmations
- Ensure auto-mining is enabled in Polar
- Check tapd logs for errors
