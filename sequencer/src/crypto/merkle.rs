use rs_merkle::{MerkleProof, MerkleTree as RsMerkleTree, algorithms::Sha256};

pub struct MerkleTree{
    pub leaves : Vec<[u8;32]>,
    pub tree   : RsMerkleTree<Sha256>,
}

impl MerkleTree{
    pub fn from_leaves(leaves:Vec<[u8;32]>)->Self{
        let tree = RsMerkleTree::<Sha256>::from_leaves(&leaves);
        Self{ leaves: leaves, tree: tree }
    }

    pub fn from_tree( tree: RsMerkleTree<Sha256>)->Self{
        let leaves= tree.leaves().clone().unwrap_or(vec![]);
        Self{ leaves: leaves, tree: tree  }
    }

    pub fn get_root(&self)->Option<[u8;32]>{
        self.tree.root()
    }

    pub fn build_proof_for(&self, indices: &[usize])-> MerkleProof<Sha256>{
        self.tree.proof(&indices)
    }

    // update leaves at given indices with new values
    pub fn transition(&mut self, indices: Vec<usize>, new_values: Vec<[u8;32]>)->MerkleProof<Sha256>{
        let proof = self.build_proof_for(&indices);
        for i in 0..indices.len() {
            let target_index = indices[i];
            let new_hash = new_values[i];
            self.leaves[target_index] = new_hash;
        }
        self.tree = RsMerkleTree::<Sha256>::from_leaves(&self.leaves);
        proof
    }   
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs_merkle::algorithms::Sha256;
    use rs_merkle::Hasher;

    fn leaf_hash(value: &str) -> [u8; 32] {
        Sha256::hash(value.as_bytes())
    }

    #[test]
    fn test_transition_proof_validation() {
        // 1. Create initial state and track the genesis root
        let initial_values = vec![
            "account-0:100",
            "account-1:100",
            "account-2:100",
            "account-3:100",
            "account-4:100",
        ];

        let initial_leaves: Vec<[u8; 32]> = initial_values.iter().map(|v| leaf_hash(v)).collect();
        
        let mut merkle_tree = MerkleTree::from_leaves(initial_leaves.clone());
        let genesis_root = merkle_tree.get_root().expect("failed to get genesis root");

        // 2. Define the target indices and their upcoming new values
        let initial_indices = vec![1usize, 3, 4];
        let update_indices =  vec![1usize, 3, 4];
        let old_leaves_to_prove = vec![initial_leaves[1],  initial_leaves[3], initial_leaves[4]];
        
        let new_values = vec![
            leaf_hash("account-1:200"),
            leaf_hash("account-3:550"),
            leaf_hash("account-4:550"),
        ];

        let malicious_indices=vec![1usize,3];
        let malicious_excluding= vec![            
            leaf_hash("account-1:200"),
            leaf_hash("account-3:550"),
        ];


        // 3. Perform the transition and harvest the proof package
        let total_leaves_count = initial_leaves.len();
        let proof = merkle_tree.transition(update_indices.clone(), new_values.clone());
        
        // Track the newly updated root state
        let new_root = merkle_tree.get_root().expect("failed to get new root");

        // 4. Verify the proof against the GENESIS state
        let verifies_genesis = proof.verify(
            genesis_root, 
            &initial_indices, 
            &old_leaves_to_prove, 
            total_leaves_count
        );
        assert!(verifies_genesis, "Proof failed to verify the old leaves against genesis root");

        // 5. Verify the EXACT SAME proof against the NEW state
        let verifies_new = proof.verify(
            new_root, 
            &update_indices, 
            &new_values, 
            total_leaves_count
        );
        assert!(verifies_new, "Proof failed to verify the new leaves against new root");
        
        let verify_malicious_exclusion=proof.verify(new_root, &malicious_indices, &malicious_excluding, 2);
        assert_ne!(verify_malicious_exclusion, true, "Proof failed to verify malicious exclusion");
    }
}