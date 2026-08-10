pub mod postgres;
use std::error::Error;

use tonic::async_trait;
#[async_trait]
pub trait Store: Send + Sync {
    async fn insert(&self, root: [u8; 32], leaves: Vec<[u8; 32]>) -> Result<u64, Box<dyn Error + Send + Sync>>;

    async fn update_by_seq_number(
        &self,
        seq_number: u32,
        old: [u8; 32],
        new: [u8; 32],
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn update_by_leaf(&self, old: [u8; 32], new: [u8; 32]) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn get_by_leaf(&self, leaf: [u8; 32]) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>>;

    async fn get_latest_seq_number(&self) -> Result<u32, Box<dyn Error + Send + Sync>>;

    async fn get_leaves_by_seq_number(
        &self,
        seq_number: u32,
    ) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>>;

    async fn get_leaves_set_by_seq_number(
        &self,
        seq_numbers: &[u64],
    ) -> Result<Vec<(u64, Vec<[u8; 32]>)>, Box<dyn Error + Send + Sync>>;

    async fn get_root_by_seq_numbers(
        &self,
        seq_numbers: Vec<u64>,
    ) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>>;

    async fn update_root_by_seq_number(
        &self,
        seq_number: u32,
        new_root: [u8; 32],
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Atomically replaces `old_leaves[i]` with `new_leaves[i]` in both the `leaves` table
    /// and the `roots.leaves` array, then sets the new Merkle root for the given sequence number.
    async fn update_leaves_and_root(
        &self,
        seq_number: u32,
        old_leaves: &[[u8; 32]],
        new_leaves: &[[u8; 32]],
        new_root: [u8; 32],
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}

pub trait DigitalSignatureService {
    fn sign(&self, msg_hash: &[u8; 32]) -> Result<Vec<u8>, Box<dyn Error>>;
    fn verify(&self, public_key_bytes: &[u8], msg_hash: &[u8; 32], sig: &[u8]) -> Result<bool, Box<dyn Error>>;
}
