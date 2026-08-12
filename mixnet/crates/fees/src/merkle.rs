//! The commitment tree, mirrored from the contract.
//!
//! The contract keeps only the frontier it needs to append (one node per level)
//! plus a short history of roots. A client needs the sibling path for its own
//! leaf, which it rebuilds from the deposit log; this is that reconstruction.

use ark_bn254::Fr;
use ark_ff::PrimeField;
use sha3::{Digest, Keccak256};

use crate::mimc;

/// Depth of the tree, and so the number of notes it holds: 2^20 ≈ 1.05M.
///
/// Every level is a MiMC hash the prover has to constrain, so depth is paid for
/// in proving time by every spender; a million notes is far more anonymity set
/// than this network will have before the depth can be revisited.
pub const DEPTH: usize = 20;

pub const EMPTY_SEED: &[u8] = b"erebus.fees.empty.v1";

/// `zeros[i]` is the root of an empty subtree of height `i`.
pub fn zeros() -> Vec<Fr> {
    let digest: [u8; 32] = Keccak256::digest(EMPTY_SEED).into();
    let mut out = Vec::with_capacity(DEPTH + 1);
    out.push(Fr::from_be_bytes_mod_order(&digest));
    for i in 1..=DEPTH {
        let below = out[i - 1];
        out.push(mimc::hash(below, below));
    }
    out
}

/// The sibling path of a leaf, and which side it sits on at each level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    pub siblings: Vec<Fr>,
    /// `true` when the leaf is the right child at that level.
    pub right: Vec<bool>,
}

impl Path {
    /// Folds the path back up. The prover constrains exactly this walk.
    pub fn root(&self, leaf: Fr) -> Fr {
        let mut node = leaf;
        for (sibling, right) in self.siblings.iter().zip(self.right.iter()) {
            node = if *right {
                mimc::hash(*sibling, node)
            } else {
                mimc::hash(node, *sibling)
            };
        }
        node
    }
}

/// Every commitment ever deposited, in order.
#[derive(Debug, Clone, Default)]
pub struct Tree {
    leaves: Vec<Fr>,
}

impl Tree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_leaves(leaves: Vec<Fr>) -> Self {
        Self { leaves }
    }

    pub fn push(&mut self, leaf: Fr) -> usize {
        self.leaves.push(leaf);
        self.leaves.len() - 1
    }

    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    pub fn leaf(&self, index: usize) -> Option<Fr> {
        self.leaves.get(index).copied()
    }

    pub fn root(&self) -> Fr {
        let zeros = zeros();
        let mut level = self.leaves.clone();
        for height in 0..DEPTH {
            let mut next = Vec::with_capacity(level.len().div_ceil(2));
            for pair in level.chunks(2) {
                let left = pair[0];
                let right = pair.get(1).copied().unwrap_or(zeros[height]);
                next.push(mimc::hash(left, right));
            }
            if next.is_empty() {
                next.push(zeros[height + 1]);
            }
            level = next;
        }
        level[0]
    }

    /// The path for a leaf, or `None` if that leaf was never deposited.
    pub fn path(&self, index: usize) -> Option<Path> {
        if index >= self.leaves.len() {
            return None;
        }
        let zeros = zeros();
        let mut siblings = Vec::with_capacity(DEPTH);
        let mut right = Vec::with_capacity(DEPTH);

        let mut level = self.leaves.clone();
        let mut at = index;
        for empty in zeros.iter().take(DEPTH) {
            let is_right = at % 2 == 1;
            let sibling_at = if is_right { at - 1 } else { at + 1 };
            siblings.push(level.get(sibling_at).copied().unwrap_or(*empty));
            right.push(is_right);

            let mut next = Vec::with_capacity(level.len().div_ceil(2));
            for pair in level.chunks(2) {
                let left = pair[0];
                let sibling = pair.get(1).copied().unwrap_or(*empty);
                next.push(mimc::hash(left, sibling));
            }
            level = next;
            at /= 2;
        }

        Some(Path { siblings, right })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_tree_is_the_empty_root() {
        assert_eq!(Tree::new().root(), zeros()[DEPTH]);
    }

    #[test]
    fn every_path_folds_back_to_the_root() {
        let mut tree = Tree::new();
        for i in 0..7u64 {
            tree.push(Fr::from(i + 1));
        }
        let root = tree.root();
        for i in 0..tree.len() {
            let path = tree.path(i).expect("leaf exists");
            assert_eq!(path.root(tree.leaf(i).unwrap()), root);
        }
    }

    #[test]
    fn a_new_deposit_moves_the_root() {
        let mut tree = Tree::new();
        tree.push(Fr::from(1u64));
        let before = tree.root();
        tree.push(Fr::from(2u64));
        assert_ne!(before, tree.root());
    }

    #[test]
    fn a_missing_leaf_has_no_path() {
        assert!(Tree::new().path(0).is_none());
    }
}
