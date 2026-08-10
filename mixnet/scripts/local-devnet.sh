#!/usr/bin/env bash
# Brings up a devnet a browser can talk to and leaves it running: three mix
# nodes, a JSON-RPC exit, and a gateway that carries packets to and from a page.
#
#   ./scripts/local-devnet.sh                       # exit forwards to $UPSTREAM
#   UPSTREAM=http://127.0.0.1:8545 ./scripts/local-devnet.sh
#
# Then open the SDK example (see sdk/README.md) and connect to ws://127.0.0.1:8080.
set -euo pipefail

UPSTREAM="${UPSTREAM:-https://rpc.testnet.robinhood.com}"
BASE_PORT="${BASE_PORT:-9000}"
EXIT_PORT="${EXIT_PORT:-9100}"
GATEWAY_PORT="${GATEWAY_PORT:-8080}"
DELIVERY_PORT="${DELIVERY_PORT:-9200}"

cd "$(dirname "$0")/.."
WORK="$(mktemp -d)"
trap 'kill $(jobs -p) 2>/dev/null || true; rm -rf "$WORK"' EXIT

cargo build --release --quiet
NODE=target/release/erebus-node
CLIENT=target/release/erebus-client
GATEWAY=target/release/erebus-gateway

declare -a IDS=()
for i in 0 1 2; do
  IDS+=("$("$NODE" keygen --out "$WORK/node$i.key")")
done

cat >"$WORK/registry.json" <<JSON
{
  "epoch_seed": "devnet-$(date +%s)",
  "nodes": [
    { "id": "${IDS[0]}", "address": "127.0.0.1:$BASE_PORT", "stake": 1 },
    { "id": "${IDS[1]}", "address": "127.0.0.1:$((BASE_PORT + 1))", "stake": 1 },
    { "id": "${IDS[2]}", "address": "127.0.0.1:$((BASE_PORT + 2))", "stake": 1 }
  ]
}
JSON

for i in 0 1 2; do
  "$NODE" run \
    --key "$WORK/node$i.key" \
    --listen "127.0.0.1:$((BASE_PORT + i))" \
    --registry "$WORK/registry.json" &
done

"$CLIENT" rpc \
  --registry "$WORK/registry.json" \
  --listen "127.0.0.1:$EXIT_PORT" \
  --upstream "$UPSTREAM" &

"$GATEWAY" \
  --registry "$WORK/registry.json" \
  --listen "127.0.0.1:$GATEWAY_PORT" \
  --mix-listen "127.0.0.1:$DELIVERY_PORT" &

cat <<INFO

devnet up
  gateway (browsers)   ws://127.0.0.1:$GATEWAY_PORT
  json-rpc exit        127.0.0.1:$EXIT_PORT  ->  $UPSTREAM
  registry             $WORK/registry.json

ctrl-c to stop everything.
INFO

wait
