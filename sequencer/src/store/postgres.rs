pub mod queries;
use sqlx::{PgPool};

pub struct PostgresDB {
    pool: PgPool,
}

impl PostgresDB {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}