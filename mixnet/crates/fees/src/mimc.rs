//! MiMC over the BN254 scalar field, in Feistel mode.
//!
//! Chosen because the same hash has to run in three places that pay for it in
//! very different currencies: inside a Groth16 circuit (where a keccak Merkle
//! path would cost millions of constraints), in Solidity (where the tree is
//! extended on every deposit), and in this crate. `x^5` is a permutation of the
//! field, costs three constraints per round, and is a handful of `mulmod`s
//! on chain.
//!
//! The round constants are a keccak chain from a fixed label, so nothing has to
//! be shipped alongside the code and Rust and Solidity derive the same table
//! independently.

use ark_bn254::Fr;
use ark_ff::{BigInteger, Field, PrimeField};
use ark_r1cs_std::fields::fp::FpVar;
use ark_r1cs_std::fields::FieldVar;
use ark_relations::r1cs::SynthesisError;
use sha3::{Digest, Keccak256};

/// Rounds of the permutation. `ceil(log_5(r)) = 110` is the bound at which the
/// algebraic degree of the round function reaches the field size, which is the
/// condition the MiMC authors state for resistance to interpolation attacks.
pub const ROUNDS: usize = 110;

pub const CONSTANT_SEED: &[u8] = b"erebus.mimc.v1";

/// The round constants, derived as `c[0] = keccak(seed)`, `c[i] = keccak(c[i-1])`,
/// each read as a big-endian integer and reduced into the field.
pub fn round_constants() -> Vec<Fr> {
    let mut out = Vec::with_capacity(ROUNDS);
    let mut digest: [u8; 32] = Keccak256::digest(CONSTANT_SEED).into();
    for _ in 0..ROUNDS {
        out.push(Fr::from_be_bytes_mod_order(&digest));
        digest = Keccak256::digest(digest).into();
    }
    out
}

/// The permutation. `(l, r)` in, `(l, r)` out, with the halves swapped every
/// round but the last so that the two inputs are mixed symmetrically.
fn permute(mut l: Fr, mut r: Fr, constants: &[Fr]) -> (Fr, Fr) {
    for (i, c) in constants.iter().enumerate() {
        let t = l + c;
        let t5 = t.square().square() * t;
        if i == ROUNDS - 1 {
            r += t5;
        } else {
            let next = r + t5;
            r = l;
            l = next;
        }
    }
    (l, r)
}

/// Compresses two field elements into one.
pub fn hash(l: Fr, r: Fr) -> Fr {
    permute(l, r, &round_constants()).0
}

/// The same permutation as a circuit gadget, so a proof about `hash` is a proof
/// about the function the contract computes.
pub fn hash_var(l: &FpVar<Fr>, r: &FpVar<Fr>) -> Result<FpVar<Fr>, SynthesisError> {
    let constants = round_constants();
    let mut left = l.clone();
    let mut right = r.clone();

    for (i, c) in constants.iter().enumerate() {
        let t = &left + FpVar::constant(*c);
        // Three constraints: t^2, t^4, t^5. Squaring is what makes x^5 cheap.
        let t2 = t.square()?;
        let t4 = t2.square()?;
        let t5 = t4 * &t;

        if i == ROUNDS - 1 {
            right += t5;
        } else {
            let next = &right + t5;
            right = left;
            left = next;
        }
    }
    Ok(left)
}

/// Reads a 32-byte big-endian value as a field element, rejecting anything that
/// is not already reduced. Deposits and nullifiers arrive from calldata, and a
/// value that wraps would be a second name for a note that already exists.
pub fn field_from_be(bytes: &[u8; 32]) -> Option<Fr> {
    Fr::from_be_bytes_mod_order(bytes)
        .into_bigint()
        .to_bytes_be()
        .eq(&bytes.to_vec())
        .then(|| Fr::from_be_bytes_mod_order(bytes))
}

pub fn field_to_be(value: &Fr) -> [u8; 32] {
    let mut out = [0u8; 32];
    let bytes = value.into_bigint().to_bytes_be();
    out[32 - bytes.len()..].copy_from_slice(&bytes);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_r1cs_std::alloc::AllocVar;
    use ark_r1cs_std::eq::EqGadget;
    use ark_relations::r1cs::ConstraintSystem;

    #[test]
    fn constants_are_reduced_and_distinct() {
        let constants = round_constants();
        assert_eq!(constants.len(), ROUNDS);
        let mut seen = constants.clone();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), ROUNDS);
    }

    #[test]
    fn hashing_is_order_sensitive() {
        let a = Fr::from(7u64);
        let b = Fr::from(11u64);
        assert_ne!(hash(a, b), hash(b, a));
    }

    #[test]
    fn the_gadget_agrees_with_the_native_hash() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let a = Fr::from(12345u64);
        let b = Fr::from(6789u64);

        let a_var = FpVar::new_witness(cs.clone(), || Ok(a)).unwrap();
        let b_var = FpVar::new_witness(cs.clone(), || Ok(b)).unwrap();
        let out = hash_var(&a_var, &b_var).unwrap();
        out.enforce_equal(&FpVar::new_input(cs.clone(), || Ok(hash(a, b))).unwrap())
            .unwrap();

        assert!(cs.is_satisfied().unwrap());
    }

    #[test]
    fn unreduced_bytes_are_rejected() {
        assert!(field_from_be(&[0xff; 32]).is_none());
        let value = Fr::from(42u64);
        assert_eq!(field_from_be(&field_to_be(&value)), Some(value));
    }
}
