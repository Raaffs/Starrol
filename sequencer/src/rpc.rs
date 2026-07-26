use tonic::{transport::Server, Request, Response, Status};
use tokio::sync::{mpsc,oneshot};
use std::sync::{Arc, Mutex};
use sequencer::root_anchoring_server::{RootAnchoring, RootAnchoringServer};
use sequencer::{RootSubmission, RootUpdate, Response as ProtoResponse, SequencerStatus};

use crate::batcher::signer::DigitalSignatureService;

pub mod sequencer {
    tonic::include_proto!("sequencer");
}

#[derive(Clone, Debug)]
pub struct SubmissionPayload {
    pub root: Vec<u8>,
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct UpdatePayload {
    pub old_root: Vec<u8>,
    pub new_root: Vec<u8>,
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
}

pub struct SubmissionBatchItem {
    pub payload: SubmissionPayload,
    pub respond_to: oneshot::Sender<Result<u64, String>>,
}

pub struct UpdateBatchItem {
    pub payload: UpdatePayload,
    pub respond_to: oneshot::Sender<Result<u64, String>>,
}

pub struct SequencerServerImpl {
    tx_submit: mpsc::Sender<SubmissionBatchItem>,
    tx_upate: mpsc::Sender<UpdateBatchItem>,
    digital_signer: Arc<Mutex<dyn DigitalSignatureService + Send>>
}

impl SequencerServerImpl{
    pub fn new(
        tx_submission: mpsc::Sender<SubmissionBatchItem>,
        mut rx_submission_batches: mpsc::Receiver<Vec<SubmissionBatchItem>>,
        tx_update: mpsc::Sender<UpdateBatchItem>,
        mut rx_update_batches: mpsc::Receiver<Vec<UpdateBatchItem>>,
        signer: Arc<Mutex<dyn DigitalSignatureService + Send>>
    )->Self{
        tokio::spawn(async move{
            while let Some(batch_items) =  rx_submission_batches.recv().await {
                let payloads : Vec<SubmissionPayload> = batch_items
                .iter()
                .map(|item| item.payload.clone())
                .collect();
                mock_process_submission_batch(&payloads);

                //get sequence number from db first

                for item in batch_items{
                    //mock seq number for now. replace with actual db seq no.
                    let _ = item.respond_to.send(Ok(1));
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
}

fn mock_process_submission_batch(_batch: &[SubmissionPayload]) {
    // Merkle tree construction / verification logic here
}

fn mock_process_update_batch(_batch: &[UpdatePayload]) {
    // Merkle tree update verification logic here
}

#[tonic::async_trait]
impl RootAnchoring for SequencerServerImpl {
    
    async fn submit_root(
        &self,
        request: Request<RootSubmission>,
    ) -> Result<Response<ProtoResponse>, Status> {
        let req = request.into_inner();
        let root_hash: &[u8;32]= match req.certificate_root.as_slice().try_into(){
                Ok(hash) => hash,
                Err(_) =>  return Ok(Response::new(ProtoResponse{
                    status: SequencerStatus::InvalidSignature.into(),
                    sequence_number:0,
                    error_details: "Invalid certificate root length (must be 32 bytes)".to_string(),
                }))
        };  

        let is_valid = match self.digital_signer.lock().unwrap().verify(&req.public_key, root_hash, &req.signature) {
            Ok(valid) => valid,
            Err(e) => {
                return Ok(Response::new(ProtoResponse {
                    status: SequencerStatus::InternalError.into(),
                    sequence_number: 0,
                    error_details: format!("Signer error during verification: {}", e),
                }));
            }
        };
        if !is_valid {
            return Ok(Response::new(ProtoResponse {
                status: SequencerStatus::InvalidSignature.into(),
                sequence_number: 0,
                error_details: "Invalid signature".to_string(),
            }));
        }
        
        let (resp_tx, resp_rx) = oneshot::channel::<Result<u64, String>>();
        let item = SubmissionBatchItem {
            payload: SubmissionPayload {
                root: req.certificate_root,
                signature: req.signature,
                public_key: req.public_key,
            },
            respond_to: resp_tx,
        };

        if self.tx_submit.send(item).await.is_err() {
            return Ok(Response::new(ProtoResponse {
                status: SequencerStatus::InternalError.into(),
                sequence_number: 0,
                error_details: "Submission channel closed".to_string(),
            }));
        }

        match resp_rx.await {
            Ok(Ok(sequence_number)) => Ok(Response::new(ProtoResponse {
                status: SequencerStatus::Accepted.into(),
                sequence_number,
                error_details: String::new(),
            })),
            Ok(Err(db_err)) => Ok(Response::new(ProtoResponse {
                status: SequencerStatus::InternalError.into(),
                sequence_number: 0,
                error_details: format!("DB Error: {}", db_err),
            })),
            Err(_) => Ok(Response::new(ProtoResponse {
                status: SequencerStatus::InternalError.into(),
                sequence_number: 0,
                error_details: "Worker dropped response channel".to_string(),
            })),
        }
    }

async fn update_root(
        &self,
        request: Request<RootUpdate>,
    ) -> Result<Response<ProtoResponse>, Status> {
        let req = request.into_inner();
        let (resp_tx, resp_rx) = oneshot::channel();

        let item = UpdateBatchItem {
            payload: UpdatePayload {
                old_root: req.old_certificate_root,
                new_root: req.new_certificate_root,
                signature: req.signature,
                public_key: req.public_key,
            },
            respond_to: resp_tx,
        };

        if self.tx_upate.send(item).await.is_err() {
            return Ok(Response::new(ProtoResponse {
                status: SequencerStatus::InternalError.into(),
                sequence_number: 0,
                error_details: "Update channel closed".to_string(),
            }));
        }

        match resp_rx.await {
            Ok(Ok(sequence_number)) => Ok(Response::new(ProtoResponse {
                status: SequencerStatus::Accepted.into(),
                sequence_number,
                error_details: String::new(),
            })),
            Ok(Err(db_err)) => Ok(Response::new(ProtoResponse {
                status: SequencerStatus::InternalError.into(),
                sequence_number: 0,
                error_details: format!("DB Error: {}", db_err),
            })),
            Err(_) => Ok(Response::new(ProtoResponse {
                status: SequencerStatus::InternalError.into(),
                sequence_number: 0,
                error_details: "Worker dropped response channel".to_string(),
            })),
        }
    }
}
