use bytemuck;
use common::{SubmissionPayload, UpdateItem};
use methods::{INSERT_ELF, INSERT_ID, UPDATE_ELF, UPDATE_ID};
use risc0_zkvm::{default_prover, ExecutorEnv};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let prover = default_prover();
    let initial_root: [u8; 32] = [0x11; 32];

    // =========================================================================
    // EXECUTE INSERT PROVER
    // =========================================================================
    println!("--- Running Insert Guest Program ---");

    let insertions = vec![
        SubmissionPayload {
            root: [0x11; 32],
            signature: [0xAA; 64],
            public_key: [0xBB; 64],
        },
        SubmissionPayload {
            root: [0x11; 32],
            signature: [0xCC; 64],
            public_key: [0xDD; 64],
        },
    ];

    let insert_count = insertions.len() as u32;
    let insert_bytes: &[u8] = bytemuck::cast_slice(&insertions);

    let insert_env = ExecutorEnv::builder()
        .write_slice(&initial_root)       // 1. Initial Root (32B)
        .write(&insert_count)?            // 2. Array Count (4B)
        .write_slice(insert_bytes)        // 3. Array Payload (N * 160B)
        .build()?;

    let insert_info = prover.prove(insert_env, INSERT_ELF)?;
    insert_info.receipt.verify(INSERT_ID)?;
    println!("Insert Proof Verified!");

    // =========================================================================
    // EXECUTE UPDATE PROVER
    // =========================================================================
    println!("\n--- Running Update Guest Program ---");

    let updates = vec![
        UpdateItem {
            old_root: [0x11; 32],
            new_root: [0x22; 32],
            signature: [0x01; 64],
            public_key: [0x02; 64],
        },
        UpdateItem {
            old_root: [0x22; 32],
            new_root: [0x33; 32],
            signature: [0x03; 64],
            public_key: [0x04; 64],
        },
    ];

    let update_count = updates.len() as u32;
    let update_bytes: &[u8] = bytemuck::cast_slice(&updates);

    let update_env = ExecutorEnv::builder()
        .write_slice(&initial_root)       // 1. Initial Root (32B)
        .write(&update_count)?            // 2. Array Count (4B)
        .write_slice(update_bytes)        // 3. Array Payload (N * 192B)
        .build()?;

    let update_info = prover.prove(update_env, UPDATE_ELF)?;
    
    let final_root: [u8; 32] = update_info.receipt.journal.decode()?;
    update_info.receipt.verify(UPDATE_ID)?;

    println!("Update Proof Verified! Final Root: {:?}", final_root);

    Ok(())
}

#![no_main]
use bytemuck;
use common::SubmissionPayload;
use risc0_zkvm::guest::env;

risc0_zkvm::guest::entry!(main);

fn main() {
    // 1. Read initial state root (32 bytes)
    let mut current_root = [0u8; 32];
    env::read_slice(&mut current_root);

    // 2. Read count of submissions
    let count: u32 = env::read();
    let count = count as usize;

    // 3. Zero-copy read array of SubmissionPayload into heap vector
    let mut submissions: Vec<SubmissionPayload> = Vec::with_capacity(count);
    
    unsafe {
        // Uninitialized memory optimization
        submissions.set_len(count);
    }

    let raw_bytes: &mut [u8] = bytemuck::cast_slice_mut(&mut submissions);
    env::read_slice(raw_bytes);

    // 4. Process insertion logic
    for sub in &submissions {
        // Perform insertion logic / verify signatures / update state
        // Example: current_root = process_insertion(current_root, sub);
    }

    // 5. Commit final public root
    env::commit(&current_root);
}

#![no_main]
use bytemuck;
use common::UpdateItem;
use risc0_zkvm::guest::env;

risc0_zkvm::guest::entry!(main);

fn main() {
    // 1. Read initial state root (32 bytes)
    let mut current_root = [0u8; 32];
    env::read_slice(&mut current_root);

    // 2. Read count of update items
    let count: u32 = env::read();
    let count = count as usize;

    // 3. Zero-copy read array of UpdateItem into heap vector
    let mut updates: Vec<UpdateItem> = Vec::with_capacity(count);
    
    unsafe {
        // Uninitialized memory optimization
        updates.set_len(count);
    }

    let raw_bytes: &mut [u8] = bytemuck::cast_slice_mut(&mut updates);
    env::read_slice(raw_bytes);

    // 4. Process state transition sequence
    for item in &updates {
        assert_eq!(
            item.old_root, current_root,
            "Invalid update transition: old_root mismatch!"
        );

        current_root = item.new_root;
    }

    // 5. Commit final public root
    env::commit(&current_root);
}