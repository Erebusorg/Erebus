---
title: "Erebus: Network-Layer Privacy for Tokenized Finance"
subtitle: "A Sphinx mixnet with shielded fee payment for Robinhood Chain"
version: "Draft 0.1"
date: "2026"
---

## Abstract

Tokenized equities move the order book onto a public ledger. A stock token is an
ERC-20: every purchase, disposal, and holding size is permanently legible next to
an address that also receives a salary, pays rent, and holds a savings position.
Confidential-transaction schemes hide the contents of a transfer but not the act
of submitting it — the transport layer still leaks an IP address, a timing
pattern, and an RPC access history rich enough to reconstruct a portfolio that
the cryptography was meant to conceal.

Erebus closes the transport gap. It is a three-layer Sphinx mixnet with
continuous Poisson mixing, a shielded pool that pays relay fees without naming
the payer, and an on-chain node registry that removes the trusted directory
server. Clients construct packets locally, route them through independently
operated mix nodes, and reach Robinhood Chain through an exit node that cannot
attribute what it submits. The result is a system where no single participant —
node operator, RPC provider, venue, or global network observer — can link a user
to a position.

This document specifies the packet format, the mixing discipline, the fee-payment
circuit, the registry and incentive design, the threat model, and the properties
Erebus does and does not provide.

## 1. Motivation

### 1.1 The setting

Robinhood Chain is a permissionless, Ethereum-compatible Layer 2 built on the
Arbitrum stack, using ETH as its native gas token and targeting tokenized
real-world assets: equities, ETPs, and the instruments that settle around them.
Its design goals — high throughput, low fees, permissionless access — are
orthogonal to privacy, and deliberately so. A chain optimized for regulated
financial instruments must keep its state auditable.

That leaves privacy to be built above the ledger rather than into it, which is
the right layering. But it also means the privacy layer inherits the ledger's
full transparency as its adversarial baseline: anything a user does not
explicitly hide is public forever.

### 1.2 Why equities are different

Payment privacy and position privacy fail differently.

A leaked payment reveals a past event. A leaked position reveals a *present
intention* and can be traded against. Concretely, a public book of tokenized
equity positions enables:

- **Front-running and copy-trading.** An address accumulating a token can be
  followed within one block. The follower gets the alpha; the originator gets
  worse fills.
- **Inference of non-public relationships.** Employment, board membership, and
  advisory positions are frequently inferable from concentrated holdings and
  vesting-shaped transfer patterns.
- **Position-aware liquidation pressure.** Where a token is used as collateral,
  a visible position with a computable liquidation threshold is an invitation.
- **Long-lived personal exposure.** Retail holdings tied to a reused address
  disclose net worth trajectory, risk appetite, and life events to anyone,
  permanently, with no revocation path.

Confidentiality at the contract layer addresses the *content* of these
transactions. It does not address the fact that the transaction was submitted
from a particular network location at a particular instant, nor that the same
network location asked an RPC provider for exactly the balances that matter.

### 1.3 What the transport layer leaks

Three leaks survive a fully shielded contract layer:

1. **Submission metadata.** A signed transaction reaches the sequencer over TCP.
   The receiving endpoint learns the submitter's IP address and arrival time.
   Correlating arrival time with on-chain inclusion links a network identity to a
   shielded action even when the action's contents are encrypted.
2. **Read patterns.** Before proving anything, a shielded client must read: its
   Merkle path, its note set, the current root. Those reads name the exact
   records the client cares about. An RPC provider that answers them learns the
   position without ever seeing a transaction.
3. **Fee payment.** Someone must pay gas. If the shielded user pays it from a
   funded address, that address is the deanonymizing link. Relayers move the
   problem rather than solving it: the relayer must be compensated, and the
   compensation path is itself traceable.

Erebus targets all three: mixnet routing for (1), mixed reads for (2), and a
shielded fee circuit for (3).

## 2. Threat model

**Adversary capabilities.** We assume a global passive adversary that observes
every link in the network and records timing and volume for all traffic. We
further assume the adversary actively controls a fraction *f* of registered mix
nodes, can inject arbitrary traffic, can run RPC endpoints, and observes all
on-chain state. The adversary is computationally bounded and cannot break the
underlying primitives (X25519, ChaCha20-Poly1305, the proving system's
assumptions).

**Goal.** Sender-recipient unlinkability: the adversary's advantage in
determining which client produced a given on-chain submission should be
negligible above the base rate implied by the anonymity set.

**Assumptions.**

- At least one node in a chosen path is honest. With 3 layers and adversarial
  fraction *f* per layer, path compromise probability is *f³* under independent
  selection; §5.3 addresses correlated ownership.
- Clients have loosely synchronized clocks (±30s) for topology epochs.
- The chain provides liveness and censorship resistance; Erebus does not attempt
  to protect against a sequencer that refuses all Erebus traffic (§8.3).

**Explicit non-goals.**

- **No protection against user-side correlation.** A client that deposits and
  withdraws the same unusual amount within one epoch deanonymizes itself.
  Denomination discipline is enforced by the protocol (§4.2) but behavioral
  patterns are not.
- **No protection against endpoint compromise.** A compromised client device is
  outside the model.
- **No low-latency guarantee.** Mixing requires delay. Erebus is not suitable
  for latency-sensitive execution (§8.1).

## 3. The mix network

### 3.1 Packet format

Erebus uses Sphinx, an onion-encrypted packet format with the properties needed
for a mixnet rather than a circuit-based system: constant length across hops,
per-hop bitwise unlinkability, integrity protection, and immunity to replay and
tagging attacks.

A packet is a fixed **32 KB** structure:

```
Packet = ( Header , Payload )

Header  = ( α , β , γ )
  α : X25519 group element (32 B) — the per-hop ephemeral key
  β : encrypted routing information (padded to a constant length)
  γ : HMAC over β under the hop's derived integrity key

Payload : ChaCha20-Poly1305 sealed body, re-randomized at each hop
```

For a path *(n₁, n₂, n₃)* the client:

1. Samples a session key *x* and derives shared secrets
   *sᵢ = KDF(nᵢ.pub^{x·Πⱼ<ᵢ bⱼ})* where *bⱼ* are blinding factors, so each hop
   sees an unrelated group element.
2. Builds *β* inside-out: the innermost layer names the exit's destination, each
   outer layer names the next hop and carries that hop's delay parameter.
3. Seals the payload three times, innermost first.

Every hop performs a constant amount of work — one scalar multiplication, one
HMAC verification, one stream-cipher pass — and emits a packet of identical
length with no field in common with the one it received. Padding is added by the
client so that intermediate hops cannot infer path position from length.

**Fixed size is load-bearing.** Variable-length packets partition traffic into
size classes, and size classes are fingerprints. Payloads larger than the fixed
body are fragmented across independent packets that traverse independent paths;
reassembly happens only at the exit.

### 3.2 Mixing discipline

Each hop implements a **continuous-time (Poisson) mix**, following the Loopix
model rather than batch-and-flush.

On receipt, a node draws *d ~ Exp(1/μ)* and holds the packet for *d* before
forwarding. Because the exponential distribution is memoryless, the residual
delay of a queued packet is independent of how long it has already waited: an
adversary observing a node's output stream learns nothing about the arrival order
of its inputs. The delay parameter *μ* is chosen by the *client* and encoded
per-hop in *β*, which lets the client trade latency against the size of the
anonymity set it hides inside — and prevents a node from lying about the delay it
applied without detection via loop probes.

Batch mixes flush on a trigger (*n* packets, or a timer). Both triggers are
observable and both create exploitable edge conditions: a batch containing one
real packet and *n−1* adversarial ones is not mixed at all. Continuous mixing has
no flush event to observe and no batch boundary to starve.

### 3.3 Cover traffic

Three streams keep a node's output rate independent of real demand:

- **Loop cover.** A client emits packets addressed back to itself along a full
  path. Their return is a heartbeat: a node dropping or delaying traffic
  incorrectly is detected without any trusted monitor, because the client knows
  exactly what it sent and when it should return.
- **Drop cover.** Packets addressed to a null endpoint at the exit layer. They
  are indistinguishable from real submissions on every link they cross.
- **Node loops.** Nodes emit loop packets among themselves, so per-node output
  rate stays near-constant even when no client is active — the case where a
  single real packet would otherwise be trivially traceable.

Every stream carries packets of identical size and identical per-hop delay
distribution. On the wire, "a trade", "a probe", and "nothing" are the same
event.

### 3.4 Replies

A client that needs a response — an RPC result, a submission receipt — includes a
**single-use reply block (SURB)**: a pre-built header for a return path, with the
client's own address sealed inside it. The responder uses the SURB as an opaque
routing token. It cannot read the destination, cannot reuse it, and cannot
distinguish it from any other header.

## 4. Shielded fee payment

### 4.1 The problem

Mix nodes provide bandwidth and must be paid, and the exit node must fund the gas
of the transaction it submits. Any payment channel that names the payer undoes
the mixnet: the anonymity set collapses to "addresses that paid a relay fee this
epoch".

### 4.2 Construction

Erebus maintains a **shielded fee pool**: an append-only commitment tree over
fixed-denomination notes. The first implementation (`contracts/FeePool.sol`,
`mixnet/crates/fees/`) uses a depth-20 tree, a MiMC hash over the BN254 scalar
field, and a Groth16 spend proof.

```
note        = ( nullifier , secret )
commitment  = H( nullifier ‖ secret )
nullifier_h = H( nullifier ‖ 1 )
```

A spend proves, in zero knowledge:

1. **Membership** — the commitment is a leaf of the tree under a root the
   contract accepts (current or recently retired; the pool keeps 30 roots).
2. **Ownership** — the prover knows the note's opening, `nullifier` and `secret`.
3. **Nullifier correctness** — the published nullifier hash is correctly derived,
   so double-spends are detectable without revealing which note was spent.
4. **Payout binding** — the proof commits to `H( chain_id ‖ pool ‖ recipients ‖
   amounts )`, so it cannot be lifted off the mempool and redirected, replayed
   against another deployment, or replayed on another chain.

The contract checks the proof, marks the nullifier spent, and credits the named
node operators, who withdraw separately. Crediting rather than transferring is
deliberate: a node that reverts on payment must not be able to block a spend, and
the claim is a second transaction with no timing relationship to the packet it
paid for. The public transaction shows a proof, a nullifier hash, and three
recipients. It shows no depositor and no link to any deposit.

**What the split says.** Amounts are equal across the hops of a route and the
whole denomination is spent at once, so the payout is one of a small number of
indistinguishable shapes. A per-payer split would be a fingerprint that survives
every other layer.

**Anonymity set.** A spend is anonymous among the pool's unspent deposits, and
nothing more. A pool with two deposits hides nothing, which is why the fee is a
fixed denomination rather than metered per packet: metering would put the payer's
traffic volume on chain.

Arbitrary deposit amounts would be a fingerprint that survives every layer of the
system: a deposit of 3.14159 ETH matched against a later withdrawal of the same
value requires no cryptanalysis at all. The pool therefore accepts exactly one
amount and reverts on anything else.

### 4.3 What is not paid for yet

The pool pays *nodes*, not *packets*. A spend names the three operators of a
route drawn from the registry and pays them; it does not prove that those nodes
carried any particular packet, and no node checks a credential before forwarding.
Binding a payment to a packet is the harder problem — a fee that identified the
route of a known packet would reintroduce the link the mixnet exists to break —
and it is not solved here.

Also unresolved: the pool does not yet require recipients to be nodes registered
in `NodeRegistry`, so an arbitrary address can be paid.

### 4.4 Trusted setup

Groth16 needs a per-circuit setup, and the setup randomness is a trapdoor: whoever
holds it can forge proofs and drain the pool. The current keys are derived from a
**public, reproducible seed** so that anyone can rebuild the verifier from the
circuit and check that it matches. That makes the deployed artifacts auditable and
the pool unsafe to hold real value. A production deployment needs a multi-party
ceremony, or a proof system with no trusted setup.

### 4.5 Proving cost

Proving happens client-side. Today it is a native binary (`erebus-fees`); in the
browser the target is sub-three-second proving on a mid-range laptop. A mobile
client offloads to a user-selected proving service using a blinded witness, at a
stated privacy cost that the client surfaces explicitly rather than silently.

## 5. Node registry and incentives

### 5.1 Discovery without a directory

Tor's consensus is produced by directory authorities. That works, but it is a
coordination point — a set of servers that can be pressured, and a document that
can be equivocated on: a client shown a tailored view of the network can be given
a path with no honest hop.

Erebus publishes the node set on-chain. An operator registers by submitting a
public key, a network endpoint, and a stake. Clients read the registry from the
chain — or, once bootstrapped, through the mixnet itself.

`NodeRegistry` is implemented and tested (`contracts/`), and the node daemon,
client, and gateway read the set from it in one `eth_call`. Reading is not
participation: a client needs no account, signs nothing, and pays nothing, and
because everyone reads the same contract nobody can be handed a tailored node
set. An operator that announces an exit stops being selected in that call
immediately, while its bond stays slashable for the unbonding period, so leaving
is not a way out of a penalty. It is deployed nowhere yet.

### 5.2 Deterministic layer assignment

Layer assignment must be unpredictable in advance but identical across clients,
with no communication. Each epoch *e*:

```
seed        = H( block_hash(epoch_start) )
priority(n) = H( seed ‖ n.pubkey )
layer(n)    = stratify( priority(n) )   // stable partition into 3 layers
```

Every client derives the same assignment from public data. An operator cannot
choose to sit in the exit layer, where the highest-value information is, and
cannot know its next-epoch position early enough to prepare a targeted attack.

The seed is a past block hash, recorded on the first transaction of each epoch, so
it is unpredictable to operators but not to whoever orders blocks — on an L2, the
sequencer. A sequencer able to grind block hashes could bias assignment; we state
that rather than claim a randomness beacon the chain does not provide.

### 5.3 Sybil resistance and its limits

Stake makes registration costly; slashing makes misbehavior costly. What the
contract implements today is the accounting — bonds, unbonding, and a slash with
its reason on the record, decided by an arbiter address rather than by a proof —
because the evidence a mixnet can produce is statistical, and automating the
judgement would claim a certainty the protocol does not have. Loop probes
provide that evidence: a node that drops packets, delays them outside its
commanded distribution, or goes offline while advertising availability produces
verifiable probe failures, and a quorum of failure reports triggers slashing.

We state the limitation plainly: **stake does not prevent a well-funded adversary
from operating many nodes.** Path compromise is *f³* only if node ownership is
independent, and ownership is not directly observable. Erebus mitigates with
diversity constraints on path selection — clients avoid selecting two hops in the
same AS or hosting provider — and by publishing per-epoch AS-level concentration
metrics so the network's real diversity is auditable rather than assumed. Any
mixnet that claims stronger is overclaiming.

## 6. Private reads

Writes are the visible half of the problem. Reads are the larger half: a wallet
issues hundreds of them per session, and each one names an address and a storage
slot.

Erebus routes reads over the mixnet, which removes the network identifier and
breaks session correlation. The provider still learns *what* was asked, only not
*by whom* — and because each request takes an independent path with an
independent SURB, requests within one session cannot be grouped.

Removing the *what* as well requires private information retrieval: a scheme in
which the server computes over its whole database and returns an encrypted answer
without learning the index. Single-server lattice PIR is now practical for
append-only structures — Merkle paths, note sets, announcement logs — which
covers the reads a shielded client depends on. Point reads over the full state
trie remain open, primarily because verifying a PIR answer against a chain root
without leaking the index is unsolved in the practical regime. Erebus ships
mixnet-routed reads now and a PIR path for tree-shaped datasets as a distinct,
separately versioned component. We do not claim what is not built.

## 7. Architecture

```
┌─────────────────────────────────────────────────────────┐
│ Client (browser / wallet, Rust core compiled to WASM)   │
│  · path selection from on-chain registry                │
│  · Sphinx packet construction, SURB generation          │
│  · witness building + proof generation                  │
│  · loop-cover scheduler                                 │
└───────────────────────┬─────────────────────────────────┘
                        │ WebSocket / QUIC
┌───────────────────────▼─────────────────────────────────┐
│ Entry layer  →  Relay layer  →  Exit layer              │
│  Poisson delay queue per hop; constant-rate output      │
└───────────────────────┬─────────────────────────────────┘
                        │ JSON-RPC
┌───────────────────────▼─────────────────────────────────┐
│ Robinhood Chain                                         │
│  NodeRegistry.sol  — stake, keys, endpoints, slashing   │
│  FeePool.sol       — commitments, nullifiers, roots     │
│  SpendVerifier.sol — Groth16 proof verification         │
│  Adaptor.sol    — atomic verify + venue execution       │
└─────────────────────────────────────────────────────────┘
```

Node daemon and client core are one Rust codebase; the client is that codebase
compiled to WebAssembly, so packet construction and packet processing cannot
drift apart in ways that create fingerprints. The SDK exposes an EIP-1193
provider: an integrating wallet replaces its transport and changes nothing else.

## 8. Limitations

We would rather state these than have them discovered.

### 8.1 Latency

Three hops with exponential delays put end-to-end latency in the **1–5 second**
range at privacy-relevant *μ*, and tail latency is by construction unbounded.
Erebus is appropriate for accumulating, rebalancing, and settling. It is not
appropriate for latency-sensitive execution, and a system that tuned *μ* low
enough to compete there would provide no meaningful mixing.

### 8.2 Anonymity is a function of use

A mixnet's anonymity set is the set of packets that could plausibly be yours.
With few users, cover traffic sustains the *rate* but not the *diversity*: if one
client sends all real traffic, mixing hides which packet, not who. Early users
should assume weaker guarantees than the steady-state design implies, and the
network publishes an anonymity-set estimate per epoch rather than leaving users
to guess.

### 8.3 Exit-layer exposure

Exit nodes submit transactions and are the layer that is visible to sequencers,
RPC providers, and regulators. They can be rate-limited or blocked, which is a
censorship risk rather than a privacy risk, and they carry abuse exposure that is
jurisdictional as much as technical. Exit operation is a deliberate,
policy-bearing role, not a default.

### 8.4 Cryptographic and trust-setup risk

A shielded pool holds value under the assumption that its circuit is correct. A
constraint-underspecification bug is a mint. If the proving system requires a
trusted setup, the ceremony is a liability for the life of the deployment;
preferring a universal or transparent setup trades proof size and verification
cost for the removal of that liability. Erebus treats audited circuits and a
public, reproducible setup as launch blockers, not follow-ups.

### 8.5 Compliance

Privacy infrastructure for regulated instruments will attract regulatory
attention, and pretending otherwise is not a strategy. Erebus's position is that
privacy should be default and disclosure should be *user-initiated*: viewing keys
let a holder prove the contents of their own activity to an auditor, tax
authority, or counterparty, selectively and verifiably, without granting any
third party a standing capability over anyone else's data. Erebus implements no
backdoor and no protocol-level decryption quorum.

## 9. Roadmap

| Phase | Deliverable | Status |
| --- | --- | --- |
| 0 | Specification, threat model, reference packet format | This document |
| 1 | Sphinx implementation, 3-node local network, CLI client | Implemented in `mixnet/` |
| 2 | Registry contract, staking, public testnet fleet, live map | Planned |
| 3 | WASM SDK, EIP-1193 provider, mixnet-routed reads | Implemented in `sdk/`, against a local devnet |
| 4 | Shielded fee pool, spend circuit, generated verifier | Implemented in `contracts/` and `mixnet/crates/fees/`, on a reproducible (unsafe) setup |
| 5 | PIR component for tree-shaped datasets | Research |
| 6 | Incentivized testnet, external audit, mainnet | Planned |

## 10. Related work

**Loopix** (Piotrowska et al., 2017) introduced the continuous-time mixing and
loop-cover design Erebus adopts; **Nym** built its production lineage.
**Sphinx** (Danezis & Goldberg, 2009) is the packet format. **Tor** provides the
low-latency, non-mixing point of comparison — stronger UX, weaker resistance to a
global observer. **Zerocash** and its descendants define the shielded-pool
construction Erebus uses for fee payment; **Privacy Pools** (Buterin et al.,
2023) shows how association sets let honest users prove non-membership in
illicit sets, an approach compatible with §8.5. Erebus's contribution is not a
new primitive but a coherent composition targeted at a specific gap: tokenized
equities on a public L2, where transport metadata is the binding constraint on
privacy.

## 11. Status

Draft 0.1. No mainnet deployment. No audit. No token. The specification will
change as the implementation finds the places where it is wrong.
