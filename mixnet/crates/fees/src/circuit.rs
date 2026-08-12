//! The spend circuit.
//!
//! Statement: *I know the opening of some commitment in the tree with root
//! `root`, its nullifier hashes to `nullifier_hash`, and I am asking for the
//! payout described by `payout`.*
//!
//! What it deliberately does not say: which commitment. That is the whole point
//! — the pool learns that a deposit it holds is being spent, not which one, so
//! the nodes being paid cannot be matched to the payer who funded them.

use ark_bn254::Fr;
use ark_r1cs_std::alloc::AllocVar;
use ark_r1cs_std::boolean::Boolean;
use ark_r1cs_std::eq::EqGadget;
use ark_r1cs_std::fields::fp::FpVar;
use ark_r1cs_std::fields::FieldVar;
use ark_r1cs_std::select::CondSelectGadget;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};

use crate::merkle::{Path, DEPTH};
use crate::mimc;
use crate::note::{nullifier_domain, Note};

/// A spend, as the prover sees it. `None` everywhere is the shape-only instance
/// used for the setup.
#[derive(Clone, Default)]
pub struct SpendCircuit {
    // Public.
    pub root: Option<Fr>,
    pub nullifier_hash: Option<Fr>,
    pub payout: Option<Fr>,
    // Private.
    pub nullifier: Option<Fr>,
    pub secret: Option<Fr>,
    pub siblings: Option<Vec<Fr>>,
    pub right: Option<Vec<bool>>,
}

impl SpendCircuit {
    pub fn new(note: &Note, path: &Path, root: Fr, payout: Fr) -> Self {
        Self {
            root: Some(root),
            nullifier_hash: Some(note.nullifier_hash()),
            payout: Some(payout),
            nullifier: Some(note.nullifier),
            secret: Some(note.secret),
            siblings: Some(path.siblings.clone()),
            right: Some(path.right.clone()),
        }
    }

    /// The public inputs, in the order the verifier and the contract expect.
    pub fn public_inputs(root: Fr, nullifier_hash: Fr, payout: Fr) -> [Fr; 3] {
        [root, nullifier_hash, payout]
    }
}

impl ConstraintSynthesizer<Fr> for SpendCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        let root = FpVar::new_input(cs.clone(), || {
            self.root.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let nullifier_hash = FpVar::new_input(cs.clone(), || {
            self.nullifier_hash.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let payout = FpVar::new_input(cs.clone(), || {
            self.payout.ok_or(SynthesisError::AssignmentMissing)
        })?;

        let nullifier = FpVar::new_witness(cs.clone(), || {
            self.nullifier.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let secret = FpVar::new_witness(cs.clone(), || {
            self.secret.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // The nullifier is what makes a note single-use, so the hash the pool
        // records has to be the hash of the same nullifier that opens the leaf.
        let expected = mimc::hash_var(&nullifier, &FpVar::constant(nullifier_domain()))?;
        expected.enforce_equal(&nullifier_hash)?;

        let mut node = mimc::hash_var(&nullifier, &secret)?;
        for level in 0..DEPTH {
            let sibling = FpVar::new_witness(cs.clone(), || {
                self.siblings
                    .as_ref()
                    .and_then(|s| s.get(level).copied())
                    .ok_or(SynthesisError::AssignmentMissing)
            })?;
            let is_right = Boolean::new_witness(cs.clone(), || {
                self.right
                    .as_ref()
                    .and_then(|r| r.get(level).copied())
                    .ok_or(SynthesisError::AssignmentMissing)
            })?;

            // Ordering the pair is the only place the index bits are used, and
            // getting it wrong would let a prover claim any sibling as a parent.
            let left = FpVar::conditionally_select(&is_right, &sibling, &node)?;
            let right = FpVar::conditionally_select(&is_right, &node, &sibling)?;
            node = mimc::hash_var(&left, &right)?;
        }
        node.enforce_equal(&root)?;

        // `payout` carries the recipients and the split. It needs no structure
        // here — the contract hashes the real values into it — but it has to be
        // constrained, or the proof would verify against any payout at all.
        let squared = payout.square()?;
        squared.enforce_equal(&(&payout * &payout))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle::Tree;
    use ark_relations::r1cs::ConstraintSystem;

    fn satisfied(circuit: SpendCircuit) -> bool {
        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();
        cs.is_satisfied().unwrap()
    }

    fn funded() -> (Note, Tree) {
        let note = Note::random();
        let mut tree = Tree::new();
        tree.push(Note::random().commitment());
        tree.push(note.commitment());
        tree.push(Note::random().commitment());
        (note, tree)
    }

    #[test]
    fn a_real_spend_satisfies_the_circuit() {
        let (note, tree) = funded();
        let path = tree.path(1).unwrap();
        let circuit = SpendCircuit::new(&note, &path, tree.root(), Fr::from(9u64));
        assert!(satisfied(circuit));
    }

    #[test]
    fn a_note_that_was_never_deposited_does_not() {
        let (_, tree) = funded();
        let path = tree.path(1).unwrap();
        let circuit = SpendCircuit::new(&Note::random(), &path, tree.root(), Fr::from(9u64));
        assert!(!satisfied(circuit));
    }

    #[test]
    fn a_wrong_nullifier_hash_does_not() {
        let (note, tree) = funded();
        let path = tree.path(1).unwrap();
        let mut circuit = SpendCircuit::new(&note, &path, tree.root(), Fr::from(9u64));
        circuit.nullifier_hash = Some(Fr::from(1234u64));
        assert!(!satisfied(circuit));
    }

    #[test]
    fn a_path_read_from_the_wrong_side_does_not() {
        let (note, tree) = funded();
        let mut path = tree.path(1).unwrap();
        path.right[0] = !path.right[0];
        let circuit = SpendCircuit::new(&note, &path, tree.root(), Fr::from(9u64));
        assert!(!satisfied(circuit));
    }
}
