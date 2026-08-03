pub mod postgres;
use std::error::Error;
pub trait Store {
    async fn insert(&self, root: [u8; 32], leaves: Vec<[u8; 32]>) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn update_by_seq_number(
        &self,
        seq_number: u32,
        old: [u8; 32],
        new: [u8; 32],
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn update_by_leaf(&self, old: [u8; 32], new: [u8; 32]) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn get_by_leaf(&self, leaf: [u8; 32]) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>>;

    async fn get_by_seq_numbers(&self, seq_numbers: Vec<u32>) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>>;

    async fn get_latest_seq_number(&self) -> Result<u32, Box<dyn Error + Send + Sync>>;
}
