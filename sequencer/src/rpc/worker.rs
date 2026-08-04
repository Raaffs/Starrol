use crate::rpc::{SequencerServerImpl,SubmissionBatchItem,SubmissionPayload,UpdateBatchItem,UpdatePayload};
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
}

fn mock_process_update_batch(_batch: &[UpdatePayload]) {
    // Merkle tree update verification logic here
}
