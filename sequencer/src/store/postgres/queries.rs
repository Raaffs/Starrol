use crate::store::Store;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use std::error::Error;
use super::PostgresDB;



#[async_trait]
impl Store for PostgresDB {
    async fn insert(
        &self,
        root: [u8; 32],
        leaves: Vec<[u8; 32]>,
    ) -> Result<u64, Box<dyn Error + Send + Sync>> {
        let mut tx = self.pool.begin().await?;

        let leaves_bytes: Vec<Vec<u8>> = leaves.iter().map(|l| l.to_vec()).collect();

        let row: (i32,) = sqlx::query_as(
            r#"
            INSERT INTO roots (root, leaves)
            VALUES ($1, $2)
            ON CONFLICT (root) DO NOTHING
            RETURNING sequence_number
            "#,
        )
        .bind(root.to_vec())
        .bind(leaves_bytes)
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

        // Return the sequence number
        Ok(seq_num as u64)
    }

    async fn update_by_seq_number(
        &self,
        seq_number: u32,
        old: [u8; 32],
        new: [u8; 32],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            UPDATE leaves
            SET leaf = $1
            WHERE sequence_number = $2 AND leaf = $3
            "#,
        )
        .bind(new.to_vec())
        .bind(seq_number as i32)
        .bind(old.to_vec())
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            UPDATE roots
            SET leaves = array_replace(leaves, $1, $2)
            WHERE sequence_number = $3
            "#,
        )
        .bind(old.to_vec())
        .bind(new.to_vec())
        .bind(seq_number as i32)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn update_by_leaf(
        &self,
        old: [u8; 32],
        new: [u8; 32],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            UPDATE leaves
            SET leaf = $1
            WHERE leaf = $2
            "#,
        )
        .bind(new.to_vec())
        .bind(old.to_vec())
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            UPDATE roots
            SET leaves = array_replace(leaves, $1, $2)
            WHERE $1 = ANY(leaves)
            "#,
        )
        .bind(old.to_vec())
        .bind(new.to_vec())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn get_by_leaf(
        &self,
        leaf: [u8; 32],
    ) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>> {
        let rows = sqlx::query(
            r#"
            SELECT r.root
            FROM roots r
            INNER JOIN leaves l ON r.sequence_number = l.sequence_number
            WHERE l.leaf = $1
            "#,
        )
        .bind(leaf.to_vec())
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            let root_bytes: Vec<u8> = row.get("root");
            if let Ok(arr) = root_bytes.as_slice().try_into() {
                results.push(arr);
            }
        }
        Ok(results)
    }

    async fn get_root_by_seq_numbers(
        &self,
        seq_numbers: Vec<u64>,
    ) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>> {
        let seq_nums_i32: Vec<i32> = seq_numbers.into_iter().map(|n| n as i32).collect();

        let rows = sqlx::query(
            r#"
            SELECT root
            FROM roots
            WHERE sequence_number = ANY($1)
            "#,
        )
        .bind(seq_nums_i32)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            let root_bytes: Vec<u8> = row.get("root");
            if let Ok(arr) = root_bytes.as_slice().try_into() {
                results.push(arr);
            }
        }

        Ok(results)
    }

    async fn get_latest_seq_number(&self) -> Result<u32, Box<dyn Error + Send + Sync>> {
        let row: (Option<i32>,) = sqlx::query_as(
            r#"
            SELECT COALESCE(MAX(sequence_number), 0) as max_seq
            FROM roots
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0.unwrap_or(0) as u32)
    }

    async fn get_leaves_by_seq_number(
        &self,
        seq_number: u32,
    ) -> Result<Vec<[u8; 32]>, Box<dyn Error + Send + Sync>> {
        let row: (Vec<Vec<u8>>,) = sqlx::query_as(
            r#"
            SELECT leaves
            FROM roots
            WHERE sequence_number = $1
            "#,
        )
        .bind(seq_number as i32)
        .fetch_one(&self.pool)
        .await?;

        let leaves = row.0.into_iter().filter_map(|b| b.try_into().ok()).collect();
        Ok(leaves)
    }

    async fn get_leaves_set_by_seq_number(
        &self,
        seq_numbers: &[u64],
    ) -> Result<Vec<(u64, Vec<[u8; 32]>)>, Box<dyn Error + Send + Sync>> {
        let seq_ids: Vec<i32> = seq_numbers.iter().map(|&id| id as i32).collect();

        // 1. Select both sequence_number and leaves so they are explicitly bound together in each row
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

        // 2. Map each row into a tuple containing (sequence_number, parsed_leaves)
        let mapped_leaves = rows
            .into_iter()
            .map(|(seq_num, row_leaves)| {
                let parsed_leaves = row_leaves
                    .into_iter()
                    .filter_map(|b| b.try_into().ok())
                    .collect();
                (seq_num as u64, parsed_leaves)
            })
            .collect();

        Ok(mapped_leaves)
    }

    async fn update_root_by_seq_number(
        &self,
        seq_number: u32,
        new_root: [u8; 32],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        sqlx::query(
            r#"
            UPDATE roots
            SET root = $1
            WHERE sequence_number = $2
            "#,
        )
        .bind(new_root.to_vec())
        .bind(seq_number as i32)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_leaves_and_root(
        &self,
        seq_number: u32,
        old_leaves: &[[u8; 32]],
        new_leaves: &[[u8; 32]],
        new_root: [u8; 32],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        assert_eq!(old_leaves.len(), new_leaves.len(), "old/new leaf slices must have the same length");

        let mut tx = self.pool.begin().await?;

        for (old, new) in old_leaves.iter().zip(new_leaves.iter()) {
            // Replace the leaf row itself
            sqlx::query(
                r#"
                UPDATE leaves
                SET leaf = $1
                WHERE sequence_number = $2 AND leaf = $3
                "#,
            )
            .bind(new.to_vec())
            .bind(seq_number as i32)
            .bind(old.to_vec())
            .execute(&mut *tx)
            .await?;

            // Replace the value inside the denormalised roots.leaves array
            sqlx::query(
                r#"
                UPDATE roots
                SET leaves = array_replace(leaves, $1, $2)
                WHERE sequence_number = $3
                "#,
            )
            .bind(old.to_vec())
            .bind(new.to_vec())
            .bind(seq_number as i32)
            .execute(&mut *tx)
            .await?;
        }

        // Finally, update the Merkle root for this sequence number
        sqlx::query(
            r#"
            UPDATE roots
            SET root = $1
            WHERE sequence_number = $2
            "#,
        )
        .bind(new_root.to_vec())
        .bind(seq_number as i32)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
}