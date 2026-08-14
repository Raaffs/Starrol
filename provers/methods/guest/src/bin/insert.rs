#![no_main] // Tell Rust not to use the standard OS entry point
use common::InsertItem;
use risc0_zkvm::guest::env;
use bytemuck;
use k256::{
    EncodedPoint, ecdsa::
    {
        Signature, SigningKey, VerifyingKey, 
        signature::hazmat::{
            PrehashSigner,PrehashVerifier
        }
    }, 
};
use risc0_zkvm::sha::{Digest, Impl, Sha256};
// Declare the zkVM entry point
risc0_zkvm::guest::entry!(main);

fn main(){
    let mut expected_root = [0u8;32];
    env::read_slice(&mut expected_root);
    
    let size:u32=env::read();
    let size= size as usize;
    let mut batch:Vec<InsertItem>=Vec::with_capacity(size as usize);

    unsafe {
        batch.set_len(size);
    }

    let raw_bytes: &mut [u8]=bytemuck::cast_slice_mut(&mut batch);
    env::read_slice(raw_bytes);

    let mut roots = Vec::with_capacity(batch.len());

    for item in batch {
        let vk = VerifyingKey::from_encoded_point(
            &EncodedPoint::from_untagged_bytes((&item.public_key).into())
        ).unwrap();

        let signature = Signature::from_slice(&item.signature).unwrap();

        assert!(vk.verify_prehash(&item.root, &signature).is_ok());

        roots.push(item.root);
    }

    let final_merkle_root = compute_merkle_root(&roots);

    assert_eq!(final_merkle_root,expected_root)
}


pub fn compute_merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return [0u8; 32];
    }

    let next_pow2 = leaves.len().next_power_of_two();

    // Allocate two reusable buffers to eliminate zkVM heap allocations in the loop
    let mut current_layer = Vec::with_capacity(next_pow2);
    current_layer.extend_from_slice(leaves);
    current_layer.resize(next_pow2, [0u8; 32]);

    let mut next_layer = Vec::with_capacity(next_pow2 / 2);

    // Reduce the tree level by level
    while current_layer.len() > 1 {
        next_layer.clear();
        for i in (0..current_layer.len()).step_by(2) {
            next_layer.push(hash_node(&current_layer[i], &current_layer[i + 1]));
        }
        // Swap buffers to avoid re-allocating memory in every level loop
        std::mem::swap(&mut current_layer, &mut next_layer);
    }

    current_layer[0]
}

#[inline(always)]
fn hash_node(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    const ZERO: [u8; 32] = [0u8; 32];
    if a == &ZERO && b == &ZERO {
        return ZERO;
    }

    let (left, right) = if a > b { (b, a) } else { (a, b) };

    // Use RISC Zero's native SHA-256 accelerator circuit
    let digest_left = Digest::from(*left);
    let digest_right = Digest::from(*right);
    let parent = Impl::hash_pair(&digest_left, &digest_right);

    parent.as_bytes().try_into().unwrap()
}