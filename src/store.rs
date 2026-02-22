use anyhow::{Context, Result};
use pgvector::Vector;
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct ChunkInput {
    pub content: String,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Serialize, Clone)]
pub struct RetrievedChunk {
    pub id: Uuid,
    pub content: String,
    pub score: f64,
}

impl Store {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn init_schema(&self, embedding_dim: i32) -> Result<()> {
        let ext = "CREATE EXTENSION IF NOT EXISTS vector;";
        sqlx::query(ext)
            .execute(&self.pool)
            .await
            .context("failed creating vector extension")?;

        let documents = r#"
            CREATE TABLE IF NOT EXISTS documents (
                id UUID PRIMARY KEY,
                source_id TEXT NOT NULL UNIQUE,
                content TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
        "#;
        sqlx::query(documents)
            .execute(&self.pool)
            .await
            .context("failed creating documents table")?;

        let chunks = format!(
            "
            CREATE TABLE IF NOT EXISTS chunks (
                id UUID PRIMARY KEY,
                document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                content TEXT NOT NULL,
                embedding VECTOR({embedding_dim}) NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
        "
        );
        sqlx::query(&chunks)
            .execute(&self.pool)
            .await
            .context("failed creating chunks table")?;

        let index = "CREATE INDEX IF NOT EXISTS chunks_embedding_idx ON chunks USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);";
        sqlx::query(index)
            .execute(&self.pool)
            .await
            .context("failed creating chunks vector index")?;

        Ok(())
    }

    pub async fn replace_document_with_chunks(
        &self,
        source_id: &str,
        full_text: &str,
        chunks: &[ChunkInput],
    ) -> Result<Uuid> {
        let mut tx = self.pool.begin().await?;
        let document_id = Uuid::new_v4();

        sqlx::query("DELETE FROM documents WHERE source_id = $1")
            .bind(source_id)
            .execute(&mut *tx)
            .await
            .context("failed deleting existing document by source_id")?;

        sqlx::query("INSERT INTO documents (id, source_id, content) VALUES ($1, $2, $3)")
            .bind(document_id)
            .bind(source_id)
            .bind(full_text)
            .execute(&mut *tx)
            .await
            .context("failed inserting document")?;

        for chunk in chunks {
            sqlx::query(
                "INSERT INTO chunks (id, document_id, content, embedding) VALUES ($1, $2, $3, $4)",
            )
            .bind(Uuid::new_v4())
            .bind(document_id)
            .bind(&chunk.content)
            .bind(Vector::from(chunk.embedding.clone()))
            .execute(&mut *tx)
            .await
            .context("failed inserting chunk")?;
        }

        tx.commit().await?;
        Ok(document_id)
    }

    pub async fn search_chunks(
        &self,
        embedding: Vec<f32>,
        top_k: i64,
    ) -> Result<Vec<RetrievedChunk>> {
        let rows = sqlx::query(
            "
            SELECT id, content, 1 - (embedding <=> $1) AS score
            FROM chunks
            ORDER BY embedding <=> $1
            LIMIT $2
            ",
        )
        .bind(Vector::from(embedding))
        .bind(top_k)
        .fetch_all(&self.pool)
        .await
        .context("failed querying similar chunks")?;

        rows.into_iter()
            .map(|row| {
                Ok(RetrievedChunk {
                    id: row.try_get("id")?,
                    content: row.try_get("content")?,
                    score: row.try_get("score")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .context("failed parsing retrieved chunks")
    }
}
