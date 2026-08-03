use crate::rpc::{SequencerServerImpl,SubmissionBatchItem,SubmissionPayload,UpdateBatchItem,UpdatePayload};
use std::sync::{Arc, Mutex};
use crate::batcher::signer::DigitalSignatureService;
use crate::crypto::merkle::MerkleTree;
use crate::store::Store;
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
                
                let Ok(merkle) = Self::process_submission_batch_merkle(&payloads) else {
                    for item in batch_items {
                        let _ = item.respond_to.send(Err("Invalid Merkle tree".into()));
                    }
                    continue;
                };
                
                let Some(root) = merkle.get_root() else{
                        for item in batch_items {
                        let _ = item.respond_to.send(Err("failed to get merkle root tree".into()));
                    }
                    continue;
                };

                let Ok(seq_no) = store_submit.insert(root,merkle.leaves).await else{
                        for item in batch_items {
                            let _ = item.respond_to.send(Err("Invalid Merkle tree".into()));
                        }
                    continue;
                };
                
                
                for item in batch_items{
                    let _ = item.respond_to.send(Ok(seq_no));
                }
            }
        });

        tokio::spawn(async move{
            while let Some(batch_items) =  rx_update_batches.recv().await {
                let payloads : Vec<UpdatePayload> = batch_items
                .iter()
                .map(|item| item.payload.clone())
                .collect();
                mock_process_update_batch(&payloads);

                //get sequence number from db first

                for item in batch_items{
                    //mock seq number for now. replace with actual db seq no.
                    let _ = item.respond_to.send(Ok(1));
                }
            }
        });
        Self { tx_submit: tx_submission, tx_upate: tx_update, digital_signer: signer, store}
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


        let merkle = MerkleTree::from_leaves(leaves);

        Ok(merkle)
    }
}

fn mock_process_update_batch(_batch: &[UpdatePayload]) {
    // Merkle tree update verification logic here
}
