use crate::internal::utils::utils;
use crate::rpc::{SequencerServerImpl,SubmissionBatchItem,SubmissionPayload,UpdateBatchItem,UpdatePayload};
use std::sync::{Arc, Mutex};
use crate::crypto::merkle::{self, MerkleTree};
use crate::store::{Store,DigitalSignatureService};
use tokio::sync::{mpsc};
use std::collections::HashMap;
pub mod sequencer {
    tonic::include_proto!("sequencer");
}

impl SequencerServerImpl{
    pub fn new(
        tx_submission: mpsc::Sender<SubmissionBatchItem>,
        mut rx_submission_batches: mpsc::Receiver<Vec<SubmissionBatchItem>>,
        tx_update: mpsc::Sender<UpdateBatchItem>,
        mut rx_update_batches: mpsc::Receiver<Vec<UpdateBatchItem>>,
        signer: Arc<Mutex<dyn DigitalSignatureService + Send>>,
        store: Arc<dyn Store + Send + Sync>
    )->Self{
        let store_submit = Arc::clone(&store);
        tokio::spawn(async move{
            while let Some(batch_items) =  rx_submission_batches.recv().await {
                
                let payloads : Vec<SubmissionPayload> = batch_items
                .iter()
                .map(|item| item.payload.clone())
                .collect();

                let fail_batch = |err_msg: &str, items:Vec<SubmissionBatchItem>| {
                    for item in items {
                        let _ = item
                        .respond_to
                        .send(Err(err_msg.into()))
                        .unwrap_or_else(|e | tracing::error!("error occurred while sending error message to clients: {:?}",e));
                    }
                };
                
                let merkle = match Self::process_submission_batch_merkle(&payloads) {
                   Ok(m) => m,
                   Err(e) => {
                        tracing::error!("error occurred while processing batch: {:?}",e);
                       fail_batch("Invalid Merkle tree", batch_items);
                       continue;
                   }
                };

                let root =  merkle.root();

                let seq_no = match store_submit.insert(root, merkle.leaves).await {
                    Ok(seq) => seq,
                    Err(e) => {
                        tracing::error!("error occurred while inserting into db: {:?}",e);
                        fail_batch("error occurred while inserting into db", batch_items);
                        continue;
                    }
                };

                for item in batch_items{
                    let _ = item.
                    respond_to.
                    send(Ok(seq_no)).
                    unwrap_or_else(|e| tracing::error!("failed to send success message to client: {:?}",e));
                }
            }
        });

        let store_update = Arc::clone(&store);
        tokio::spawn(async move{
            while let Some(batch_items) =  rx_update_batches.recv().await {
                let payloads : Vec<UpdatePayload> = batch_items
                .iter()
                .map(|item| item.payload.clone())
                .collect();

                let fail_batch = |err_msg: &str, items: Vec<UpdateBatchItem>| {
                    for item in items {
                        let _ = item
                        .respond_to
                        .send(Err(err_msg.into()))
                        .unwrap_or_else(|e| tracing::error!("error occurred while sending error message to clients: {:?}", e));
                    }
                };

                if let Err(e) = Self::process_update_batch(&payloads, &store_update).await {
                    tracing::error!("error occurred while processing update batch: {:?}", e);
                    fail_batch("error occurred while processing update batch", batch_items);
                    continue;
                }

                for item in batch_items {
                    let seq_no = item.payload.sequence_number;
                    let _ = item.respond_to.send(Ok(seq_no)).unwrap_or_else(|e| tracing::error!("failed to send success message to client: {:?}", e));
                }
            }
        });
        Self { tx_submit: tx_submission, tx_upate: tx_update, digital_signer: signer}
    }

    fn process_submission_batch_merkle(
        batch: &[SubmissionPayload],
    ) -> Result<MerkleTree, Box<dyn std::error::Error + Send + Sync>> {
        let mut leaves = Vec::with_capacity(batch.len());
        for x in batch {
            let leaf: [u8; 32] = x.root
                .clone()
                .try_into()
                .map_err(|_| "Root must be exactly 32 bytes")?;
            leaves.push(leaf);
        }
        
        let merkle = MerkleTree::new(leaves);

        Ok(merkle)
    }

    async fn process_update_batch(
        batch: &[UpdatePayload],
        store: &Arc<dyn Store + Send + Sync>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let sequence_numbers = utils::unique_elements(
            &batch.iter().map(|b| b.sequence_number).collect::<Vec<_>>(),
        );

        // Fetch current leaves for every affected sequence number from the DB.
        let leaves_by_sequence = store.get_leaves_set_by_seq_number(&sequence_numbers).await?;

        // Build a lookup: seq_number -> its current leaf list.
        let seq_to_leaves: HashMap<u64, Vec<[u8; 32]>> =
            leaves_by_sequence.into_iter().collect();

        // Group the incoming payloads by sequence number so we can process each
        // sub-tree independently and compute the correct per-seq new root.
        let mut payloads_by_seq: HashMap<u64, Vec<&UpdatePayload>> = HashMap::new();
        for payload in batch {
            payloads_by_seq
                .entry(payload.sequence_number)
                .or_default()
                .push(payload);
        }

        // For each sequence number: identify which leaves change, recompute the
        // sub-tree root, and persist everything in a single atomic transaction.
        for seq_number in &sequence_numbers {
            let current_leaves = seq_to_leaves
                .get(seq_number)
                .ok_or_else(|| format!("no leaves found for sequence number {}", seq_number))?;

            let updates = payloads_by_seq
                .get(seq_number)
                .ok_or_else(|| format!("no payloads for sequence number {}", seq_number))?;

            // Build a lookup: old_leaf -> position in current_leaves.
            let leaf_to_index: HashMap<[u8; 32], usize> = current_leaves
                .iter()
                .enumerate()
                .map(|(i, leaf)| (*leaf, i))
                .collect();

            let mut old_leaves_vec: Vec<[u8; 32]> = Vec::with_capacity(updates.len());
            let mut new_leaves_vec: Vec<[u8; 32]> = Vec::with_capacity(updates.len());
            let mut target_indices: Vec<usize> = Vec::with_capacity(updates.len());

            for payload in updates {
                let old_leaf: [u8; 32] = payload
                    .old_root
                    .clone()
                    .try_into()
                    .map_err(|_| "old_root is not 32 bytes".to_string())?;

                let new_leaf: [u8; 32] = payload
                    .new_root
                    .clone()
                    .try_into()
                    .map_err(|_| "new_root is not 32 bytes".to_string())?;

                let idx = *leaf_to_index
                    .get(&old_leaf)
                    .ok_or_else(|| {
                        format!(
                            "old_root {:?} not found in leaves for seq {}",
                            old_leaf, seq_number
                        )
                    })?;

                old_leaves_vec.push(old_leaf);
                new_leaves_vec.push(new_leaf);
                target_indices.push(idx);
            }

            // Sort by index — build_multi_proof requires target_indices to be sorted.
            let mut order: Vec<usize> = (0..target_indices.len()).collect();
            order.sort_by_key(|&i| target_indices[i]);

            let sorted_indices: Vec<usize> = order.iter().map(|&i| target_indices[i]).collect();
            let sorted_old: Vec<[u8; 32]> = order.iter().map(|&i| old_leaves_vec[i]).collect();
            let sorted_new: Vec<[u8; 32]> = order.iter().map(|&i| new_leaves_vec[i]).collect();

            // Build the sub-tree for this sequence number and compute the new root.
            let sub_tree = MerkleTree::new(current_leaves.clone());
            let (_old_root, new_root, _height, _proof, _flags) =
                sub_tree.build_multi_proof(&sorted_indices, &sorted_old);

            tracing::debug!(
                seq_number,
                old_root = ?_old_root,
                new_root = ?new_root,
                "computed new merkle root for sequence number"
            );

            // Persist: replace old leaves with new leaves and update the root — all in one tx.
            store
                .update_leaves_and_root(*seq_number as u32, &sorted_old, &sorted_new, new_root)
                .await
                .map_err(|e| {
                    tracing::error!(
                        seq_number,
                        error = ?e,
                        "failed to persist leaf/root update"
                    );
                    e
                })?;
        }

        Ok(())
    }

}

