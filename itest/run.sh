#!/usr/bin/env bash
# Stand up a bitcoind + lnd + tapd regtest stack and run the integration suite
# against it. The tests self-bootstrap (fund lnd, mine, mint), so this only has
# to bring the services up, load a wallet, and mine an initial block so lnd
# reports synced and tapd can connect.
#
# Usage: ./run.sh [extra cargo-test args...]
#   With no args it runs every integration test except the load/shutdown ones.
set -euo pipefail

ITEST_DIR="$(cd "$(dirname "$0")" && pwd)"
# Absolute compose file so cleanup works after the script cd's into the repo root.
COMPOSE="docker compose -f $ITEST_DIR/docker-compose.yml"
ARTIFACTS="$(mktemp -d)"
cd "$ITEST_DIR"

BITCOIND_HOST_PORT="${BITCOIND_HOST_PORT:-18443}"
LND_HOST_PORT="${LND_HOST_PORT:-8081}"
TAPD_HOST_PORT="${TAPD_HOST_PORT:-8289}"
export BITCOIND_HOST_PORT LND_HOST_PORT TAPD_HOST_PORT

bc() { $COMPOSE exec -T bitcoind bitcoin-cli -regtest -rpcuser=polaruser -rpcpassword=polarpass "$@"; }

MINER_PID=""
FUNDER_PID=""
cleanup() {
  [ -n "$MINER_PID" ] && kill "$MINER_PID" 2>/dev/null || true
  [ -n "$FUNDER_PID" ] && kill "$FUNDER_PID" 2>/dev/null || true
  $COMPOSE logs --no-color >"$ITEST_DIR/compose.log" 2>&1 || true
  $COMPOSE down -v --remove-orphans || true
}
trap cleanup EXIT

echo "==> Building tapd image and starting bitcoind + lnd"
$COMPOSE up -d --build --wait --wait-timeout 900 bitcoind lnd

echo "==> Loading a wallet and mining an initial block"
# lnd treats the ancient genesis timestamp as "not synced"; one fresh block with
# a current timestamp flips it to synced, which is what unblocks tapd.
bc createwallet itest >/dev/null 2>&1 || bc loadwallet itest >/dev/null 2>&1 || true
ADDR="$(bc getnewaddress)"
# 110 blocks so the coinbase from the first block is mature (>100 confs) and
# there are ample spendable outputs to fund lnd from.
bc generatetoaddress 110 "$ADDR" >/dev/null

echo "==> Starting tapd"
$COMPOSE up -d --wait --wait-timeout 300 tapd

echo "==> Extracting macaroons"
$COMPOSE cp tapd:/home/tap/.tapd/data/regtest/admin.macaroon "$ARTIFACTS/tapd.macaroon"
$COMPOSE cp lnd:/home/lnd/.lnd/data/chain/bitcoin/regtest/admin.macaroon "$ARTIFACTS/lnd.macaroon"

echo "==> Waiting for tapd getinfo"
MAC_HEX="$(xxd -p -c 100000 "$ARTIFACTS/tapd.macaroon")"
for i in $(seq 1 60); do
  if curl -skf -H "Grpc-Metadata-macaroon: $MAC_HEX" \
      "https://127.0.0.1:${TAPD_HOST_PORT}/v1/taproot-assets/getinfo" >/dev/null; then
    echo "    tapd is up"; break
  fi
  [ "$i" = 60 ] && { echo "tapd never became reachable"; exit 1; }
  sleep 2
done

cat >../.env <<ENV
TAPROOT_ASSETS_HOST=127.0.0.1:${TAPD_HOST_PORT}
TAPD_MACAROON_PATH=$ARTIFACTS/tapd.macaroon
LND_MACAROON_PATH=$ARTIFACTS/lnd.macaroon
LND_URL=https://127.0.0.1:${LND_HOST_PORT}
TLS_VERIFY=false
CORS_ORIGINS=http://localhost:5173,http://127.0.0.1:5173,http://localhost:3000
SERVER_ADDRESS=127.0.0.1:8080
RUST_LOG=warn
REQUEST_TIMEOUT_SECS=30
RATE_LIMIT_PER_MINUTE=10000
BITCOIN_RPC_URL=http://127.0.0.1:${BITCOIND_HOST_PORT}
BITCOIN_RPC_USER=polaruser
BITCOIN_RPC_PASS=polarpass
ENV

LND_MAC_HEX="$(xxd -p -c 100000 "$ARTIFACTS/lnd.macaroon")"

# Keep lnd supplied with fresh, unlocked on-chain UTXOs. Asset sends anchor with
# lnd's wallet, and the psbt tests lease UTXOs, so a single large funding output
# gets locked and lnd's spendable balance drops to zero mid-suite. Trickling in
# separate outputs keeps some unlocked.
fund_lnd() {
  local a
  a="$(curl -sk -H "Grpc-Metadata-macaroon: $LND_MAC_HEX" \
        "https://127.0.0.1:${LND_HOST_PORT}/v1/newaddress" \
        | python3 -c 'import sys,json;print(json.load(sys.stdin).get("address",""))' 2>/dev/null)"
  [ -n "$a" ] && bc sendtoaddress "$a" 5 >/dev/null 2>&1 || true
}
# Seed several outputs before the suite starts.
for _ in $(seq 1 5); do fund_lnd; done

# Mine a block periodically so mints, sends and fresh funding confirm during the
# run, and keep topping lnd up so it never runs out of spendable coins.
( while true; do bc generatetoaddress 1 "$ADDR" >/dev/null 2>&1 || true; sleep 5; done ) &
MINER_PID=$!
( while true; do fund_lnd; sleep 20; done ) &
FUNDER_PID=$!

echo "==> Running integration suite"
cd ..
# Capture the suite's exit code and re-exit with it so a test failure fails the
# job. set -e does not fire here because the result is consumed by the trap.
rc=0
if [ "$#" -gt 0 ]; then
  cargo test "$@" || rc=$?
else
  # stop_daemon shuts the node down mid-run; benchmarks/performance are load tests.
  TARGETS=$(ls tests/*.rs | sed 's|.*/||;s|\.rs||' \
    | grep -vE '^(stop_daemon|benchmarks|performance)$' \
    | sed 's/^/--test /' | tr '\n' ' ')
  cargo test $TARGETS --no-fail-fast -- --test-threads=1 || rc=$?
fi
exit "$rc"
