//! The proof itself: generated once for the whole file, because a Groth16 setup
//! for this circuit is the slowest thing in the crate.

use ark_bn254::Fr;
use erebus_fees::{payout_hash, prove, setup, verify, FeeError, Note, Tree};
use std::sync::OnceLock;

type Keys = (
    ark_groth16::ProvingKey<ark_bn254::Bn254>,
    ark_groth16::VerifyingKey<ark_bn254::Bn254>,
);

fn keys() -> &'static Keys {
    static KEYS: OnceLock<Keys> = OnceLock::new();
    KEYS.get_or_init(|| setup().expect("setup"))
}

fn pool_of(notes: &[Note]) -> Tree {
    Tree::from_leaves(notes.iter().map(Note::commitment).collect())
}

fn payout() -> Fr {
    payout_hash(
        31337,
        [7u8; 20],
        &[[1u8; 20], [2u8; 20], [3u8; 20]],
        &[1, 1, 1],
    )
    .expect("payout")
}

#[test]
fn a_deposited_note_can_be_spent() {
    let (pk, vk) = keys();
    let notes: Vec<Note> = (0..4).map(|_| Note::random()).collect();
    let tree = pool_of(&notes);

    let spend = prove(pk, &tree, &notes[2], payout()).expect("prove");
    assert_eq!(spend.root, tree.root());
    assert_eq!(spend.nullifier_hash, notes[2].nullifier_hash());
    verify(vk, &spend).expect("verify");
}

#[test]
fn a_note_that_was_never_deposited_cannot_be_spent() {
    let (pk, _) = keys();
    let tree = pool_of(&[Note::random(), Note::random()]);
    let err = prove(pk, &tree, &Note::random(), payout()).expect_err("no leaf");
    assert!(matches!(err, FeeError::NoteNotFunded));
}

#[test]
fn the_proof_is_bound_to_the_payout() {
    let (pk, vk) = keys();
    let notes = [Note::random(), Note::random()];
    let tree = pool_of(&notes);

    let mut spend = prove(pk, &tree, &notes[0], payout()).expect("prove");
    // Redirecting the money to a different node set after the fact: the same
    // proof, one public input changed.
    spend.payout = payout_hash(
        31337,
        [7u8; 20],
        &[[9u8; 20], [2u8; 20], [3u8; 20]],
        &[1, 1, 1],
    )
    .unwrap();
    assert!(matches!(verify(vk, &spend), Err(FeeError::Rejected)));
}

#[test]
fn the_proof_is_bound_to_the_root_and_the_nullifier() {
    let (pk, vk) = keys();
    let notes = [Note::random(), Note::random()];
    let tree = pool_of(&notes);

    let good = prove(pk, &tree, &notes[1], payout()).expect("prove");

    let mut wrong_root = good.clone();
    wrong_root.root = Fr::from(1u64);
    assert!(matches!(verify(vk, &wrong_root), Err(FeeError::Rejected)));

    let mut wrong_nullifier = good.clone();
    wrong_nullifier.nullifier_hash = notes[0].nullifier_hash();
    assert!(matches!(
        verify(vk, &wrong_nullifier),
        Err(FeeError::Rejected)
    ));
}

#[test]
fn two_spends_of_one_note_publish_the_same_nullifier_hash() {
    let (pk, vk) = keys();
    let notes = [Note::random(), Note::random()];
    let tree = pool_of(&notes);

    let first = prove(pk, &tree, &notes[0], payout()).expect("prove");
    let second = prove(pk, &tree, &notes[0], payout()).expect("prove");

    verify(vk, &first).expect("verify");
    verify(vk, &second).expect("verify");
    // Different proofs, so a double spend cannot be caught by comparing them —
    // the pool catches it because this value is the same both times.
    assert_eq!(first.nullifier_hash, second.nullifier_hash);
    assert_ne!(first.proof, second.proof);
}

#[test]
fn the_setup_is_reproducible() {
    let (_, first) = setup().expect("setup");
    let (_, second) = setup().expect("setup");
    assert_eq!(first, second);
}
