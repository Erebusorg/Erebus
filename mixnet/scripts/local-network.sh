#!/usr/bin/env bash
# Brings up a three node mixnet on loopback, sends one message through it, and
# tears everything down.
#
#   ./scripts/local-network.sh "buy 10 AAPL"
set -euo pipefail

MESSAGE="${1:-buy 10 AAPL}"
MEAN_DELAY_MS="${MEAN_DELAY_MS:-50}"
BASE_PORT="${BASE_PORT:-9000}"
SINK_PORT="${SINK_PORT:-9100}"

cd "$(dirname "$0")/.."
WORK="$(mktemp -d)"
trap 'kill $(jobs -p) 2>/dev/null || true; rm -rf "$WORK"' EXIT

cargo build --release --quiet
NODE=target/release/erebus-node
CLIENT=target/release/erebus-client

echo "generating three node keys"
declare -a IDS=()
for i in 0 1 2; do
  IDS+=("$("$NODE" keygen --out "$WORK/node$i.key")")
done

cat >"$WORK/registry.json" <<JSON
{
  "epoch_seed": "local-$(date +%s)",
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

"$CLIENT" sink --registry "$WORK/registry.json" --listen "127.0.0.1:$SINK_PORT" &

# Give the listeners a moment before the first packet arrives.
sleep 1

echo "sending through three hops, mean delay ${MEAN_DELAY_MS}ms per hop"
"$CLIENT" send \
  --registry "$WORK/registry.json" \
  --to "127.0.0.1:$SINK_PORT" \
  --message "$MESSAGE" \
  --mean-delay-ms "$MEAN_DELAY_MS"

"$CLIENT" probe \
  --registry "$WORK/registry.json" \
  --mean-delay-ms "$MEAN_DELAY_MS"
