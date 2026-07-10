# Integration test harness

The integration tests under `tests/` exercise the gateway against a live
`tapd`, so they need a running `bitcoind` + `lnd` + `tapd` stack. This directory
stands that stack up and runs the suite against it.

The unit tests (`cargo test --lib`) run on every PR; this suite runs nightly and
on demand via the `Integration` workflow, because it takes several minutes and
builds `tapd` from source.

## Run it locally

```bash
./itest/run.sh
```

Run a subset by passing normal `cargo test` arguments:

```bash
./itest/run.sh --test burn -- --test-threads=1
```

The script writes a `.env` at the repo root pointing at the stack, so a manual
`cargo test --test <name>` also works while the stack is up.

### Ports

Defaults are `18443` (bitcoind), `8081` (lnd), `8289` (tapd). If those collide
with a local Polar network, override them:

```bash
BITCOIND_HOST_PORT=28443 LND_HOST_PORT=28081 TAPD_HOST_PORT=28289 ./itest/run.sh
```

## What it does

1. Builds `tapd` at the tag in `Dockerfile.tapd` (default `v0.8.0`) and starts
   `bitcoind` and `lnd`.
2. Loads a wallet and mines an initial block. `lnd` treats the genesis block's
   timestamp as "not synced", so one fresh block flips it to synced, which is
   what lets `tapd` connect.
3. Starts `tapd`, extracts the macaroons, and waits for `getinfo`.
4. Runs the suite. `stop_daemon`, `benchmarks` and `performance` are excluded
   (they shut the node down or are load tests).

Stack logs are written to `itest/compose.log` on exit.
