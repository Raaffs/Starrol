pub mod service;
pub mod worker;

// Generate protobuf types ONCE
pub mod sequencer {
    tonic::include_proto!("sequencer");
}

pub use sequencer::root_anchoring_server::RootAnchoringServer;

// Shared payload structs
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
    pub respond_to: tokio::sync::oneshot::Sender<Result<u64, String>>,
}

pub struct UpdateBatchItem {
    pub payload: UpdatePayload,
    pub respond_to: tokio::sync::oneshot::Sender<Result<u64, String>>,
}

pub struct SequencerServerImpl {
    pub tx_submit: tokio::sync::mpsc::Sender<SubmissionBatchItem>,
    pub tx_upate: tokio::sync::mpsc::Sender<UpdateBatchItem>,
    pub digital_signer: std::sync::Arc<std::sync::Mutex<dyn crate::store::DigitalSignatureService + Send>>,
}

