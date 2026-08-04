use k256::ecdsa::{SigningKey, Signature, signature::hazmat::PrehashSigner, VerifyingKey};
use tonic::Request;
use std::error::Error;
use std::io::Read;

pub mod sequencer {
    include!("../pb/sequencer.rs");
}

use sequencer::root_anchoring_client::RootAnchoringClient;
use sequencer::{RootSubmission, RootUpdate, SequencerStatus};

/// Read exactly `N` random bytes from /dev/urandom.
fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    std::fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut buf)
        .unwrap();
    buf
}

/// Generate a fresh secp256k1 key-pair.
fn gen_keypair() -> (SigningKey, VerifyingKey) {
    loop {
        let bytes = random_bytes::<32>();
        if let Ok(sk) = SigningKey::from_slice(&bytes) {
            let vk = VerifyingKey::from(&sk);
            return (sk, vk);
        }
    }
}

/// Sign a 32-byte message hash and return DER-encoded signature bytes.
fn sign(signing_key: &SigningKey, msg: &[u8; 32]) -> Vec<u8> {
    let sig: Signature = signing_key.sign_prehash(msg).expect("sign_prehash failed");
    sig.to_vec()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Sequencer insert-then-update test ===\n");

    let mut client = RootAnchoringClient::connect("http://127.0.0.1:50051").await?;
    println!("Connected to sequencer at 127.0.0.1:50051\n");

    // ── Step 1: generate 3 certificate roots and submit them in one batch ──────

    // We use 3 different key-pairs to simulate 3 issuers.
    let (sk_a, vk_a) = gen_keypair();
    let (sk_b, vk_b) = gen_keypair();
    let (sk_c, vk_c) = gen_keypair();

    let root_a: [u8; 32] = random_bytes();
    let root_b: [u8; 32] = random_bytes();
    let root_c: [u8; 32] = random_bytes();

    println!("--- Submitting 3 roots ---");
    println!("  root_a: {}", hex::encode(root_a));
    println!("  root_b: {}", hex::encode(root_b));
    println!("  root_c: {}", hex::encode(root_c));
    println!();

    let submissions: Vec<(Vec<u8>, &SigningKey, &VerifyingKey)> = vec![
        (root_a.to_vec(), &sk_a, &vk_a),
        (root_b.to_vec(), &sk_b, &vk_b),
        (root_c.to_vec(), &sk_c, &vk_c),
    ];

    // Fire all three submissions concurrently and collect their sequence numbers.
    let mut seq_nos: Vec<(Vec<u8>, u64)> = Vec::new(); // (root, seq_no)

    for (root_vec, sk, vk) in &submissions {
        let root_arr: [u8; 32] = root_vec.as_slice().try_into().unwrap();
        let sig_bytes = sign(sk, &root_arr);
        let pub_key_bytes = vk.to_sec1_bytes().to_vec();

        let req = Request::new(RootSubmission {
            certificate_root: root_vec.clone(),
            signature: sig_bytes,
            public_key: pub_key_bytes,
        });

        let resp = client.submit_root(req).await?.into_inner();
        let status = SequencerStatus::try_from(resp.status).unwrap_or(SequencerStatus::Unknown);

        println!(
            "  submit_root({}) → status={:?}, seq_no={}",
            hex::encode(root_vec),
            status,
            resp.sequence_number,
        );

        if status == SequencerStatus::Accepted {
            seq_nos.push((root_vec.clone(), resp.sequence_number));
        } else {
            eprintln!("    ERROR: {}", resp.error_details);
        }
    }

    if seq_nos.is_empty() {
        eprintln!("\nNo submissions were accepted; cannot proceed with update.");
        return Ok(());
    }

    println!();

    // ── Step 2: update root_a to a freshly generated root_a_new ───────────────

    let (old_root_vec, seq_no) = &seq_nos[0];
    let old_root_arr: [u8; 32] = old_root_vec.as_slice().try_into().unwrap();

    let new_root_a: [u8; 32] = random_bytes();

    // Re-use the same key-pair that originally signed root_a (sk_a / vk_a).
    let sig_update = sign(&sk_a, &new_root_a);
    let pub_key_update = vk_a.to_sec1_bytes().to_vec();

    println!("--- Updating root_a (seq_no={}) ---", seq_no);
    println!("  old root: {}", hex::encode(old_root_arr));
    println!("  new root: {}", hex::encode(new_root_a));
    println!();

    let update_req = Request::new(RootUpdate {
        old_certificate_root: old_root_vec.clone(),
        new_certificate_root: new_root_a.to_vec(),
        sequence_number: *seq_no,
        signature: sig_update,
        public_key: pub_key_update,
    });

    let update_resp = client.update_root(update_req).await?.into_inner();
    let update_status =
        SequencerStatus::try_from(update_resp.status).unwrap_or(SequencerStatus::Unknown);

    println!(
        "  update_root → status={:?}, seq_no={}",
        update_status, update_resp.sequence_number
    );
    if !update_resp.error_details.is_empty() {
        eprintln!("  error_details: {}", update_resp.error_details);
    }

    println!("\n=== Done ===");
    Ok(())
}
