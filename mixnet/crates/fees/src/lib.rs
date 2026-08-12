//! Shielded fees for Erebus.
//!
//! A payer deposits a fixed amount into the pool under a commitment, and later
//! proves — without saying which deposit is theirs — that it may direct one
//! deposit's worth of value at a set of nodes. The nodes are paid on chain in
//! the clear, because a node has to be able to see that it was paid; what stays
//! hidden is who paid them.
//!
//! Nothing here is a substitute for an audit, and the setup below is
//! deliberately not a ceremony (see [`setup`]).

pub mod circuit;
pub mod error;
pub mod merkle;
pub mod mimc;
pub mod note;

use ark_bn254::{Bn254, Fr};
use ark_ff::PrimeField;
use ark_groth16::{Groth16, Proof, ProvingKey, VerifyingKey};
use ark_serialize::CanonicalSerialize;
use ark_snark::SNARK;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use sha3::{Digest, Keccak256};

pub use circuit::SpendCircuit;
pub use error::FeeError;
pub use merkle::{Path, Tree, DEPTH};
pub use note::Note;

/// Seed for the setup randomness.
///
/// A real deployment needs a multi-party ceremony: whoever knows the randomness
/// that produced a Groth16 key can forge proofs for that circuit, and here that
/// randomness is a string in a public repository. It is written down instead of
/// generated so that the Rust prover and the committed Solidity verifier are
/// reproducibly the same circuit, and so that anyone can check the verifier
/// bytes against the code that made them. Until a ceremony replaces it, the
/// pool is a testnet toy.
pub const SETUP_SEED: [u8; 32] = *b"erebus.fees.unsafe.setup.v1\0\0\0\0\0";

/// A field element from arbitrary bytes, reduced. Used for the payout binding,
/// where the value only has to be a function both sides compute identically.
fn field_from_keccak(bytes: &[u8]) -> Fr {
    Fr::from_be_bytes_mod_order(&Keccak256::digest(bytes))
}

/// The public input that ties a proof to one payout on one pool.
///
/// Byte for byte what `FeePool._payoutHash` builds, including the chain id and
/// the pool address, so a proof cannot be lifted to another chain, another
/// deployment, or another set of recipients.
pub fn payout_hash(
    chain_id: u64,
    pool: [u8; 20],
    recipients: &[[u8; 20]],
    amounts: &[u128],
) -> Result<Fr, FeeError> {
    if recipients.is_empty() {
        return Err(FeeError::EmptyPayout);
    }
    if recipients.len() != amounts.len() {
        return Err(FeeError::PayoutMismatch {
            recipients: recipients.len(),
            amounts: amounts.len(),
        });
    }

    let mut data = Vec::new();
    data.extend_from_slice(&u256_be(chain_id as u128));
    data.extend_from_slice(&left_pad(&pool));
    data.extend_from_slice(&u256_be(recipients.len() as u128));
    for recipient in recipients {
        data.extend_from_slice(&left_pad(recipient));
    }
    for amount in amounts {
        data.extend_from_slice(&u256_be(*amount));
    }

    Ok(field_from_keccak(&data))
}

/// Splits one deposit across the nodes on a route.
///
/// Equal shares, with the rounding remainder on the last hop, because the split
/// is public: an uneven split chosen per payer would be a fingerprint, and the
/// pool insists the total is exactly one denomination.
pub fn even_split(denomination: u128, ways: usize) -> Vec<u128> {
    if ways == 0 {
        return Vec::new();
    }
    let share = denomination / ways as u128;
    let mut out = vec![share; ways];
    out[ways - 1] = denomination - share * (ways as u128 - 1);
    out
}

fn left_pad(address: &[u8; 20]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(address);
    out
}

fn u256_be(value: u128) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[16..].copy_from_slice(&value.to_be_bytes());
    out
}

/// Keys for the spend circuit, derived from [`SETUP_SEED`].
pub fn setup() -> Result<(ProvingKey<Bn254>, VerifyingKey<Bn254>), FeeError> {
    let mut rng = ChaCha20Rng::from_seed(SETUP_SEED);
    Groth16::<Bn254>::circuit_specific_setup(SpendCircuit::default(), &mut rng)
        .map_err(|e| FeeError::Proving(e.to_string()))
}

/// A spend proof, plus the public inputs a verifier needs to check it.
#[derive(Debug, Clone)]
pub struct Spend {
    pub proof: Proof<Bn254>,
    pub root: Fr,
    pub nullifier_hash: Fr,
    pub payout: Fr,
}

/// Proves the right to spend `note`, which must already be in `tree`.
pub fn prove(
    key: &ProvingKey<Bn254>,
    tree: &Tree,
    note: &Note,
    payout: Fr,
) -> Result<Spend, FeeError> {
    let commitment = note.commitment();
    let index = (0..tree.len())
        .find(|i| tree.leaf(*i) == Some(commitment))
        .ok_or(FeeError::NoteNotFunded)?;
    let path = tree.path(index).ok_or(FeeError::NoteNotFunded)?;
    let root = tree.root();

    let circuit = SpendCircuit::new(note, &path, root, payout);
    let mut rng = ChaCha20Rng::from_seed(rand::random());
    let proof = Groth16::<Bn254>::prove(key, circuit, &mut rng)
        .map_err(|e| FeeError::Proving(e.to_string()))?;

    Ok(Spend {
        proof,
        root,
        nullifier_hash: note.nullifier_hash(),
        payout,
    })
}

/// Checks a spend the way the contract will.
pub fn verify(key: &VerifyingKey<Bn254>, spend: &Spend) -> Result<(), FeeError> {
    let inputs = SpendCircuit::public_inputs(spend.root, spend.nullifier_hash, spend.payout);
    let ok = Groth16::<Bn254>::verify(key, &inputs, &spend.proof)
        .map_err(|e| FeeError::Proving(e.to_string()))?;
    ok.then_some(()).ok_or(FeeError::Rejected)
}

/// The proof as the eight field elements `FeePool.spend` takes.
///
/// `B` is a point over the extension field, and the pairing precompile reads
/// those coordinates imaginary part first (EIP-197), which is the one detail
/// that silently breaks a hand-rolled verifier.
pub fn proof_words(proof: &Proof<Bn254>) -> [String; 8] {
    let a = g1_words(&proof.a);
    let c = g1_words(&proof.c);
    let b = g2_words(&proof.b);
    [
        a[0].clone(),
        a[1].clone(),
        b[0].clone(),
        b[1].clone(),
        b[2].clone(),
        b[3].clone(),
        c[0].clone(),
        c[1].clone(),
    ]
}

fn hex_word(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn g1_words(point: &ark_bn254::G1Affine) -> [String; 2] {
    [hex_word(&be_bytes(&point.x)), hex_word(&be_bytes(&point.y))]
}

fn g2_words(point: &ark_bn254::G2Affine) -> [String; 4] {
    [
        hex_word(&be_bytes(&point.x.c1)),
        hex_word(&be_bytes(&point.x.c0)),
        hex_word(&be_bytes(&point.y.c1)),
        hex_word(&be_bytes(&point.y.c0)),
    ]
}

fn be_bytes<F: CanonicalSerialize>(value: &F) -> Vec<u8> {
    let mut le = Vec::new();
    value
        .serialize_compressed(&mut le)
        .expect("field elements serialize");
    le.reverse();
    le
}

/// Renders the Solidity verifier for the key returned by [`setup`].
pub fn solidity_verifier(vk: &VerifyingKey<Bn254>) -> String {
    let alpha = g1_words(&vk.alpha_g1);
    let beta = g2_words(&vk.beta_g2);
    let gamma = g2_words(&vk.gamma_g2);
    let delta = g2_words(&vk.delta_g2);

    let mut ic = String::new();
    for (i, point) in vk.gamma_abc_g1.iter().enumerate() {
        let words = g1_words(point);
        ic.push_str(&format!(
            "        IC[{i}] = G1Point({}, {});\n",
            words[0], words[1]
        ));
    }

    format!(
        r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title Groth16 verifier for the Erebus spend circuit.
///
/// @notice Generated by `cargo run --release -p erebus-fees -- export-verifier`
/// and then `forge fmt`. Do not edit: the constants below are the verifying key
/// for the circuit in
/// `mixnet/crates/fees/src/circuit.rs`, derived from the seed in that crate, and
/// a proof only verifies against the key that came out of the same setup.
contract SpendVerifier {{
    uint256 private constant P = 21888242871839275222246405745257275088696311157297823662689037894645226208583;
    uint256 private constant R = 21888242871839275222246405745257275088548364400416034343698204186575808495617;
    uint256 private constant INPUTS = {inputs};

    struct G1Point {{
        uint256 x;
        uint256 y;
    }}

    /// Coordinates of a point over Fp2, imaginary part first, as the pairing
    /// precompile expects them.
    struct G2Point {{
        uint256[2] x;
        uint256[2] y;
    }}

    error BadInput();
    error BadProof();

    /// @return true when `proof` proves the statement for `input`.
    function verify(uint256[8] calldata proof, uint256[INPUTS] calldata input)
        public
        view
        returns (bool)
    {{
        for (uint256 i = 0; i < INPUTS; i++) {{
            if (input[i] >= R) revert BadInput();
        }}

        G1Point[INPUTS + 1] memory IC;
{ic}
        G1Point memory vkX = IC[0];
        for (uint256 i = 0; i < INPUTS; i++) {{
            vkX = _add(vkX, _mul(IC[i + 1], input[i]));
        }}

        G1Point memory a = G1Point(proof[0], proof[1]);
        G2Point memory b = G2Point([proof[2], proof[3]], [proof[4], proof[5]]);
        G1Point memory c = G1Point(proof[6], proof[7]);

        // e(-A, B) * e(alpha, beta) * e(vkX, gamma) * e(C, delta) == 1
        uint256[24] memory pairing;
        _put(pairing, 0, _negate(a), b);
        _put(pairing, 1, G1Point({alpha0}, {alpha1}), G2Point([{beta0}, {beta1}], [{beta2}, {beta3}]));
        _put(pairing, 2, vkX, G2Point([{gamma0}, {gamma1}], [{gamma2}, {gamma3}]));
        _put(pairing, 3, c, G2Point([{delta0}, {delta1}], [{delta2}, {delta3}]));

        uint256[1] memory out;
        bool ok;
        assembly {{
            ok := staticcall(gas(), 8, pairing, 768, out, 32)
        }}
        if (!ok) revert BadProof();
        return out[0] == 1;
    }}

    function _put(uint256[24] memory into, uint256 slot, G1Point memory p, G2Point memory q)
        private
        pure
    {{
        uint256 at = slot * 6;
        into[at] = p.x;
        into[at + 1] = p.y;
        into[at + 2] = q.x[0];
        into[at + 3] = q.x[1];
        into[at + 4] = q.y[0];
        into[at + 5] = q.y[1];
    }}

    function _negate(G1Point memory p) private pure returns (G1Point memory) {{
        if (p.x == 0 && p.y == 0) return p;
        return G1Point(p.x, P - (p.y % P));
    }}

    function _add(G1Point memory a, G1Point memory b) private view returns (G1Point memory out) {{
        uint256[4] memory input = [a.x, a.y, b.x, b.y];
        bool ok;
        assembly {{
            ok := staticcall(gas(), 6, input, 128, out, 64)
        }}
        if (!ok) revert BadProof();
    }}

    function _mul(G1Point memory p, uint256 scalar) private view returns (G1Point memory out) {{
        uint256[3] memory input = [p.x, p.y, scalar];
        bool ok;
        assembly {{
            ok := staticcall(gas(), 7, input, 96, out, 64)
        }}
        if (!ok) revert BadProof();
    }}
}}
"#,
        inputs = vk.gamma_abc_g1.len() - 1,
        ic = ic.trim_end(),
        alpha0 = alpha[0],
        alpha1 = alpha[1],
        beta0 = beta[0],
        beta1 = beta[1],
        beta2 = beta[2],
        beta3 = beta[3],
        gamma0 = gamma[0],
        gamma1 = gamma[1],
        gamma2 = gamma[2],
        gamma3 = gamma[3],
        delta0 = delta[0],
        delta1 = delta[1],
        delta2 = delta[2],
        delta3 = delta[3],
    )
}

/// Parses `0x`-prefixed 20-byte hex into an address.
pub fn address_from_hex(text: &str) -> Result<[u8; 20], FeeError> {
    let raw = hex::decode(text.trim().trim_start_matches("0x"))
        .map_err(|_| FeeError::MalformedAddress)?;
    if raw.len() != 20 {
        return Err(FeeError::MalformedAddress);
    }
    let mut out = [0u8; 20];
    out.copy_from_slice(&raw);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_payout_binds_the_chain_the_pool_and_the_split() {
        let pool = [1u8; 20];
        let nodes = [[2u8; 20], [3u8; 20]];
        let amounts = [5u128, 5u128];

        let base = payout_hash(1, pool, &nodes, &amounts).unwrap();
        assert_ne!(base, payout_hash(2, pool, &nodes, &amounts).unwrap());
        assert_ne!(base, payout_hash(1, [9u8; 20], &nodes, &amounts).unwrap());
        assert_ne!(
            base,
            payout_hash(1, pool, &[[2u8; 20], [4u8; 20]], &amounts).unwrap()
        );
        assert_ne!(base, payout_hash(1, pool, &nodes, &[6u128, 4u128]).unwrap());
    }

    #[test]
    fn a_payout_needs_matching_arrays() {
        assert!(payout_hash(1, [0u8; 20], &[], &[]).is_err());
        assert!(payout_hash(1, [0u8; 20], &[[1u8; 20]], &[1u128, 2u128]).is_err());
    }

    #[test]
    fn a_split_adds_up_to_the_denomination() {
        for ways in 1..8usize {
            let split = even_split(10_000_000_000_000_000, ways);
            assert_eq!(split.len(), ways);
            assert_eq!(split.iter().sum::<u128>(), 10_000_000_000_000_000);
        }
        assert!(even_split(10, 0).is_empty());
    }

    #[test]
    fn addresses_are_twenty_bytes() {
        assert!(address_from_hex("0x00").is_err());
        assert_eq!(
            address_from_hex(&format!("0x{}", "11".repeat(20))).unwrap(),
            [0x11u8; 20]
        );
    }
}
