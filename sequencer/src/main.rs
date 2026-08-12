mod batcher;
mod config;
mod crypto;
mod internal;
mod rpc;
mod store;

use config::Config;
use crate::crypto::secp256k1::Secp256k1;
use crate::rpc::SequencerServerImpl;
// Import the generated gRPC server struct:
use crate::rpc::sequencer::root_anchoring_server::RootAnchoringServer;
use std::sync::{Arc, Mutex};
use store::rocks::SequencerStore;
use store::{Store,DigitalSignatureService};
use tokio::sync::mpsc;
use tracing::subscriber::set_global_default;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, fmt};
use tracing_appender::non_blocking::WorkerGuard;
use tonic::transport::Server;

pub fn init_logging() -> WorkerGuard {
    let (non_blocking, guard) = tracing_appender::non_blocking(std::io::stdout());

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .json()
                .flatten_event(true)
                .with_writer(non_blocking),
        )
        .init();

    guard 
}

fn init_store() -> Arc<dyn Store + Send + Sync> {
    let db_path = std::env::var("ROCKSDB_PATH")
        .unwrap_or_else(|_| "data/sequencer_db".to_string());

    Arc::new(SequencerStore::new(&db_path).expect("Failed to open RocksDB"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt::init();

    let config = Config::load().expect("Failed to load config");
    let store = init_store();

    let (tx_submit, rx_submit) = mpsc::channel(1024);
    let (tx_submit_batches, rx_submit_batches) = mpsc::channel(64);

    let (tx_update, rx_update) = mpsc::channel(1024);
    let (tx_update_batches, rx_update_batches) = mpsc::channel(64);

    let submit_batcher_config = batcher::batcher::BatcherConfig {
        max_batch_size: config.max_batch_size,
        max_wait_time: config.max_wait_ms,
    };
    let mut submit_batcher =
        batcher::batcher::BatcherEngine::new(submit_batcher_config, rx_submit, tx_submit_batches);
    tokio::spawn(async move { submit_batcher.run().await });

    let update_batcher_config = batcher::batcher::BatcherConfig {
        max_batch_size: config.max_batch_size,
        max_wait_time: config.max_wait_ms,
    };
    let mut update_batcher =
        batcher::batcher::BatcherEngine::new(update_batcher_config, rx_update, tx_update_batches);
    tokio::spawn(async move { update_batcher.run().await });

    // Load private key from sequencer/.env (key: secpk, hex-encoded 32 bytes)
    dotenvy::from_path("sequencer/.env").expect("Failed to load sequencer/.env");
    let secpk_hex = std::env::var("secpk").expect("`secpk` not set in sequencer/.env");
    let key_bytes: Vec<u8> = hex::decode(secpk_hex.trim())
        .expect("`secpk` must be a valid hex string");
    let private_key_bytes: [u8; 32] = key_bytes
        .try_into()
        .expect("`secpk` must be exactly 32 bytes (64 hex chars)");
    let signer: Arc<Mutex<dyn DigitalSignatureService + Send + 'static>> = Arc::new(
        Mutex::new(Secp256k1::new(&private_key_bytes).expect("Invalid secp256k1 private key")),
    );

    let server = SequencerServerImpl::new(
        tx_submit,
        rx_submit_batches,
        tx_update,
        rx_update_batches,
        signer,
        store
    );

    let addr = config.rpc_address.parse()?;
    println!("Sequencer listening on {}", addr);

    Server::builder()
        .add_service(RootAnchoringServer::new(server))
        .serve(addr)
        .await?;

    Ok(())
}
