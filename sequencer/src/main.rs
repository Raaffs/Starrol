mod config;
mod crypto;
mod internal;
mod rpc;
mod batcher;

use std::sync::{Arc, Mutex};
use crate::crypto::secp256k1::Secp256k1;
use config::Config;
use tokio::sync::mpsc;
use tonic::transport::Server;
use crate::rpc::sequencer::root_anchoring_server::RootAnchoringServer;
use crate::rpc::SequencerServerImpl;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load().expect("Failed to load config");

    let (tx_submit, rx_submit) = mpsc::channel(1024);
    let (tx_submit_batches, rx_submit_batches) = mpsc::channel(64);

    let (tx_update, rx_update) = mpsc::channel(1024);
    let (tx_update_batches, rx_update_batches) = mpsc::channel(64);

    let submit_batcher_config = batcher::batcher::BatcherConfig {
        max_batch_size: config.max_batch_size,
        max_wait_time: config.max_wait_ms,
    };
    let mut submit_batcher = batcher::batcher::BatcherEngine::new(submit_batcher_config, rx_submit, tx_submit_batches);
    tokio::spawn(async move { submit_batcher.run().await });

    let update_batcher_config = batcher::batcher::BatcherConfig {
        max_batch_size: config.max_batch_size,
        max_wait_time: config.max_wait_ms,
    };
    let mut update_batcher = batcher::batcher::BatcherEngine::new(update_batcher_config, rx_update, tx_update_batches);
    tokio::spawn(async move { update_batcher.run().await });

    // TODO: load private key bytes from a secrets store / env / config
    let private_key_bytes: [u8; 32] = [0u8; 32]; // replace with real key bytes
    let signer: Arc<Mutex<dyn crate::batcher::signer::DigitalSignatureService + Send>> =
        Arc::new(Mutex::new(Secp256k1::new(&private_key_bytes).expect("Invalid private key")));
        
    let server = SequencerServerImpl::new(tx_submit, rx_submit_batches, tx_update, rx_update_batches, signer);

    let addr = config.rpc_address.parse()?;
    println!("Sequencer listening on {}", addr);

    Server::builder()
        .add_service(RootAnchoringServer::new(server))
        .serve(addr)
        .await?;

    Ok(())

}