use std::error::Error;
use rocksdb::{Options, ReadOptions, WriteBatch, DB};
use tonic::async_trait;
use crate::store::Store;
use super::SequencerStore;

#[async_trait]
impl Store for SequencerStore {
    async fn insert(
        &self,
        root: [u8; 32],
        leaves: Vec<[u8; 32]>,
    ) -> Result<u32, Box<dyn Error + Send + Sync>> {
        let next_seq = self.get_latest_seq_internal() + 1;
        let seq_bytes = next_seq.to_be_bytes();
        let mut wb = WriteBatch::default();

        wb.put(Self::root_key(next_seq), root);
        wb.put(Self::leaves_key(next_seq), leaves.as_flattened());

        for leaf in &leaves {
            wb.put(Self::leaf_idx_key(leaf), seq_bytes);
        }

        wb.put(b"meta:latest", seq_bytes);
        self.db.write(wb)?;

        Ok(next_seq)
    }

    async fn get_by_leaf(
        &self,
        leaf: [u8; 32],
    ) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>> {
        let seq_bytes = self
            .db
            .get(Self::leaf_idx_key(&leaf))?
            .ok_or_else(|| "Leaf not found in index")?;

        let seq_number = u32::from_be_bytes(seq_bytes[..4].try_into()?);

        let bytes = self
            .db
            .get(Self::leaves_key(seq_number))?
            .ok_or_else(|| format!("Sequence number {seq_number} not found"))?;

        Ok(bytes
            .chunks_exact(32)
            .map(|chunk| chunk.try_into().unwrap())
            .collect())
    }

    async fn get_root_by_seq_numbers(
        &self,
        seq_numbers: &[u32],
    ) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>> {
        let keys: Vec<[u8; 5]> = seq_numbers.iter().copied().map(Self::root_key).collect();
        let results = self.db.multi_get_opt(&keys, &ReadOptions::default());

        let mut roots = Vec::with_capacity(seq_numbers.len());
        for (seq, res) in seq_numbers.iter().zip(results) {
            let bytes = res?.ok_or_else(|| format!("Root for seq {seq} not found"))?;
            roots.push(bytes[..32].try_into()?);
        }

        Ok(roots)
    }

    //get (root,[leaves])
    async fn get_leaves_set_by_seq_number(
        &self,
        seq_numbers: &[u32],
    ) -> Result<Vec<(u32, Vec<[u8; 32]>)>, Box<dyn Error + Send + Sync>> {
        let keys: Vec<[u8; 5]> = seq_numbers.iter().copied().map(Self::leaves_key).collect();
        let results = self.db.multi_get_opt(&keys, &ReadOptions::default());

        let mut output = Vec::with_capacity(seq_numbers.len());
        for (seq, res) in seq_numbers.iter().zip(results) {
            let bytes = res?.ok_or_else(|| format!("Sequence {seq} not found"))?;
            let leaves = bytes
                .chunks_exact(32)
                .map(|chunk| chunk.try_into().unwrap())
                .collect();
            output.push((*seq, leaves));
        }
        Ok(output)
    }

    async fn update_leaves_by_indices(
        &self,
        updates: &[(u32, Vec<(usize, [u8; 32])>)],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut wb = WriteBatch::default();

        for (seq_num, leaf_updates) in updates {
            let seq_bytes = seq_num.to_be_bytes();
            let key = Self::leaves_key(*seq_num);

            let raw_bytes = self
                .db
                .get(key)?
                .ok_or_else(|| format!("Sequence number {seq_num} not found"))?;

            let mut leaves: Vec<[u8; 32]> = raw_bytes
                .chunks_exact(32)
                .map(|c| c.try_into().unwrap())
                .collect();

            for &(idx, new_leaf) in leaf_updates {
                if idx >= leaves.len() {
                    return Err(format!(
                        "Index {idx} out of bounds for sequence {seq_num} (len: {})",
                        leaves.len()
                    )
                    .into());
                }

                let old_leaf = leaves[idx];
                wb.delete(Self::leaf_idx_key(&old_leaf));
                wb.put(Self::leaf_idx_key(&new_leaf), seq_bytes);

                leaves[idx] = new_leaf;
            }

            wb.put(key, leaves.as_flattened());
        }

        self.db.write(wb)?;
        Ok(())
    }
}