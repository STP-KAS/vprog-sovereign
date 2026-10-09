//! Balance tree for one sovereign app. Depth 8, so 256 accounts.
//! Empty leaves are a fixed hash. This is a lab tree, not the framework's sparse Merkle tree.

use sha2::{Digest, Sha256};

pub const DEPTH: usize = 8;
pub const ACCOUNTS: usize = 1 << DEPTH;

pub type Digest32 = [u8; 32];

pub fn hash_parts(parts: &[&[u8]]) -> Digest32 {
    let mut h = Sha256::new();
    for part in parts {
        h.update(part);
    }
    h.finalize().into()
}

fn empty_at(level: usize) -> Digest32 {
    // Level 0 is a leaf. Each parent is the node of two empties below it.
    let mut node = hash_parts(&[b"empty"]);
    for _ in 0..level {
        node = hash_parts(&[b"node", &node, &node]);
    }
    node
}

pub fn leaf(balance: u64, locked: u64) -> Digest32 {
    hash_parts(&[b"leaf", &balance.to_le_bytes(), &locked.to_le_bytes()])
}

pub fn node(left: Digest32, right: Digest32) -> Digest32 {
    hash_parts(&[b"node", &left, &right])
}

pub fn root(leaves: &[Digest32; ACCOUNTS]) -> Digest32 {
    let mut level = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len() / 2);
        for pair in level.chunks(2) {
            next.push(node(pair[0], pair[1]));
        }
        level = next;
    }
    level[0]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Proof {
    pub siblings: [Digest32; DEPTH],
}

pub fn prove(leaves: &[Digest32; ACCOUNTS], index: usize) -> Proof {
    let mut siblings = [Digest32::default(); DEPTH];
    let mut level = leaves.to_vec();
    let mut cursor = index;
    for depth in 0..DEPTH {
        let sibling = cursor ^ 1;
        siblings[depth] = level[sibling];
        let mut next = Vec::with_capacity(level.len() / 2);
        for pair in level.chunks(2) {
            next.push(node(pair[0], pair[1]));
        }
        level = next;
        cursor /= 2;
    }
    Proof { siblings }
}

pub fn verify(root: Digest32, index: usize, leaf: Digest32, proof: &Proof) -> bool {
    let mut acc = leaf;
    let mut cursor = index;
    for depth in 0..DEPTH {
        let sibling = proof.siblings[depth];
        acc = if cursor & 1 == 0 {
            node(acc, sibling)
        } else {
            node(sibling, acc)
        };
        cursor /= 2;
    }
    acc == root
}

pub fn empty_leaf() -> Digest32 {
    empty_at(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_sompi_moves_the_root_and_invalidates_the_old_proof() {
        let mut leaves = [empty_leaf(); ACCOUNTS];
        leaves[3] = leaf(20, 0);
        leaves[4] = leaf(7, 0);
        let first = root(&leaves);
        let proof = prove(&leaves, 3);
        assert!(verify(first, 3, leaves[3], &proof));

        leaves[4] = leaf(8, 0);
        let second = root(&leaves);
        assert_ne!(first, second);
        assert!(!verify(second, 3, leaf(20, 0), &proof));
        let again = prove(&leaves, 3);
        assert!(verify(second, 3, leaf(20, 0), &again));
    }
}
