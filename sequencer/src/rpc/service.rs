use crate::rpc::{SequencerServerImpl,UpdateBatchItem,UpdatePayload,SubmissionBatchItem,SubmissionPayload};
use tokio::sync::{oneshot};

use crate::rpc::sequencer::root_anchoring_server::RootAnchoring;
use crate::rpc::sequencer::{
    RootSubmission, RootUpdate, SequencerStatus, Response as ProtoResponse
};
use tonic::{Request, Response, Status};


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