use sha2::{Digest, Sha256};

#[inline(always)]
fn hash_node(a: &[u8; 32], b: &[u8; 32], hasher: &mut Sha256) -> [u8; 32] {
    let zero = [0u8; 32];
    if a == &zero && b == &zero {
        return zero;
    }
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

    pub fn root(&self) -> [u8; 32] {
        if self.leaves.is_empty() { return [0u8; 32]; }
        let mut hasher = Sha256::new();
        let mut current_layer = self.leaves.clone();
        let next_pow2 = current_layer.len().next_power_of_two();
        current_layer.resize(next_pow2, [0u8; 32]);

        while current_layer.len() > 1 {
            let mut next_layer = Vec::with_capacity(current_layer.len() / 2);
            for i in (0..current_layer.len()).step_by(2) {
                next_layer.push(hash_node(&current_layer[i], &current_layer[i + 1], &mut hasher));
            }
            current_layer = next_layer;
        }
        current_layer[0]
    }

    pub fn build_multi_proof(
        &self,
        target_indices: &[usize],
        target_leaves_values: &[[u8; 32]],
    ) -> ([u8; 32], [u8; 32], u32, Vec<[u8; 32]>, Vec<bool>) {
        assert!(!self.leaves.is_empty(), "Tree must have at least one leaf");
        assert_eq!(target_indices.len(), target_leaves_values.len());
        assert!(target_indices.windows(2).all(|w| w[0] < w[1]));

        let mut hasher = Sha256::new();
        let mut current_nodes = self.leaves.clone();
        let mut new_nodes = self.leaves.clone();

        for (&idx, &val) in target_indices.iter().zip(target_leaves_values.iter()) {
            if idx < new_nodes.len() {
                new_nodes[idx] = val;
            }
        }

        let next_pow2 = current_nodes.len().next_power_of_two();
        current_nodes.resize(next_pow2, [0u8; 32]);
        new_nodes.resize(next_pow2, [0u8; 32]);

        let height = next_pow2.trailing_zeros();

        if current_nodes.len() == 1 {
            return (current_nodes[0], new_nodes[0], height, vec![], vec![]);
        }

        let mut current_targets = target_indices.to_vec();
        let mut proof = Vec::new();
        let mut flags = Vec::new();

        while current_nodes.len() > 1 {
            let mut next_current = Vec::with_capacity(current_nodes.len() / 2);
            let mut next_new = Vec::with_capacity(new_nodes.len() / 2);
            let mut next_targets = Vec::new();
            let mut target_pos = 0;
            let mut i = 0;

            while i < current_nodes.len() {
                let left_curr = current_nodes[i];
                let right_curr = current_nodes[i + 1];
                let left_new = new_nodes[i];
                let right_new = new_nodes[i + 1];

                let left_is_target = target_pos < current_targets.len() && current_targets[target_pos] == i;
                if left_is_target { target_pos += 1; }

                let right_is_target = target_pos < current_targets.len() && current_targets[target_pos] == i + 1;
                if right_is_target { target_pos += 1; }

                match (left_is_target, right_is_target) {
                    (true, true) => {
                        flags.push(true);
                        next_targets.push(i / 2);
                    }
                    (true, false) => {
                        flags.push(false);
                        proof.push(right_curr);
                        next_targets.push(i / 2);
                    }
                    (false, true) => {
                        flags.push(false);
                        proof.push(left_curr);
                        next_targets.push(i / 2);
                    }
                    (false, false) => {}
                }

                next_current.push(hash_node(&left_curr, &right_curr, &mut hasher));
                next_new.push(hash_node(&left_new, &right_new, &mut hasher));
                i += 2;
            }

            current_nodes = next_current;
            new_nodes = next_new;
            current_targets = next_targets;
        }

        (current_nodes[0], new_nodes[0], height, proof, flags)
    }

    // reason why we're extracting a target layer: 
    // when we get a batch of updates from issuers, we're constructing a 
    // new global merkle tree, as the credential of issuers might be in different trees
    // However, etherum smart contract maintains a list of roots [g1,g2,g3,g4,...,gn]
    // We cannot just send a single global root (well we can, but it'd require 
    // rebuilding merkle tree which defeats the point), we need to send the list of updated roots
    // which is why, we're extracting the layer at which all roots will be at.    
    // since all the merkle trees will be idential in the shape

    // now, if we ever allow each set to have different size of merkle trees, it will be a headache
    // but we can pass an array of layers, maybe. 
    pub fn verify_multi_proof(
        expected_root: &[u8; 32],
        target_leaves: &[[u8; 32]],
        proof: &[[u8; 32]],
        flags: &[bool],
        target_layer: Option<u32>,
    ) -> (bool, [u8; 32], Option<Vec<[u8; 32]>>) {
        if target_leaves.len() == 1 && proof.is_empty() && flags.is_empty() {
            let valid = target_leaves[0] == *expected_root;
            let layer = if target_layer == Some(0) { Some(vec![target_leaves[0]]) } else { None };
            return (valid, target_leaves[0], layer);
        }

        if target_leaves.is_empty() {
            return (false, [0u8; 32], None);
        }

        let mut hasher = Sha256::new();
        let mut hashes: Vec<([u8; 32], u32)> = Vec::with_capacity(flags.len());
        let mut leaf_pos = 0;
        let mut hash_pos = 0;
        let mut proof_pos = 0;
        let mut extracted_layer = Vec::new();

        for &flag in flags {
            let a_item = if leaf_pos < target_leaves.len() {
                leaf_pos += 1;
                (target_leaves[leaf_pos - 1], 0)
            } else if hash_pos < hashes.len() {
                hash_pos += 1;
                hashes[hash_pos - 1]
            } else {
                return (false, [0u8; 32], None);
            };

            let b_item = if flag {
                if leaf_pos < target_leaves.len() {
                    leaf_pos += 1;
                    (target_leaves[leaf_pos - 1], 0)
                } else if hash_pos < hashes.len() {
                    hash_pos += 1;
                    hashes[hash_pos - 1]
                } else {
                    return (false, [0u8; 32], None);
                }
            } else {
                if proof_pos < proof.len() {
                    proof_pos += 1;
                    (proof[proof_pos - 1], a_item.1)
                } else {
                    return (false, [0u8; 32], None);
                }
            };

            if target_layer == Some(a_item.1) {
                extracted_layer.push(a_item.0);
                extracted_layer.push(b_item.0);
            }

            let new_hash = hash_node(&a_item.0, &b_item.0, &mut hasher);
            hashes.push((new_hash, a_item.1 + 1));
        }

        if leaf_pos != target_leaves.len() || proof_pos != proof.len() {
            return (false, [0u8; 32], None);
        }

        let computed_root = hashes.last().unwrap().0;
        let root_level = hashes.last().unwrap().1;

        if target_layer == Some(root_level) {
            extracted_layer.push(computed_root);
        }

        let layer_result = if target_layer.is_some() { Some(extracted_layer) } else { None };
        (computed_root == *expected_root, computed_root, layer_result)
    }
}