use crate::store::Store;
use async_trait::async_trait;
use std::error::Error;
use super::PostgresDB;

#[async_trait]
impl Store for PostgresDB {
    async fn insert(
        &self,
        root: [u8; 32],
        leaves: Vec<[u8; 32]>,
    ) -> Result<u32, Box<dyn Error + Send + Sync>> {
        let mut tx = self.pool.begin().await?;

        let leaves_bytes: Vec<Vec<u8>> = leaves.iter().map(|l| l.to_vec()).collect();

        let row: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO roots (root, leaves)
            VALUES ($1, $2)
            RETURNING sequence_number
            "#,
        )
        .bind(root.to_vec())
        .bind(&leaves_bytes)
        .fetch_one(&mut *tx)
        .await?;

        let seq_num = row.0;

        for leaf in leaves {
            sqlx::query(
                r#"
                INSERT INTO leaves (leaf, sequence_number)
                VALUES ($1, $2)
                "#,
            )
            .bind(leaf.to_vec())
            .bind(seq_num)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(seq_num as u32)
    }

    async fn get_by_leaf(
        &self,
        leaf: [u8; 32],
    ) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>> {
        let row: (Vec<Vec<u8>>,) = sqlx::query_as(
            r#"
            SELECT r.leaves
            FROM roots r
            INNER JOIN leaves l ON r.sequence_number = l.sequence_number
            WHERE l.leaf = $1
            "#,
        )
        .bind(leaf.to_vec())
        .fetch_one(&self.pool)
        .await?;

        let leaves = row.0.into_iter().filter_map(|b| b.try_into().ok()).collect();
        Ok(leaves)
    }

    async fn get_root_by_seq_numbers(
        &self,
        seq_numbers: &[u32],
    ) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>> {
        let seq_ids: Vec<i32> = seq_numbers.iter().map(|&n| n as i32).collect();

        let rows: Vec<(Vec<u8>,)> = sqlx::query_as(
            r#"
            SELECT root
            FROM roots
            WHERE sequence_number = ANY($1)
            "#,
        )
        .bind(&seq_ids)
        .fetch_all(&self.pool)
        .await?;

        let roots = rows.into_iter().filter_map(|(b,)| b.try_into().ok()).collect();
        Ok(roots)
    }

    async fn get_leaves_set_by_seq_number(
        &self,
        seq_numbers: &[u32],
    ) -> Result<Vec<(u32, Vec<[u8; 32]>)>, Box<dyn Error + Send + Sync>> {
        let seq_ids: Vec<i32> = seq_numbers.iter().map(|&n| n as i32).collect();

        let rows: Vec<(i32, Vec<Vec<u8>>)> = sqlx::query_as(
            r#"
            SELECT sequence_number, leaves
            FROM roots
            WHERE sequence_number = ANY($1)
            "#,
        )
        .bind(&seq_ids)
        .fetch_all(&self.pool)
        .await?;

        let mapped_leaves = rows
            .into_iter()
            .map(|(seq_num, row_leaves)| {
                let parsed_leaves = row_leaves
                    .into_iter()
                    .filter_map(|b| b.try_into().ok())
                    .collect();
                (seq_num as u32, parsed_leaves)
            })
            .collect();

        Ok(mapped_leaves)
    }

    async fn update_leaves_by_indices(
        &self,
        updates: &[(u32, Vec<(usize, [u8; 32] )>)],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut tx = self.pool.begin().await?;

        for (seq_num, leaf_updates) in updates {
            let seq_i32 = *seq_num as i32;

            let row: (Vec<Vec<u8>>,) = sqlx::query_as(
                r#"
                SELECT leaves FROM roots WHERE sequence_number = $1 FOR UPDATE
                "#,
            )
            .bind(seq_i32)
            .fetch_one(&mut *tx)
            .await?;

            let mut leaves: Vec<[u8; 32]> = row
                .0
                .into_iter()
                .filter_map(|b| b.try_into().ok())
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

                sqlx::query(
                    r#"
                    UPDATE leaves
                    SET leaf = $1
                    WHERE sequence_number = $2 AND leaf = $3
                    "#,
                )
                .bind(new_leaf.to_vec())
                .bind(seq_i32)
                .bind(old_leaf.to_vec())
                .execute(&mut *tx)
                .await?;

                leaves[idx] = new_leaf;
            }

            let updated_bytes: Vec<Vec<u8>> = leaves.iter().map(|l| l.to_vec()).collect();

            sqlx::query(
                r#"
                UPDATE roots
                SET leaves = $1
                WHERE sequence_number = $2
                "#,
            )
            .bind(&updated_bytes)
            .bind(seq_i32)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}