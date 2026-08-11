pub mod rocks_queries;
use std::error::Error;
use rocksdb::{Options,  DB};
pub struct SequencerStore {
    db: DB,
}

impl SequencerStore {
    pub fn new(path: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path)?;
        Ok(Self { db })
    }

    #[inline]
    fn root_key(seq: u32) -> [u8; 5] {
        let mut key = [0u8; 5];
        key[0] = b'r';
        key[1..5].copy_from_slice(&seq.to_be_bytes());
        key
    }

    #[inline]
    fn leaves_key(seq: u32) -> [u8; 5] {
        let mut key = [0u8; 5];
        key[0] = b'l';
        key[1..5].copy_from_slice(&seq.to_be_bytes());
        key
    }

    #[inline]
    fn leaf_idx_key(leaf: &[u8; 32]) -> [u8; 33] {
        let mut key = [0u8; 33];
        key[0] = b'i';
        key[1..33].copy_from_slice(leaf);
        key
    }

    fn get_latest_seq_internal(&self) -> u32 {
        match self.db.get(b"meta:latest") {
            Ok(Some(bytes)) => u32::from_be_bytes(bytes[..4].try_into().unwrap_or_default()),
            _ => 0,
        }
    }
}