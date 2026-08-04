use crate::rpc::{SequencerServerImpl,SubmissionBatchItem,SubmissionPayload,UpdateBatchItem,UpdatePayload};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::crypto::merkle::MerkleTree;
use crate::store::{Store,DigitalSignatureService};
use tokio::sync::{mpsc};

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
        let mut seq_map: HashMap<u64, Vec<&UpdatePayload>> = HashMap::new();
        for payload in batch {
            seq_map.entry(payload.sequence_number).or_default().push(payload);
        }

        for (seq_no, updates) in seq_map {
            // get all roots=>leaves based on seqno
            let leaves = store.get_leaves_by_seq_number(seq_no as u32).await?;
            if leaves.is_empty() {
                return Err(format!("No leaves found for sequence_number {}", seq_no).into());
            }
            let merkle = MerkleTree::new(leaves.clone());

            // based on leaves, map indices of old leaves 
            let mut pairs: Vec<(usize, [u8; 32], [u8; 32])> = Vec::new();
            for update in updates {
                let old_leaf: [u8; 32] = update
                    .old_root
                    .as_slice()
                    .try_into()
                    .map_err(|_| "old_root must be exactly 32 bytes")?;
                    let new_leaf: [u8; 32] = update
                    .new_root
                    .as_slice()
                    .try_into()
                    .map_err(|_| "new_root must be exactly 32 bytes")?;

                if let Some(idx) = leaves.iter().position(|l| l == &old_leaf) {
                    pairs.push((idx, new_leaf, old_leaf));
                } else {
                    return Err(format!("Leaf {:?} not found in sequence_number {}", old_leaf, seq_no).into());
                }
            }

            pairs.sort_by_key(|(idx, _, _)| *idx);
            pairs.dedup_by_key(|(idx, _, _)| *idx);

            if pairs.is_empty() {
                continue;
            }

            let target_indices: Vec<usize> = pairs.iter().map(|(i, _, _)| *i).collect();
            let target_leaves_values: Vec<[u8; 32]> = pairs.iter().map(|(_, new_l, _)| *new_l).collect();

            let (_old_root, new_root, _height, _proof, _flags) =
                merkle.build_multi_proof(&target_indices, &target_leaves_values);

            for (_idx, new_leaf, old_leaf) in pairs {
                store
                    .update_by_seq_number(seq_no as u32, old_leaf, new_leaf)
                    .await?;
            }
            store.update_root_by_seq_number(seq_no as u32, new_root).await?;
        }
        Ok(())
    }

}

