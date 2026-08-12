//! A fee note: the only thing a payer keeps, and the only thing that links a
//! deposit to the nodes it eventually pays.

use ark_bn254::Fr;
use ark_ff::UniformRand;
use rand::{rngs::OsRng, RngCore};

use crate::error::FeeError;
use crate::mimc;

/// Domain tag that keeps a nullifier hash from ever being read as a commitment.
fn null_domain() -> Fr {
    Fr::from(1u64)
}

/// The secret half of a deposit.
///
/// `commitment` goes on chain when the note is funded; `nullifier_hash` goes on
/// chain when it is spent. Nothing published in either transaction ties the two
/// together without `nullifier`, which never leaves the payer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub nullifier: Fr,
    pub secret: Fr,
}

impl Note {
    /// Draws a fresh note from the OS. Two 254-bit secrets, so the anonymity of
    /// a spend rests on the size of the deposit set rather than on the entropy.
    pub fn random() -> Self {
        let mut rng = OsRng;
        Self {
            nullifier: Fr::rand(&mut rng),
            secret: Fr::rand(&mut rng),
        }
    }

    pub fn commitment(&self) -> Fr {
        mimc::hash(self.nullifier, self.secret)
    }

    pub fn nullifier_hash(&self) -> Fr {
        mimc::hash(self.nullifier, null_domain())
    }

    pub fn to_hex(&self) -> String {
        format!(
            "0x{}{}",
            hex::encode(mimc::field_to_be(&self.nullifier)),
            hex::encode(mimc::field_to_be(&self.secret))
        )
    }

    pub fn from_hex(text: &str) -> Result<Self, FeeError> {
        let raw = hex::decode(text.trim().trim_start_matches("0x"))
            .map_err(|_| FeeError::MalformedNote)?;
        if raw.len() != 64 {
            return Err(FeeError::MalformedNote);
        }
        let mut nullifier = [0u8; 32];
        let mut secret = [0u8; 32];
        nullifier.copy_from_slice(&raw[..32]);
        secret.copy_from_slice(&raw[32..]);

        Ok(Self {
            nullifier: mimc::field_from_be(&nullifier).ok_or(FeeError::MalformedNote)?,
            secret: mimc::field_from_be(&secret).ok_or(FeeError::MalformedNote)?,
        })
    }
}

/// The circuit's own view of the nullifier hash, for the gadget to match.
pub(crate) fn nullifier_domain() -> Fr {
    null_domain()
}

/// Fills `bytes` from the OS. Used for the deposit label in the CLI, which is
/// not a secret but should not be guessable either.
pub fn random_label() -> [u8; 8] {
    let mut out = [0u8; 8];
    OsRng.fill_bytes(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_distinct() {
        assert_ne!(Note::random().commitment(), Note::random().commitment());
    }

    #[test]
    fn a_note_survives_a_round_trip_through_text() {
        let note = Note::random();
        assert_eq!(Note::from_hex(&note.to_hex()).unwrap(), note);
    }

    #[test]
    fn a_commitment_is_not_a_nullifier_hash() {
        let note = Note::random();
        assert_ne!(note.commitment(), note.nullifier_hash());
    }

    #[test]
    fn malformed_notes_are_rejected() {
        assert!(Note::from_hex("0x00").is_err());
        assert!(Note::from_hex(&"ff".repeat(64)).is_err());
    }
}
