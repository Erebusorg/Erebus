#!/usr/bin/env bash
# A paid round against the contracts deployed on Robinhood Chain testnet, rather
# than against a chain you started yourself: three nodes stake their own bonds
# into the live registry, a request goes through all three, and a shielded fee
# pays the operators without the chain naming the payer.
#
#   FUNDER_KEY=0x… ./scripts/testnet-round.sh
#   KEEP=1 FUNDER_KEY=0x… ./scripts/testnet-round.sh   # leave the nodes running
#
# `FUNDER_KEY` needs about 0.008 testnet ETH — the faucet is at
# https://faucet.testnet.chain.robinhood.com. Everything else is generated: one
# key per operator, one payer, one relayer, so no single account is both staking
# and being paid. Keys land in $WORK and are re-used on a second run, so the
# script is idempotent: nodes already active are not registered twice.
#
# The endpoints registered are loopback. The nodes are real and so is the money,
# but they are all on one machine, and nothing about this round makes it a public
# network. Read the endpoints on https://erebusorg.com/network and you can tell.
#
# Needs foundry (cast, forge) and python3.
set -euo pipefail

RPC="${RPC:-https://rpc.testnet.chain.robinhood.com}"
REGISTRY="${REGISTRY:-0x1afa15F03e8d4f656374864750E0b62CCB6C8ad7}"
POOL="${POOL:-0x7e4E497aa102FdE094431F81BEFec6652A98b799}"
BASE_PORT="${BASE_PORT:-9000}"
EXIT_PORT="${EXIT_PORT:-9100}"

: "${FUNDER_KEY:?set FUNDER_KEY to a funded testnet key}"
for tool in cast python3; do
  command -v "$tool" >/dev/null || { echo "$tool is not on PATH" >&2; exit 1; }
done

cd "$(dirname "$0")/.."
WORK="${WORK:-$HOME/.erebus-testnet-round}"
mkdir -p "$WORK"

cargo build --release --quiet
NODE=target/release/erebus-node
CLIENT=target/release/erebus-client
FEES=target/release/erebus-fees

# The pool and the registry decide the amounts, not this script.
MIN_STAKE="$(cast call "$REGISTRY" "minStake()(uint256)" --rpc-url "$RPC" | awk '{print $1}')"
DENOMINATION="$(cast call "$POOL" "denomination()(uint256)" --rpc-url "$RPC" | awk '{print $1}')"
CHAIN_ID="$(cast chain-id --rpc-url "$RPC")"
FUNDER="$(cast wallet address --private-key "$FUNDER_KEY")"

step() { echo; echo "== $1"; }
send() { cast send --rpc-url "$RPC" --private-key "$1" "${@:2}" >/dev/null; }

wallet() { # name -> address, generating and remembering the key once
  if [[ ! -f "$WORK/$1.key" ]]; then
    cast wallet new --json \
      | python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["private_key"])' \
      >"$WORK/$1.key"
    chmod 600 "$WORK/$1.key"
  fi
  cast wallet address --private-key "$(cat "$WORK/$1.key")"
}

fund() { # address, wei — tops up rather than sending blind
  local have; have="$(cast balance "$1" --rpc-url "$RPC")"
  if (( have < $2 )); then
    send "$FUNDER_KEY" "$1" --value "$(( $2 - have ))"
  fi
  echo "  $1  $(cast balance "$1" --rpc-url "$RPC") wei"
}

echo "chain $CHAIN_ID, funder $FUNDER ($(cast balance "$FUNDER" --rpc-url "$RPC") wei)"
echo "registry $REGISTRY, pool $POOL"

step "accounts"
declare -a OP_ADDR=() OP_KEY=()
for i in 0 1 2; do
  OP_ADDR+=("$(wallet "operator$i")")
  OP_KEY+=("$(cat "$WORK/operator$i.key")")
done
PAYER="$(wallet payer)";     PAYER_KEY="$(cat "$WORK/payer.key")"
RELAYER="$(wallet relayer)"; RELAYER_KEY="$(cat "$WORK/relayer.key")"

# Which nodes still need a bond. Asked before funding anybody, so a second run
# costs gas rather than another three stakes.
declare -a NEED=()
for i in 0 1 2; do
  [[ -f "$WORK/node$i.key" ]] || "$NODE" keygen --out "$WORK/node$i.key" >"$WORK/node$i.id"
  if [[ "$(cast call "$REGISTRY" "isActive(bytes32)(bool)" "0x$(cat "$WORK/node$i.id")" --rpc-url "$RPC")" == "true" ]]; then
    NEED+=(0)
  else
    NEED+=("$MIN_STAKE")
  fi
done

# Gas headroom on top of what each account has to lock up. The L2 charges for
# calldata, so the spend proof is the expensive transaction here.
GAS=400000000000000
for i in 0 1 2; do fund "${OP_ADDR[$i]}" $(( NEED[i] + GAS )); done
fund "$PAYER" $(( DENOMINATION + GAS ))
fund "$RELAYER" $(( GAS ))

step "registering three nodes, each operator staking its own bond"
for i in 0 1 2; do
  ID="$(cat "$WORK/node$i.id")"
  if (( NEED[i] == 0 )); then
    echo "  $ID already active"
  else
    send "${OP_KEY[$i]}" "$REGISTRY" "register(bytes32,string)" \
      "0x$ID" "127.0.0.1:$((BASE_PORT + i))" --value "$MIN_STAKE"
    echo "  registered $ID  operator ${OP_ADDR[$i]}"
  fi
done

# An epoch with no recorded seed still assigns layers consistently, but it
# assigns them from zero: nobody gets the reshuffle they are owed.
EPOCH="$(cast call "$REGISTRY" "currentEpoch()(uint256)" --rpc-url "$RPC" | awk '{print $1}')"
if [[ "$(cast call "$REGISTRY" "seedOf(uint256)(bytes32)" "$EPOCH" --rpc-url "$RPC")" == "0x$(printf '0%.0s' {1..64})" ]]; then
  send "${OP_KEY[0]}" "$REGISTRY" "seedEpoch()"
fi
echo "  epoch $EPOCH, $(cast call "$REGISTRY" "count()(uint256)" --rpc-url "$RPC" | awk '{print $1}') keys registered"

step "starting the nodes, which read their set from the same registry"
trap 'kill $(jobs -p) 2>/dev/null || true' EXIT
for i in 0 1 2; do
  "$NODE" run --key "$WORK/node$i.key" --listen "127.0.0.1:$((BASE_PORT + i))" \
    --chain-rpc "$RPC" --contract "$REGISTRY" >"$WORK/node$i.log" 2>&1 &
done
"$CLIENT" sink --chain-rpc "$RPC" --contract "$REGISTRY" \
  --listen "127.0.0.1:$EXIT_PORT" >"$WORK/sink.log" 2>&1 &
sleep 4

step "one request through three hops, path drawn from the chain"
"$CLIENT" send --chain-rpc "$RPC" --contract "$REGISTRY" \
  --to "127.0.0.1:$EXIT_PORT" --message "${MESSAGE:-buy 10 AAPL}" --mean-delay-ms 50
"$CLIENT" probe --chain-rpc "$RPC" --contract "$REGISTRY" --mean-delay-ms 50

step "funding a note, and a decoy so the spend is not the only leaf"
NOTE_OUT="$("$FEES" new-note 2>/dev/null)"
NOTE="$(awk '/^note/ {print $2}' <<<"$NOTE_OUT")"
COMMITMENT="$(awk '/^commitment/ {print $2}' <<<"$NOTE_OUT")"
DECOY="$("$FEES" new-note 2>/dev/null | awk '/^commitment/ {print $2}')"
send "$FUNDER_KEY" "$POOL" "deposit(uint256)" "$DECOY" --value "$DENOMINATION"
send "$PAYER_KEY" "$POOL" "deposit(uint256)" "$COMMITMENT" --value "$DENOMINATION"
echo "  pool holds $(cast call "$POOL" "leaves()(uint256)" --rpc-url "$RPC" | awk '{print $1}') notes"

# The deposit set, read back off the chain the way any payer would.
cast logs --from-block 0 --address "$POOL" \
  'Deposited(uint256,uint256,uint256)' --rpc-url "$RPC" --json \
  | python3 -c 'import json,sys; print(json.dumps([l["topics"][1] for l in json.load(sys.stdin)]))' \
  >"$WORK/leaves.json"
"$FEES" root --leaves "$WORK/leaves.json"

step "the nodes to pay, chosen from the registry like a route"
PAYEES="$("$CLIENT" payees --chain-rpc "$RPC" --contract "$REGISTRY" 2>/dev/null)"
echo "  $PAYEES"

step "proving the spend"
DEADLINE=$(( $(cast block latest --field timestamp --rpc-url "$RPC") + 3600 ))
SPEND="$("$FEES" spend --note "$NOTE" --leaves "$WORK/leaves.json" \
  --pool "$POOL" --chain-id "$CHAIN_ID" --nodes "$PAYEES" \
  --denomination "$DENOMINATION" --deadline "$DEADLINE")"
echo "$SPEND"
ROOT="$(awk '/^root/ {print $2}' <<<"$SPEND")"
NULLIFIER="$(awk '/^nullifierHash/ {print $2}' <<<"$SPEND")"
AMOUNTS="$(awk '/^amounts/ {print $2}' <<<"$SPEND")"
PROOF="$(awk '/^proof/ {print $2}' <<<"$SPEND")"

step "submitting it from an account that never touched the pool"
cast send "$POOL" "spend(uint256,uint256,uint256,address[],uint256[],uint256[8])" \
  "$ROOT" "$NULLIFIER" "$DEADLINE" "[$PAYEES]" "$AMOUNTS" "$PROOF" \
  --rpc-url "$RPC" --private-key "$RELAYER_KEY" --json \
  | python3 -c 'import json,sys; r=json.load(sys.stdin); print("  tx", r["transactionHash"], "status", r["status"])'

step "claiming"
for i in 0 1 2; do
  earned="$(cast call "$POOL" "earned(address)(uint256)" "${OP_ADDR[$i]}" --rpc-url "$RPC" | awk '{print $1}')"
  [[ "$earned" == "0" ]] && continue
  before="$(cast balance "${OP_ADDR[$i]}" --rpc-url "$RPC")"
  send "${OP_KEY[$i]}" "$POOL" "claim()"
  echo "  ${OP_ADDR[$i]}  $before -> $(cast balance "${OP_ADDR[$i]}" --rpc-url "$RPC")"
done

step "state"
echo "  spent nullifier: $(cast call "$POOL" "spent(uint256)(bool)" "$NULLIFIER" --rpc-url "$RPC")"
echo "  pool balance:    $(cast balance "$POOL" --rpc-url "$RPC") wei (the unspent deposits)"
echo "  funder left:     $(cast balance "$FUNDER" --rpc-url "$RPC") wei"

if [[ -n "${KEEP:-}" ]]; then
  echo
  echo "nodes still running on 127.0.0.1:$BASE_PORT-$((BASE_PORT + 2)) — ctrl-c to stop."
  wait
fi
