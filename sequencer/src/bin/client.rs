use k256::ecdsa::{signature::hazmat::PrehashSigner, Signature, SigningKey, VerifyingKey};
use std::error::Error;
use std::io::Read;
use tokio::time::{sleep, Duration};
use tonic::Request;

pub mod sequencer {
    include!("../pb/sequencer.rs");
}

use sequencer::root_anchoring_client::RootAnchoringClient;
use sequencer::{RootSubmission, RootUpdate, SequencerStatus};

// ── helpers ──────────────────────────────────────────────────────────────────

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

/// Sign a 32-byte message hash and return the raw signature bytes.
fn sign(signing_key: &SigningKey, msg: &[u8; 32]) -> Vec<u8> {
    let sig: Signature = signing_key.sign_prehash(msg).expect("sign_prehash failed");
    sig.to_vec()
}

fn sep(title: &str) {
    println!("\n{}", "─".repeat(60));
    println!("  {}", title);
    println!("{}\n", "─".repeat(60));
}

// ── main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Sequencer batch-submit + multi-update test ===\n");

    let client = RootAnchoringClient::connect("http://127.0.0.1:50051").await?;
    println!("Connected to sequencer at 127.0.0.1:50051");

    // ── STEP 1: generate 5 issuers, each with their own key-pair and root ────

    sep("STEP 1 — submitting 5 roots concurrently");

    // (signing_key, verifying_key, current_root)
    let issuers: Vec<(SigningKey, VerifyingKey, [u8; 32])> = (0..5)
        .map(|_| {
            let (sk, vk) = gen_keypair();
            let root = random_bytes::<32>();
            (sk, vk, root)
        })
        .collect();

    for (i, (_, _, root)) in issuers.iter().enumerate() {
        println!("  issuer[{}] root: {}", i, hex::encode(root));
    }
    println!();

    // Spawn concurrent tokio tasks for each submission without waiting for individual responses
    let mut submit_handles = Vec::new();

    for (i, (sk, vk, root)) in issuers.iter().enumerate() {
        let mut c = client.clone();
        let sig = sign(sk, root);
        let pub_key = vk.to_sec1_bytes().to_vec();
        let root_vec = root.to_vec();

        let h = tokio::spawn(async move {
            let resp = c
                .submit_root(Request::new(RootSubmission {
                    certificate_root: root_vec,
                    signature: sig,
                    public_key: pub_key,
                }))
                .await;
            (i, resp)
        });
        submit_handles.push(h);
    }

    // Collect responses from all concurrent submissions
    let mut seq_map: Vec<Option<u64>> = vec![None; issuers.len()];

    for h in submit_handles {
        match h.await? {
            (i, Ok(resp)) => {
                let inner = resp.into_inner();
                let status =
                    SequencerStatus::try_from(inner.status).unwrap_or(SequencerStatus::Unknown);

                println!(
                    "  submit[{}] → status={:?}  seq_no={}{}",
                    i,
                    status,
                    inner.sequence_number,
                    if inner.error_details.is_empty() {
                        String::new()
                    } else {
                        format!("  ERR: {}", inner.error_details)
                    }
                );

                if status == SequencerStatus::Accepted {
                    seq_map[i] = Some(inner.sequence_number);
                }
            }
            (i, Err(e)) => {
                eprintln!("  submit[{}] | gRPC error: {}", i, e);
            }
        }
    }

    // Collect the successfully accepted issuers.
    let accepted: Vec<(usize, u64)> = seq_map
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.map(|seq| (i, seq)))
        .collect();

    if accepted.is_empty() {
        eprintln!("\nNo submissions accepted — aborting.");
        return Ok(());
    }

    println!("\n  {} / 5 roots accepted.", accepted.len());

    // ── STEP 2: wait briefly for the server to finish processing ─────────────

    sep("STEP 2 — waiting 300 ms for the server to flush the batch");
    sleep(Duration::from_millis(300)).await;
    println!("  Done waiting.");

    // ── STEP 3: send 4 simultaneous updates ──────────────────────────────────

    sep("STEP 3 — sending up to 4 concurrent root updates");

    // We'll update at most 4 of the accepted issuers.
    let to_update: Vec<(usize, u64)> = accepted.into_iter().take(4).collect();

    // Prepare (old_root, new_root, seq_no) for each update.
    let mut update_specs: Vec<(usize, [u8; 32], [u8; 32], u64)> = to_update
        .iter()
        .map(|&(issuer_idx, seq_no)| {
            let old_root = issuers[issuer_idx].2;
            let new_root = random_bytes::<32>();
            (issuer_idx, old_root, new_root, seq_no)
        })
        .collect();

    for (idx, old, new, seq) in &update_specs {
        println!(
            "  issuer[{}] seq={} | old={} → new={}",
            idx,
            seq,
            hex::encode(old),
            hex::encode(new)
        );
    }
    println!();

    // Clone a handle per update and fire them all concurrently.
    let mut update_handles = Vec::new();
    for (issuer_idx, old_root, new_root, seq_no) in update_specs.drain(..) {
        let mut c = client.clone();
        let (sk, vk, _) = &issuers[issuer_idx];
        let sig = sign(sk, &new_root);
        let pub_key = vk.to_sec1_bytes().to_vec();

        let h = tokio::spawn(async move {
            let resp = c
                .update_root(Request::new(RootUpdate {
                    old_certificate_root: old_root.to_vec(),
                    new_certificate_root: new_root.to_vec(),
                    sequence_number: seq_no,
                    signature: sig,
                    public_key: pub_key,
                }))
                .await;
            (issuer_idx, seq_no, old_root, new_root, resp)
        });
        update_handles.push(h);
    }

    sep("STEP 4 — collecting update responses");

    for h in update_handles {
        match h.await? {
            (issuer_idx, seq_no, old_root, new_root, Ok(resp)) => {
                let inner = resp.into_inner();
                let status =
                    SequencerStatus::try_from(inner.status).unwrap_or(SequencerStatus::Unknown);
                println!(
                    "  issuer[{}] seq={} | update → status={:?}  returned_seq={}{}",
                    issuer_idx,
                    seq_no,
                    status,
                    inner.sequence_number,
                    if inner.error_details.is_empty() {
                        String::new()
                    } else {
                        format!("  ERR: {}", inner.error_details)
                    }
                );
                if status == SequencerStatus::Accepted {
                    println!(
                        "    old={} → new={}  ✓",
                        hex::encode(old_root),
                        hex::encode(new_root)
                    );
                }
            }
            (issuer_idx, seq_no, _, _, Err(e)) => {
                eprintln!(
                    "  issuer[{}] seq={} | gRPC error: {}",
                    issuer_idx, seq_no, e
                );
            }
        }
    }

    println!("\n=== Done ===");
    Ok(())
}