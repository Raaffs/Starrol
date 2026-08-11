use std::error::Error;
use tonic::async_trait;

pub mod postgres;
pub mod rocks;

#[async_trait]
pub trait Store: Send + Sync {
    async fn insert(
        &self,
        root: [u8; 32],
        leaves: Vec<[u8; 32]>,
    ) -> Result<u32, Box<dyn Error + Send + Sync>>;

    async fn get_by_leaf(
        &self,
        leaf: [u8; 32],
    ) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>>;

    async fn get_root_by_seq_numbers(
        &self,
        seq_numbers: &[u32],
    ) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>>;

    /// Equivalent to: SELECT sequence_number, leaves FROM roots WHERE sequence_number = ANY($1)
    async fn get_leaves_set_by_seq_number(
        &self,
        seq_numbers: &[u32],
    ) -> Result<Vec<(u32, Vec<[u8; 32]>)>, Box<dyn Error + Send + Sync>>;

    async fn update_leaves_by_indices(
        &self,
        updates: &[(u32, Vec<(usize, [u8; 32])>)],
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}

pub trait DigitalSignatureService {
    fn sign(&self, msg_hash: &[u8; 32]) -> Result<Vec<u8>, Box<dyn Error>>;
    fn verify(&self, public_key_bytes: &[u8], msg_hash: &[u8; 32], sig: &[u8]) -> Result<bool, Box<dyn Error>>;
}