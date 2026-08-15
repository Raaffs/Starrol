#![no_main]
use common::{StateTransitionProof, UpdateItem};
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
    let expected_root: [u8; 32] = env::read();
    let updated_expected_root: [u8; 32] = env::read();
    let batch_size:u32=env::read();

    let batch_size =  batch_size as usize;

    let current_flags : Vec<bool>=env::read();
    let updated_flags : Vec<bool>=env::read();
    let mut proof : Vec<StateTransitionProof>=Vec::with_capacity(batch_size);
    unsafe {
        proof.set_len(batch_size);
    }
    let raw_proof_bytes:&mut [u8]=bytemuck::cast_slice_mut(&mut proof);
    env::read_slice(raw_proof_bytes);

    let mut batch : Vec<UpdateItem>=Vec::with_capacity(batch_size);
    unsafe {
        batch.set_len(batch_size);
    }
    let raw_batch_bytes:&mut [u8] = bytemuck::cast_slice_mut(&mut batch);
    env::read_slice(raw_batch_bytes);

    // AoS -> flat leaf/proof-node arrays. verify_multi_proof requires contiguous
    // &[[u8;32]] slices; StateTransitionProof and UpdateItem interleave other
    // fields between the [u8;32]s we need, so a bytemuck reinterpret cast can't
    // produce these views. One Vec allocation per array is the minimum copy
    // possible without changing verify_multi_proof's signature.
    let mut current_leaves: Vec<[u8; 32]> = Vec::with_capacity(batch_size);
    let mut updated_leaves: Vec<[u8; 32]> = Vec::with_capacity(batch_size);
    for item in &batch {
        current_leaves.push(item.current_credential_root);
        updated_leaves.push(item.updated_credential_root);
    }

    let mut current_proof_nodes: Vec<[u8; 32]> = Vec::with_capacity(batch_size);
    let mut updated_proof_nodes: Vec<[u8; 32]> = Vec::with_capacity(batch_size);
    for p in &proof {
        current_proof_nodes.push(p.current_state_proof);
        updated_proof_nodes.push(p.next_state_proof);
    }

    // 1. verify current state
    let (current_ok, _, _) = verify_multi_proof(
        &expected_root,
        &current_leaves,
        &current_proof_nodes,
        &current_flags,
        None,
    );
    assert!(current_ok);

    // 2. verify updated state
    let (updated_ok, _, _) = verify_multi_proof(
        &updated_expected_root,
        &updated_leaves,
        &updated_proof_nodes,
        &updated_flags,
        None,
    );
    assert!(updated_ok);

    for item in batch{
        let vk = VerifyingKey::from_encoded_point(
            &EncodedPoint::from_untagged_bytes((&item.public_key).into())
        ).unwrap();

        let signature = Signature::from_slice(&item.signature).unwrap();
        assert!(vk.verify_prehash(&hash_sig_data(&item.current_credential_root, &item.updated_credential_root), &signature).is_ok());
    }

}

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

        let new_hash = hash_node(&a_item.0, &b_item.0);
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

#[inline(always)]
fn hash_sig_data(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut concat = [0u8; 64];
    concat[..32].copy_from_slice(a);
    concat[32..].copy_from_slice(b);
    // Standard SHA-256 hash using RISC Zero's built-in engine
    Impl::hash_bytes(&concat).as_bytes().try_into().unwrap()
}

#[inline(always)]
fn hash_node(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    const ZERO: [u8; 32] = [0u8; 32];
    if a == &ZERO && b == &ZERO {
        return ZERO;
    }

    let (left, right) = if a > b { (b, a) } else { (a, b) };

    let digest_left = Digest::from(*left);
    let digest_right = Digest::from(*right);
    let parent = Impl::hash_pair(&digest_left, &digest_right);

    parent.as_bytes().try_into().unwrap()
}