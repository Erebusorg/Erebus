#!/usr/bin/env bash
# A paid round through the mixnet, end to end on a local chain: a registry, a
# shielded fee pool, three nodes that stake and register themselves, a request
# through all three, and a fee that pays them without saying who paid.
#
#   ./scripts/paid-round.sh          # runs the round, then exits
#   KEEP=1 ./scripts/paid-round.sh   # leaves the chain and the nodes running
#
# What to watch for: the deposit comes from one account, the spend is submitted
# by another, and nothing on chain links them. The nodes end up with money and no
# way to tell which of the pool's deposits paid for it.
#
# Needs foundry (anvil, forge, cast) and jq.
set -euo pipefail

CHAIN_PORT="${CHAIN_PORT:-8545}"
BASE_PORT="${BASE_PORT:-9000}"
EXIT_PORT="${EXIT_PORT:-9100}"
MIN_STAKE="${MIN_STAKE:-1000000000000000}"  # 0.001 ether
DENOMINATION="${DENOMINATION:-10000000000000000}" # 0.01 ether
EPOCH="${EPOCH:-3600}"

# anvil's default accounts: public knowledge, funded only on this chain.
DEPLOYER_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
DEPLOYER=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
# One key per node operator, so a payout is visibly not the deployer's money.
OPERATOR_KEYS=(
  0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
  0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a
  0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6
)
OPERATORS=(
  0x70997970C51812dc3A010C7d01b50e0d17dc79C8
  0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
  0x90F79bf6EB2c4f870365E785982E1f101E93b906
)
# The payer funds a note. The relayer submits the spend. They are different
# accounts on purpose: that separation is the product.
PAYER_KEY=0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a
DECOY_KEYS=(
  0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba
  0x92db14e403b83dfe3df233f83dfa3a0d7096f21ca9b0d6d6b8d88b2b4ec1564e
)
RELAYER_KEY=0x4bbbf85ce3377467afe5d46f804f221813b2bb87f24d81f60f1fcdbf7cbf4356

for tool in anvil forge cast jq; do
  command -v "$tool" >/dev/null || { echo "$tool is not on PATH" >&2; exit 1; }
done

cd "$(dirname "$0")/.."
WORK="$(mktemp -d)"
trap 'kill $(jobs -p) 2>/dev/null || true; rm -rf "$WORK"' EXIT

RPC="http://127.0.0.1:$CHAIN_PORT"
anvil --port "$CHAIN_PORT" >"$WORK/anvil.log" 2>&1 &
until cast block-number --rpc-url "$RPC" >/dev/null 2>&1; do sleep 0.2; done
CHAIN_ID="$(cast chain-id --rpc-url "$RPC")"

deploy() { # contract, constructor args...
  local what="$1"; shift
  forge create "$what" \
    --root ../contracts --rpc-url "$RPC" --private-key "$DEPLOYER_KEY" \
    --broadcast --json "$@" \
  | sed -n 's/.*"deployedTo": *"\([^"]*\)".*/\1/p'
}

echo "deploying"
REGISTRY="$(deploy ../contracts/src/NodeRegistry.sol:NodeRegistry \
  --constructor-args "$MIN_STAKE" 604800 "$EPOCH" "$DEPLOYER" "$DEPLOYER")"
VERIFIER="$(deploy ../contracts/src/SpendVerifier.sol:SpendVerifier)"
POOL="$(deploy ../contracts/src/FeePool.sol:FeePool \
  --constructor-args "$DENOMINATION" "$VERIFIER")"
[[ -n "$REGISTRY" && -n "$VERIFIER" && -n "$POOL" ]] || { echo "deployment failed" >&2; exit 1; }
echo "  NodeRegistry  $REGISTRY"
echo "  SpendVerifier $VERIFIER"
echo "  FeePool       $POOL  (denomination $DENOMINATION wei)"

cargo build --release --quiet
NODE=target/release/erebus-node
CLIENT=target/release/erebus-client
FEES=target/release/erebus-fees

echo
for i in 0 1 2; do
  ID="$("$NODE" keygen --out "$WORK/node$i.key")"
  cast send "$REGISTRY" "register(bytes32,string)" "0x$ID" "127.0.0.1:$((BASE_PORT + i))" \
    --value "$MIN_STAKE" --rpc-url "$RPC" --private-key "${OPERATOR_KEYS[$i]}" >/dev/null
  echo "registered $ID  operator ${OPERATORS[$i]}"
done

for i in 0 1 2; do
  "$NODE" run --key "$WORK/node$i.key" --listen "127.0.0.1:$((BASE_PORT + i))" \
    --chain-rpc "$RPC" --contract "$REGISTRY" &
done
"$CLIENT" sink --chain-rpc "$RPC" --contract "$REGISTRY" --listen "127.0.0.1:$EXIT_PORT" &
sleep 2

echo
"$CLIENT" send --chain-rpc "$RPC" --contract "$REGISTRY" \
  --to "127.0.0.1:$EXIT_PORT" --message "buy 10 AAPL" --mean-delay-ms 20

echo
echo "funding a note"
NOTE_OUT="$("$FEES" new-note 2>/dev/null)"
NOTE="$(awk '/^note/ {print $2}' <<<"$NOTE_OUT")"
COMMITMENT="$(awk '/^commitment/ {print $2}' <<<"$NOTE_OUT")"

# Decoys first, so the note being spent is not the only leaf in the tree — the
# anonymity of a spend is exactly the size of this set.
for key in "${DECOY_KEYS[@]}"; do
  DECOY="$("$FEES" new-note 2>/dev/null | awk '/^commitment/ {print $2}')"
  cast send "$POOL" "deposit(uint256)" "$DECOY" --value "$DENOMINATION" \
    --rpc-url "$RPC" --private-key "$key" >/dev/null
done
cast send "$POOL" "deposit(uint256)" "$COMMITMENT" --value "$DENOMINATION" \
  --rpc-url "$RPC" --private-key "$PAYER_KEY" >/dev/null
echo "  pool holds $(cast call "$POOL" "leaves()(uint256)" --rpc-url "$RPC") notes"

# The deposit set, read back off the chain the way any payer would.
cast logs --from-block 0 --address "$POOL" \
  'Deposited(uint256,uint256,uint256)' --rpc-url "$RPC" --json \
  | jq '[.[].topics[1]]' >"$WORK/leaves.json"
"$FEES" root --leaves "$WORK/leaves.json"
echo "  pool root  $(cast call "$POOL" "currentRoot()" --rpc-url "$RPC")"

echo
echo "the nodes to pay, chosen from the registry like a route"
PAYEES="$("$CLIENT" payees --chain-rpc "$RPC" --contract "$REGISTRY" 2>/dev/null)"
echo "  $PAYEES"

echo
echo "proving the spend"
SPEND="$("$FEES" spend --note "$NOTE" --leaves "$WORK/leaves.json" \
  --pool "$POOL" --chain-id "$CHAIN_ID" --nodes "$PAYEES" --denomination "$DENOMINATION")"
echo "$SPEND"
ROOT="$(awk '/^root/ {print $2}' <<<"$SPEND")"
NULLIFIER="$(awk '/^nullifierHash/ {print $2}' <<<"$SPEND")"
AMOUNTS="$(awk '/^amounts/ {print $2}' <<<"$SPEND")"
PROOF="$(awk '/^proof/ {print $2}' <<<"$SPEND")"

echo
echo "submitting it from an account that never touched the pool"
cast send "$POOL" "spend(uint256,uint256,address[],uint256[],uint256[8])" \
  "$ROOT" "$NULLIFIER" "[$PAYEES]" "$AMOUNTS" "$PROOF" \
  --rpc-url "$RPC" --private-key "$RELAYER_KEY" >/dev/null

echo
for address in ${PAYEES//,/ }; do
  echo "  earned $(cast call "$POOL" "earned(address)(uint256)" "$address" --rpc-url "$RPC") wei  $address"
done

echo
echo "claiming"
for i in 0 1 2; do
  if [[ "$(cast call "$POOL" "earned(address)(uint256)" "${OPERATORS[$i]}" --rpc-url "$RPC" | awk '{print $1}')" != "0" ]]; then
    before="$(cast balance "${OPERATORS[$i]}" --rpc-url "$RPC")"
    cast send "$POOL" "claim()" --rpc-url "$RPC" --private-key "${OPERATOR_KEYS[$i]}" >/dev/null
    after="$(cast balance "${OPERATORS[$i]}" --rpc-url "$RPC")"
    echo "  ${OPERATORS[$i]} $before -> $after"
  fi
done

echo
echo "spent nullifier: $(cast call "$POOL" "spent(uint256)(bool)" "$NULLIFIER" --rpc-url "$RPC")"
echo "pool balance:    $(cast balance "$POOL" --rpc-url "$RPC") wei (the unspent deposits)"

if [[ -n "${KEEP:-}" ]]; then
  echo
  echo "chain $RPC, registry $REGISTRY, pool $POOL — ctrl-c to stop everything."
  wait
fi
