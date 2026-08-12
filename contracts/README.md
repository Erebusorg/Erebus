# Erebus contracts

`NodeRegistry` is the only shared source of truth in Erebus: who the mix nodes
are, what they staked, and which epoch seed clients derive layers from.

It holds as little as it can. It does not assign layers, choose paths, price
anything, or see traffic. Layer assignment is a pure function of the epoch seed
and each node's public key, computed independently by every client
([`../mixnet/crates/topology`](../mixnet/crates/topology)), so the contract
cannot place a node where it wants it and an operator cannot buy the exit layer,
where the most valuable metadata is.

| | |
| --- | --- |
| `register(key, endpoint)` | Joins the node set, bonding at least `minStake`. The key is the X25519 key packets are onion-encrypted to. |
| `setEndpoint`, `addStake` | Move a node, or top a bond back up above the minimum. |
| `announceExit` → `withdraw` | Leave. Selection stops at once; the bond stays slashable for `unbondingPeriod`, so misbehaving and leaving in the same block is not an escape. |
| `slash(key, amount, reason)` | The arbiter takes stake, with the reason on the record. Slashed stake goes to the treasury, never to the arbiter. |
| `snapshot()` | One call: the epoch, its seed, and every node clients should be routing through. |
| `seedEpoch()` | Records this epoch's seed if nobody has yet. Anyone may call it; every state change does it too. |

An endpoint has to fit in a Sphinx delivery tag — 32 bytes — so a longer one is
refused here rather than registered and never routed to.

## Checks

```bash
cd contracts
forge test
forge fmt --check
```

## Deploy

```bash
MIN_STAKE=1000000000000000 UNBONDING=604800 EPOCH=3600 \
ARBITER=0x… TREASURY=0x… \
forge script script/Deploy.s.sol --rpc-url $RPC --broadcast
```

Then point the mixnet at it (see [`../mixnet`](../mixnet)):

```bash
cargo run --bin erebus-registry -- fetch --rpc $RPC --contract 0xREGISTRY
```

## What this is not

- **Not a reward system.** There are no fees and no emissions, so today running a
  node costs money and earns none. Paying for mixing without linking the payment
  to the traffic is the shielded-fee problem, and it is not solved here.
- **Not automated slashing.** The contract records a decision made off chain. The
  evidence a mixnet can produce — loop probes that never return — is statistical,
  so automating the judgement would claim a certainty the protocol does not have.
- **Not unpredictable.** The epoch seed is a past block hash, which stops an
  operator choosing its own layer. It does not stop whoever orders blocks.
- **Not governance.** `arbiter` and `treasury` are addresses fixed at deployment.
  Making them a DAO, a multisig, or anything with a process is a separate job.
- **Not audited.**
