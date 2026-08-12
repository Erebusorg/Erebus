#!/usr/bin/env bash
# Brings up a mixnet whose node set comes from the registry contract rather than
# a file: a local chain, a deployed NodeRegistry, three nodes that stake and
# register themselves, and a client that reads the set back off the chain.
#
#   ./scripts/chain-devnet.sh          # runs a probe and a request, then exits
#   KEEP=1 ./scripts/chain-devnet.sh   # leaves the chain and the nodes running
#
# Needs foundry (anvil, forge, cast): https://getfoundry.sh
set -euo pipefail

CHAIN_PORT="${CHAIN_PORT:-8545}"
BASE_PORT="${BASE_PORT:-9000}"
EXIT_PORT="${EXIT_PORT:-9100}"
MIN_STAKE="${MIN_STAKE:-1000000000000000}" # 0.001 ether
EPOCH="${EPOCH:-3600}"

# anvil's first account, which is public knowledge and funded only on this chain.
DEPLOYER_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
DEPLOYER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

for tool in anvil forge cast; do
  command -v "$tool" >/dev/null || { echo "$tool is not on PATH — see https://getfoundry.sh" >&2; exit 1; }
done

cd "$(dirname "$0")/.."
WORK="$(mktemp -d)"
trap 'kill $(jobs -p) 2>/dev/null || true; rm -rf "$WORK"' EXIT

RPC="http://127.0.0.1:$CHAIN_PORT"
anvil --port "$CHAIN_PORT" >"$WORK/anvil.log" 2>&1 &
until cast block-number --rpc-url "$RPC" >/dev/null 2>&1; do sleep 0.2; done

echo "deploying the registry"
REGISTRY="$(
  forge create ../contracts/src/NodeRegistry.sol:NodeRegistry \
    --root ../contracts \
    --rpc-url "$RPC" \
    --private-key "$DEPLOYER_KEY" \
    --broadcast \
    --json \
    --constructor-args "$MIN_STAKE" 604800 "$EPOCH" "$DEPLOYER" "$DEPLOYER" \
  | sed -n 's/.*"deployedTo": *"\([^"]*\)".*/\1/p'
)"
[[ -n "$REGISTRY" ]] || { echo "the registry did not deploy" >&2; exit 1; }
echo "  NodeRegistry $REGISTRY"

cargo build --release --quiet
NODE=target/release/erebus-node
CLIENT=target/release/erebus-client
LOOKUP=target/release/erebus-registry

for i in 0 1 2; do
  ID="$("$NODE" keygen --out "$WORK/node$i.key")"
  cast send "$REGISTRY" "register(bytes32,string)" "0x$ID" "127.0.0.1:$((BASE_PORT + i))" \
    --value "$MIN_STAKE" --rpc-url "$RPC" --private-key "$DEPLOYER_KEY" >/dev/null
  echo "registered $ID at 127.0.0.1:$((BASE_PORT + i))"
done

echo
echo "the node set, as any client reads it:"
"$LOOKUP" fetch --rpc "$RPC" --contract "$REGISTRY"

for i in 0 1 2; do
  "$NODE" run \
    --key "$WORK/node$i.key" \
    --listen "127.0.0.1:$((BASE_PORT + i))" \
    --chain-rpc "$RPC" --contract "$REGISTRY" &
done

"$CLIENT" sink --chain-rpc "$RPC" --contract "$REGISTRY" --listen "127.0.0.1:$EXIT_PORT" &
sleep 2

echo
"$CLIENT" probe --chain-rpc "$RPC" --contract "$REGISTRY" --mean-delay-ms 20
"$CLIENT" send \
  --chain-rpc "$RPC" --contract "$REGISTRY" \
  --to "127.0.0.1:$EXIT_PORT" \
  --message "buy 10 AAPL" \
  --mean-delay-ms 20

if [[ -n "${KEEP:-}" ]]; then
  echo
  echo "chain $RPC, registry $REGISTRY — ctrl-c to stop everything."
  wait
fi
