# Erebus contracts

Two contracts. `NodeRegistry` says who the mix nodes are; `FeePool` pays them
without saying who paid.

## NodeRegistry

The only shared source of truth in Erebus: who the mix nodes are, what they
staked, and which epoch seed clients derive layers from.

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

## FeePool

A fixed-denomination shielded pool. Deposits go in with a commitment to a secret
note; a spend proves in zero knowledge that some unspent note in the pool is the
prover's, and the pool credits the node operators the proof names.

| | |
| --- | --- |
| `deposit(commitment)` | Takes exactly `denomination` and appends the commitment to a depth-20 tree. One amount only: a distinctive deposit is a fingerprint no later layer can remove. |
| `spend(root, nullifierHash, deadline, recipients, amounts, proof)` | Anyone may submit, until `deadline`. The root must be one of the last 30, the nullifier must be unspent, the amounts must total exactly one denomination, and every recipient must be an operator with an active node in `NodeRegistry`. |
| `claim()` | A paid operator withdraws. Crediting rather than transferring means a recipient that reverts cannot block a spend, and the claim is a separate transaction with no timing relationship to anything. |
| `payoutHash(deadline, recipients, amounts)` | What the proof is bound to: `chainid`, this pool, the deadline, the recipients, the amounts. A proof lifted from the mempool cannot be redirected or stretched to a later deadline, and one from another deployment or chain does not verify. |

`SpendVerifier.sol` is generated, not written:

```bash
# from the repo root: the cargo workspace is mixnet/, the output path is not
cargo run --release --manifest-path mixnet/Cargo.toml -p erebus-fees -- export-verifier
cd contracts && forge fmt
```

**The setup is not safe.** The Groth16 keys come from a public, reproducible seed
so that anyone can regenerate the verifier and check it matches the circuit —
which also means anyone can forge a proof and drain the pool. Real value needs a
multi-party ceremony, or a proof system with no trusted setup.

The registry check is on the operator, not on the route: it says the payee runs a
node the network is currently selecting, not that this payee carried these
packets. A payer can still direct a spend at any three active operators.

The deadline is a plain timestamp the payer chooses and the proof commits to. It
bounds how long an unsubmitted proof stays usable; it is not an epoch accounting
scheme, and it does not rate-limit anything.

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

DENOMINATION=10000000000000000 \
forge script script/DeployFees.s.sol --rpc-url $RPC --broadcast
```

Then point the mixnet at it (see [`../mixnet`](../mixnet)):

```bash
cargo run --bin erebus-registry -- fetch --rpc $RPC --contract 0xREGISTRY
```

Or run the whole thing locally — chain, both contracts, three nodes, a request,
and a fee that pays them — with `../mixnet/scripts/paid-round.sh`.

## What this is not

- **Not payment per packet.** `FeePool` pays the operators of a route drawn from
  the registry. Nothing proves those nodes carried anything, and no node checks a
  fee before forwarding. A credential that identified the route of a known packet
  would rebuild the link the mixnet exists to break, so this is left open.
- **Not a safe pool.** See the setup note above.
- **Not automated slashing.** The contract records a decision made off chain. The
  evidence a mixnet can produce — loop probes that never return — is statistical,
  so automating the judgement would claim a certainty the protocol does not have.
- **Not unpredictable.** The epoch seed is a past block hash, which stops an
  operator choosing its own layer. It does not stop whoever orders blocks.
- **Not governance.** `arbiter` and `treasury` are addresses fixed at deployment.
  Making them a DAO, a multisig, or anything with a process is a separate job.
- **Not audited.**
