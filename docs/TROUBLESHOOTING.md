# Taproot Assets REST Gateway - Troubleshooting Guide

This guide covers common issues and their solutions when using the Taproot Assets REST Gateway.

## Table of Contents
- [Connection Issues](#connection-issues)
- [Authentication Problems](#authentication-problems)
- [CORS Errors](#cors-errors)
- [Asset Operations](#asset-operations)
- [Performance Issues](#performance-issues)
- [Docker Issues](#docker-issues)
- [Development Setup](#development-setup)
- [Debugging Tools](#debugging-tools)

## Connection Issues

### Gateway Won't Start

**Symptoms:**
- `Error: Failed to bind to address`
- `Address already in use`

**Solutions:**

1. Check if port is already in use:
```bash
# Linux/Mac
lsof -i :8080

# Windows
netstat -ano | findstr :8080
```

2. Kill the process or change port:
```bash
# Change port in .env
SERVER_ADDRESS=127.0.0.1:8081
```

### Cannot Connect to tapd

**Symptoms:**
- `502 Bad Gateway`
- `Error: connect ECONNREFUSED`
- `Cannot connect to tapd`

**Solutions:**

1. Verify tapd is running:
```bash
# Check if tapd process exists
ps aux | grep tapd

# Check tapd version
tapd --version

# Check tapd REST API directly
curl -k https://localhost:8289/v1/taproot-assets/getinfo
```

2. Verify connection settings:
```bash
# Check your .env
cat .env | grep TAPROOT_ASSETS_HOST

# Should match tapd's REST port (usually 8289)
# Test connection
curl -k https://127.0.0.1:8289/v1/taproot-assets/getinfo
```

3. Check tapd logs:
```bash
# Default location
tail -f ~/.tapd/logs/tapd.log

# Or check tapd output if running in foreground
```

4. Ensure tapd REST API is enabled:
```bash
# In tapd config file
restlisten=0.0.0.0:8289
```

### TLS Certificate Errors

**Symptoms:**
- `certificate signed by unknown authority`
- `x509: certificate is valid for X, not Y`
- `unable to verify the first certificate`

**Solutions:**

1. For development (self-signed certs):
```bash
# In .env
TLS_VERIFY=false
```

2. For production:
```bash
# Ensure TLS_VERIFY=true
# Use proper certificates
# Or add tapd cert to trusted store
```

3. Certificate mismatch:
```bash
# Check certificate details
openssl s_client -connect localhost:8289 -showcerts

# Ensure hostname matches certificate
```

## Authentication Problems

### Invalid Macaroon

**Symptoms:**
- `401 Unauthorized`
- `invalid macaroon`
- `macaroon validation failed`

**Solutions:**

1. Verify macaroon path:
```bash
# Check file exists
ls -la /path/to/admin.macaroon

# Check it's readable
file /path/to/admin.macaroon
```

2. Use absolute paths:
```bash
# Wrong
TAPD_MACAROON_PATH=./admin.macaroon

# Right
TAPD_MACAROON_PATH=/home/user/.tapd/data/regtest/admin.macaroon
```

3. Ensure using admin macaroon:
```bash
# Must be admin.macaroon, not:
# - readonly.macaroon
# - invoice.macaroon
```

4. Check file permissions:
```bash
# Should be readable by gateway user
chmod 644 /path/to/admin.macaroon

# For security, better to use:
chmod 600 /path/to/admin.macaroon
# And run gateway as same user
```

5. Regenerate macaroon if corrupted:
```bash
# Stop tapd
# Delete macaroon
rm ~/.tapd/data/regtest/admin.macaroon
# Restart tapd (will regenerate)
```

### Macaroon Not Found

**Symptoms:**
- `Failed to read tapd macaroon`
- `ENOENT: no such file or directory`

**Solutions:**

1. Find macaroon location:
```bash
# Use our helper script
./scripts/find-macaroons.sh

# Or search manually
find ~ -name "admin.macaroon" -type f 2>/dev/null
```

2. Common locations:
```bash
# Polar
~/.polar/networks/{id}/volumes/tapd/{node}/data/{network}/admin.macaroon

# Standard tapd
~/.tapd/data/{network}/admin.macaroon

# Docker
/var/lib/docker/volumes/{volume}/_data/admin.macaroon
```

## CORS Errors

### Blocked by CORS Policy

**Symptoms:**
- `Access-Control-Allow-Origin' header is missing`
- `CORS policy: No 'Access-Control-Allow-Origin'`
- `Cross-Origin Request Blocked`

**Solutions:**

1. Add your frontend URL:
```bash
# In .env
CORS_ORIGINS=https://myapp.com,https://localhost:3000,http://localhost:3000

# Multiple origins comma-separated
# Include protocol (http:// or https://)
```

2. For development with wildcards:
```bash
# NOT recommended for production
CORS_ORIGINS=*
```

3. Restart gateway after changes:
```bash
docker-compose restart
# or
pkill -f taproot-assets-rest-gateway
cargo run --release
```

4. Check headers:
```bash
# Verify CORS headers are present
curl -I -X OPTIONS http://localhost:8080/v1/taproot-assets/assets \
  -H "Origin: http://localhost:3000" \
  -H "Access-Control-Request-Method: GET"
```

### Preflight Request Failed

**Symptoms:**
- `OPTIONS` request fails
- `Method not allowed`

**Solutions:**

1. Gateway automatically handles OPTIONS
2. Check middleware order isn't blocking
3. Verify no proxy interfering

## Asset Operations

### Assets Not Appearing After Mint

**Symptoms:**
- Minted asset but list shows empty
- `assets` array is empty
- Batch shows complete but no assets

**Solutions:**

1. Wait for confirmations:
```bash
# Assets need blockchain confirmations
# Check current block height
curl http://localhost:8080/v1/taproot-assets/getinfo | jq .block_height

# For regtest, mine blocks manually
bitcoin-cli -regtest generatetoaddress 10 <address>
```

2. Check batch status:
```bash
# List all batches
curl http://localhost:8080/v1/taproot-assets/assets/mint/batches | jq

# Check specific batch
curl http://localhost:8080/v1/taproot-assets/assets/mint/batches/{batch_key} | jq
```

3. Ensure Polar auto-mining is enabled:
- In Polar UI: Settings → Auto Mining
- Set to ~30 seconds

4. Check for errors in batch:
```bash
# Look for state
# Should be BATCH_STATE_FINALIZED
```

### Send Transaction Fails

**Symptoms:**
- `insufficient funds`
- `no matching assets`
- `failed to find input assets`

**Solutions:**

1. Verify asset balance:
```bash
curl http://localhost:8080/v1/taproot-assets/assets/balance | jq
```

2. Decode recipient address:
```bash
curl -X POST http://localhost:8080/v1/taproot-assets/addrs/decode \
  -H "Content-Type: application/json" \
  -d '{"addr": "tapbc1..."}' | jq
```

3. Ensure address matches asset:
- Asset ID in address must match your asset
- Can't send wrong asset type to address

4. Check for locked UTXOs:
```bash
curl http://localhost:8080/v1/taproot-assets/assets/utxos | jq
```

### Mint Batch Stuck

**Symptoms:**
- Batch in `BATCH_STATE_PENDING` forever
- Can't create new batches
- `batch already exists`

**Solutions:**

1. Cancel pending batch:
```bash
curl -X POST http://localhost:8080/v1/taproot-assets/assets/mint/cancel \
  -H "Content-Type: application/json" \
  -d '{}'
```

2. Check for funding issues:
- Ensure LND has sufficient balance
- Check fee rate isn't too low

3. Manual state progression:
```bash
# Fund batch
curl -X POST http://localhost:8080/v1/taproot-assets/assets/mint/fund \
  -H "Content-Type: application/json" \
  -d '{"short_response": true, "fee_rate": 20}'

# Wait for confirmation, then finalize
curl -X POST http://localhost:8080/v1/taproot-assets/assets/mint/finalize \
  -H "Content-Type: application/json" \
  -d '{"short_response": true, "fee_rate": 20}'
```

## Performance Issues

### Slow Response Times

**Symptoms:**
- Requests take several seconds
- Timeouts on operations
- UI feels sluggish

**Solutions:**

1. Check tapd performance:
```bash
# Monitor CPU and memory
top -p $(pgrep tapd)

# Check tapd sync status
curl http://localhost:8080/v1/taproot-assets/getinfo | jq
```

2. Increase timeouts:
```bash
# In .env
REQUEST_TIMEOUT_SECS=60
```

3. Enable debug logging:
```bash
# Check where time is spent
RUST_LOG=debug cargo run
```

4. Database optimization:
```bash
# If tapd database is large
# Consider pruning old proofs
```

### High Memory Usage

**Symptoms:**
- Gateway using excessive RAM
- Out of memory errors
- System slowdown

**Solutions:**

1. Check for memory leaks:
```bash
# Monitor over time
docker stats

# Or use htop
htop -p $(pgrep taproot-assets)
```

2. Limit connections:
- Implement connection pooling
- Rate limit more aggressively

3. Restart periodically:
```bash
# Add to cron for daily restart
0 3 * * * docker-compose restart taproot-assets-gateway
```

## Docker Issues

### Container Won't Start

**Symptoms:**
- `docker-compose up` fails
- Container exits immediately
- No logs produced

**Solutions:**

1. Check logs:
```bash
docker-compose logs -f taproot-assets-gateway
```

2. Verify volume mounts:
```bash
# Ensure macaroon paths exist
ls -la /path/to/macaroons

# Check docker can access them
docker run --rm -v /path/to/macaroons:/test alpine ls -la /test
```

3. Debug interactively:
```bash
# Run with shell
docker-compose run --rm taproot-assets-gateway /bin/bash

# Then try running manually
taproot-assets-gateway
```

### Permission Denied in Container

**Symptoms:**
- `Permission denied` accessing macaroons
- Can't read mounted files

**Solutions:**

1. Check file ownership:
```bash
# See who owns files
ls -la /path/to/macaroons

# See container user
docker-compose exec taproot-assets-gateway id
```

2. Fix permissions:
```bash
# Make readable by container
chmod 644 /path/to/admin.macaroon

# Or run container as your user
# In docker-compose.yml:
user: "${UID}:${GID}"
```

3. Use Docker secrets (production):
```yaml
secrets:
  tapd_macaroon:
    file: ./secrets/admin.macaroon
```

### Container Can't Reach tapd

**Symptoms:**
- `Connection refused` from container
- Works on host but not in Docker

**Solutions:**

1. Use host network:
```bash
# In docker-compose.yml
network_mode: host
```

2. Or use container names:
```bash
# If tapd in container
TAPROOT_ASSETS_HOST=tapd-container:8289
```

3. Check network:
```bash
# List networks
docker network ls

# Inspect network
docker network inspect bridge
```

## Development Setup

### Polar Issues

**Common problems and solutions:**

1. Nodes not starting:
- Check Docker is running
- Ensure sufficient disk space
- Reset Polar data: Help → Reset

2. Can't find macaroons:
- Wait for nodes to fully start
- Check Polar logs for errors
- Use absolute paths in .env

3. Mining not working:
- Enable auto-mining in settings
- Or manually mine via terminal

### Test Environment

**Setting up for tests:**

1. Configure test environment:
```bash
# .env
BITCOIN_RPC_URL=http://127.0.0.1:18443
BITCOIN_RPC_USER=polaruser
BITCOIN_RPC_PASS=polarpass
```

2. Common test failures:
- Ensure auto-mining enabled
- Wait between operations
- Check sufficient balance

## Debugging Tools

### Enable Debug Logging

```bash
# Maximum verbosity
RUST_LOG=trace cargo run

# Just gateway debug
RUST_LOG=taproot_assets=debug cargo run

# With timestamps
RUST_LOG=taproot_assets=debug,timestamp=on cargo run
```

### Test Individual Endpoints

```bash
# Create test script
cat > test-endpoint.sh << 'EOF'
#!/bin/bash
ENDPOINT=$1
METHOD=${2:-GET}
DATA=${3:-}

if [ -z "$DATA" ]; then
  curl -X $METHOD http://localhost:8080/v1/taproot-assets/$ENDPOINT | jq
else
  curl -X $METHOD http://localhost:8080/v1/taproot-assets/$ENDPOINT \
    -H "Content-Type: application/json" \
    -d "$DATA" | jq
fi
EOF

chmod +x test-endpoint.sh

# Usage
./test-endpoint.sh getinfo
./test-endpoint.sh assets
./test-endpoint.sh addrs POST '{"asset_id":"...", "amt":"100"}'
```

### Monitor Gateway Logs

```bash
# Follow logs
docker-compose logs -f --tail=100

# Filter errors only
docker-compose logs -f | grep -E "(ERROR|WARN)"

# Save to file for analysis
docker-compose logs > gateway.log
```

### Check Gateway Metrics

```bash
# Simple health dashboard
watch -n 2 'echo "=== Health ===" && \
  curl -s http://localhost:8080/health | jq && \
  echo -e "\n=== Readiness ===" && \
  curl -s http://localhost:8080/readiness | jq && \
  echo -e "\n=== Info ===" && \
  curl -s http://localhost:8080/v1/taproot-assets/getinfo | jq ".version,.network"'
```

## Getting Help

If problems persist:

1. **Collect Information:**
   - Gateway logs
   - tapd logs
   - Configuration (sanitized)
   - Error messages
   - Steps to reproduce

2. **Check Resources:**
   - [GitHub Issues](https://github.com/privkeyio/taproot-assets-rest-gateway/issues)
   - [API Reference](API_REFERENCE.md)
   - [Examples](../examples/)

3. **Open Issue with:**
   - Clear problem description
   - Environment details
   - Logs and errors
   - What you've tried

Remember: Most issues are configuration-related. Double-check paths, ports, and permissions first!
