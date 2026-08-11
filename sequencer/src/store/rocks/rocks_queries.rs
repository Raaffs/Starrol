use rocksdb::{Options, ReadOptions, WriteBatch, DB};
use std::error::Error;

type StoreResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub struct SequencerStore {
    db: DB,
}

impl SequencerStore {
    pub fn new(path: &str) -> StoreResult<Self> {
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

    pub async fn insert(&self, root: [u8; 32], leaves: Vec<[u8; 32]>) -> StoreResult<u64> {
        let next_seq = self.get_latest_seq_number().await.unwrap_or(0) + 1;
        let seq_bytes = next_seq.to_be_bytes();
        let mut wb = WriteBatch::default();

        // 1. Store Root
        wb.put(Self::root_key(next_seq), root);

        // 2. Store Leaves (Zero-copy flattening)
        wb.put(Self::leaves_key(next_seq), leaves.as_flattened());

        // 3. Reverse Index: leaf -> sequence_number
        for leaf in &leaves {
            wb.put(Self::leaf_idx_key(leaf), seq_bytes);
        }

        // 4. Update latest sequence number pointer
        wb.put(b"meta:latest", seq_bytes);

        self.db.write(wb)?;
        Ok(next_seq as u64)
    }

    pub async fn get_latest_seq_number(&self) -> StoreResult<u32> {
        match self.db.get(b"meta:latest")? {
            Some(bytes) => Ok(u32::from_be_bytes(bytes[..4].try_into()?)),
            None => Ok(0),
        }
    }

    pub async fn get_leaves_by_seq_number(&self, seq_number: u32) -> StoreResult<Vec<[u8; 32]>> {
        let bytes = self
            .db
            .get(Self::leaves_key(seq_number))?
            .ok_or_else(|| format!("Sequence number {seq_number} not found"))?;

        Ok(bytes
            .chunks_exact(32)
            .map(|chunk| chunk.try_into().unwrap())
            .collect())
    }

    /// Parallel lookup for multiple sequence numbers preserving insertion order.
    pub async fn get_leaves_set_by_seq_number(
        &self,
        seq_numbers: &[u32],
    ) -> StoreResult<Vec<(u32, Vec<[u8; 32]>)>> {
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

    /// Parallel lookup to retrieve Merkle roots for multiple sequence numbers.
    pub async fn get_root_by_seq_numbers(
        &self,
        seq_numbers: &[u32],
    ) -> StoreResult<Vec<[u8; 32]>> {
        let keys: Vec<[u8; 5]> = seq_numbers.iter().copied().map(Self::root_key).collect();
        let results = self.db.multi_get_opt(&keys, &ReadOptions::default());

        let mut roots = Vec::with_capacity(seq_numbers.len());

        for (seq, res) in seq_numbers.iter().zip(results) {
            let bytes = res?.ok_or_else(|| format!("Root for seq {seq} not found"))?;
            roots.push(bytes[..32].try_into()?);
        }

        Ok(roots)
    }

    /// Performs reverse lookup: locates the sequence batch containing a specific leaf.
    pub async fn get_by_leaf(&self, leaf: [u8; 32]) -> StoreResult<Vec<[u8; 32]>> {
        let seq_bytes = self
            .db
            .get(Self::leaf_idx_key(&leaf))?
            .ok_or_else(|| format!("Leaf not found in index"))?;

        let seq_number = u32::from_be_bytes(seq_bytes[..4].try_into()?);
        self.get_leaves_by_seq_number(seq_number).await
    }

    pub async fn update_root_by_seq_number(
        &self,
        seq_number: u32,
        new_root: [u8; 32],
    ) -> StoreResult<()> {
        self.db.put(Self::root_key(seq_number), new_root)?;
        Ok(())
    }

    pub async fn update_by_seq_number(
        &self,
        seq_number: u32,
        old: [u8; 32],
        new: [u8; 32],
    ) -> StoreResult<()> {
        let mut leaves = self.get_leaves_by_seq_number(seq_number).await?;

        if let Some(pos) = leaves.iter().position(|l| l == &old) {
            leaves[pos] = new;

            let mut wb = WriteBatch::default();
            wb.put(Self::leaves_key(seq_number), leaves.as_flattened());
            wb.delete(Self::leaf_idx_key(&old));
            wb.put(Self::leaf_idx_key(&new), seq_number.to_be_bytes());

            self.db.write(wb)?;
        }
        Ok(())
    }

    pub async fn update_by_leaf(&self, old: [u8; 32], new: [u8; 32]) -> StoreResult<()> {
        let seq_bytes = self
            .db
            .get(Self::leaf_idx_key(&old))?
            .ok_or_else(|| format!("Leaf not found in reverse index"))?;

        let seq_number = u32::from_be_bytes(seq_bytes[..4].try_into()?);
        self.update_by_seq_number(seq_number, old, new).await
    }

    pub async fn update_leaves_and_root(
        &self,
        seq_number: u32,
        old_leaves: &[[u8; 32]],
        new_leaves: &[[u8; 32]],
        new_root: [u8; 32],
    ) -> StoreResult<()> {
        let mut leaves = self.get_leaves_by_seq_number(seq_number).await?;
        let seq_bytes = seq_number.to_be_bytes();
        let mut wb = WriteBatch::default();

        for (old_l, new_l) in old_leaves.iter().zip(new_leaves.iter()) {
            if let Some(pos) = leaves.iter().position(|l| l == old_l) {
                leaves[pos] = *new_l;
            }
            wb.delete(Self::leaf_idx_key(old_l));
            wb.put(Self::leaf_idx_key(new_l), seq_bytes);
        }

        wb.put(Self::leaves_key(seq_number), leaves.as_flattened());
        wb.put(Self::root_key(seq_number), new_root);

        self.db.write(wb)?;
        Ok(())
    }

    /// Batch updates leaves using sequence numbers and array indices directly.
    ///
    /// Input: `&[(seq_number, vec![(leaf_index, new_leaf_hash)])]`
    pub async fn update_leaves_by_indices(
        &self,
        updates: &[(u32, Vec<(usize, [u8; 32])>)],
    ) -> StoreResult<()> {
        let mut wb = WriteBatch::default();

        for (seq_num, leaf_updates) in updates {
            let seq_bytes = seq_num.to_be_bytes();
            let key = Self::leaves_key(*seq_num);

            let mut leaves = self.get_leaves_by_seq_number(*seq_num).await?;

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