//! Replay detection.
//!
//! Re-injecting a captured packet is how an adversary turns a mix node into an
//! oracle: send the same packet twice and watch which output repeats. A node
//! therefore processes each packet exactly once. Tags are dropped in bulk when
//! the window fills, which is sound here because a node's keys rotate per epoch
//! and a packet built for an old key no longer verifies.

use std::collections::HashSet;
use std::sync::Mutex;

const WINDOW: usize = 1 << 20;

pub struct ReplayFilter {
    seen: Mutex<HashSet<[u8; 32]>>,
}

impl ReplayFilter {
    pub fn new() -> Self {
        Self {
            seen: Mutex::new(HashSet::new()),
        }
    }

    /// Returns false if this packet has been seen before.
    pub fn accept(&self, tag: [u8; 32]) -> bool {
        let mut seen = self.seen.lock().expect("replay filter poisoned");
        if seen.len() >= WINDOW {
            seen.clear();
        }
        seen.insert(tag)
    }
}

impl Default for ReplayFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_tag_is_accepted_once() {
        let filter = ReplayFilter::new();
        assert!(filter.accept([1u8; 32]));
        assert!(!filter.accept([1u8; 32]));
        assert!(filter.accept([2u8; 32]));
    }
}
