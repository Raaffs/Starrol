use sha2::{Digest, Sha256};

#[inline(always)]
fn hash_node(a: &[u8; 32], b: &[u8; 32], hasher: &mut Sha256) -> [u8; 32] {
    let (left, right) = if a > b { (b, a) } else { (a, b) };
    hasher.update(left);
    hasher.update(right);
    hasher.finalize_reset().into()
}

pub struct MerkleTree {
    pub leaves: Vec<[u8; 32]>,
}

impl MerkleTree {
    pub fn new(leaves: Vec<[u8; 32]>) -> Self {
        Self { leaves }
    }

    pub fn height(&self) -> u32 {
        let mut num_leaves = self.leaves.len();
        if num_leaves == 0 {
            return 0;
        }
        let mut height = 1;
        while num_leaves > 1 {
            num_leaves = (num_leaves + 1) / 2;
            height += 1;
        }
        height
    }

    pub fn root(&self) -> [u8; 32] {
        if self.leaves.is_empty() {
            return [0u8; 32];
        }

        let mut hasher = Sha256::new();
        let mut current_layer = self.leaves.clone();

        while current_layer.len() > 1 {
            let mut next_layer = Vec::with_capacity((current_layer.len() + 1) / 2);
            for i in (0..current_layer.len()).step_by(2) {
                if i + 1 < current_layer.len() {
                    next_layer.push(hash_node(&current_layer[i], &current_layer[i + 1], &mut hasher));
                } else {
                    next_layer.push(current_layer[i]);
                }
            }
            current_layer = next_layer;
        }

        current_layer[0]
    }

    /// Takes target indices and target leaf values. Applies the target values to a copy
    /// of the tree's stored leaves, constructs the tree layer-by-layer, and returns
    /// the calculated root alongside the proof.
    pub fn build_multi_proof(
        &self,
        target_indices: &[usize],
        target_leaves_values: &[[u8; 32]],
        target_layer: Option<u32>,
    ) -> ([u8; 32], u32, Vec<[u8; 32]>, Vec<bool>, Option<Vec<[u8; 32]>>) {
        if self.leaves.is_empty() {
            return ([0u8; 32], 0, vec![], vec![], None);
        }

        let height = self.height();
        let mut hasher = Sha256::new();
        let mut proof = Vec::new();
        let mut flags = Vec::new();

        let mut current_layer = self.leaves.clone();

        // Apply provided target leaf values to target positions
        for (&idx, &val) in target_indices.iter().zip(target_leaves_values.iter()) {
            if idx < current_layer.len() {
                current_layer[idx] = val;
            }
        }

        let mut current_indices = target_indices.to_vec();
        let mut saved_layer = None;
        let mut level: u32 = 0;

        while current_layer.len() > 1 {
            if target_layer == Some(level) {
                saved_layer = Some(current_layer.clone());
            }

            let mut next_indices = Vec::new();
            let mut i = 0;

            while i < current_indices.len() {
                let idx = current_indices[i];
                let sibling = idx ^ 1;

                if i + 1 < current_indices.len() && current_indices[i + 1] == sibling {
                    flags.push(true);
                    next_indices.push(idx / 2);
                    i += 2;
                } else if sibling < current_layer.len() {
                    proof.push(current_layer[sibling]);
                    flags.push(false);
                    next_indices.push(idx / 2);
                    i += 1;
                } else {
                    next_indices.push(idx / 2);
                    i += 1;
                }
            }

            let mut next_layer = Vec::with_capacity((current_layer.len() + 1) / 2);
            for j in (0..current_layer.len()).step_by(2) {
                if j + 1 < current_layer.len() {
                    next_layer.push(hash_node(&current_layer[j], &current_layer[j + 1], &mut hasher));
                } else {
                    next_layer.push(current_layer[j]);
                }
            }

            current_layer = next_layer;
            current_indices = next_indices;
            level += 1;
        }

        if target_layer == Some(level) {
            saved_layer = Some(current_layer.clone());
        }

        let root = current_layer[0];
        (root, height, proof, flags, saved_layer)
    }

    pub fn verify_multi_proof(
        expected_root: &[u8; 32],
        leaves: &[[u8; 32]],
        proof: &[[u8; 32]],
        flags: &[bool],
    ) -> (bool, [u8; 32]) {
        let total_hashes = leaves.len() + proof.len();
        if total_hashes == 0 || flags.len() != total_hashes - 1 {
            return (false, [0u8; 32]);
        }

        let mut hasher = Sha256::new();
        let mut stack: Vec<[u8; 32]> = Vec::with_capacity(leaves.len());
        let mut leaf_idx = 0;
        let mut proof_idx = 0;

        for &flag in flags {
            let a = match stack.pop() {
                Some(node) => node,
                None if leaf_idx < leaves.len() => {
                    let node = leaves[leaf_idx];
                    leaf_idx += 1;
                    node
                }
                _ => return (false, [0u8; 32]),
            };

            let b = if flag {
                match stack.pop() {
                    Some(node) => node,
                    None if leaf_idx < leaves.len() => {
                        let node = leaves[leaf_idx];
                        leaf_idx += 1;
                        node
                    }
                    _ => return (false, [0u8; 32]),
                }
            } else if proof_idx < proof.len() {
                let node = proof[proof_idx];
                proof_idx += 1;
                node
            } else {
                return (false, [0u8; 32]);
            };

            stack.push(hash_node(&a, &b, &mut hasher));
        }

        if stack.len() == 1 {
            let computed_root = stack[0];
            (computed_root == *expected_root, computed_root)
        } else {
            (false, [0u8; 32])
        }
    }
}
