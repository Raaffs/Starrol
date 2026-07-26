use k256::ecdsa::{SigningKey, VerifyingKey, Signature, signature::hazmat::PrehashSigner};
use tonic::Request;
use std::error::Error;

pub mod sequencer {
    tonic::include_proto!("sequencer");
}

use sequencer::root_anchoring_client::RootAnchoringClient;
use sequencer::RootSubmission;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. Load the private key from sequencer/.env
    dotenvy::from_path("sequencer/.env").ok(); // fallback to standard env if not found
    let secpk_hex = std::env::var("secpk").expect("`secpk` environment variable not set");
    let key_bytes = hex::decode(secpk_hex.trim())?;
    let signing_key = SigningKey::from_slice(&key_bytes)?;
    
    // 2. Derive the verifying (public) key and serialize to SEC1 bytes (compressed)
    let verifying_key = VerifyingKey::from(&signing_key);
    let public_key_bytes = verifying_key.to_sec1_bytes().to_vec();

    // 3. Define a dummy certificate root hash (32 bytes)
    // E.g., SHA256 of some data, or all zeroes, or whatever we want. Let's make it a nice 32-byte hash.
    let mut certificate_root = [0u8; 32];
    for i in 0..32 {
        certificate_root[i] = (i + 1) as u8;
    }

    // 4. Sign the certificate root
    let sig: Signature = signing_key.sign_prehash(&certificate_root)?;
    let signature_bytes = sig.to_vec();

    println!("Connecting to sequencer RPC endpoint...");
    // 5. Connect to the gRPC endpoint (port 50051 as configured)
    let mut client = RootAnchoringClient::connect("http://127.0.0.1:50051").await?;

    let request = Request::new(RootSubmission {
        certificate_root: certificate_root.to_vec(),
        signature: signature_bytes,
        public_key: public_key_bytes,
    });

    println!("Sending RootSubmission request:");
    println!("  Certificate Root (hex): {}", hex::encode(&certificate_root));
    println!("  Public Key (hex):       {}", hex::encode(&request.get_ref().public_key));
    println!("  Signature (hex):        {}", hex::encode(&request.get_ref().signature));

    let response = client.submit_root(request).await?;
    println!("Received response: {:?}", response.into_inner());

    Ok(())
}
