use std::sync::Arc;

use anyhow::{Context, Result};
use redis::aio::ConnectionManager;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

use crate::{
    cache,
    config::AppConfig,
    embeddings::EmbeddingClient,
    langfuse::LangfuseClient,
    llm_client::LlmClient,
    store::{ChunkInput, RetrievedChunk, Store},
};

#[derive(Clone)]
pub struct RagService {
    config: AppConfig,
    store: Store,
    redis: Arc<Mutex<ConnectionManager>>,
    embedding_client: EmbeddingClient,
    llm_client: LlmClient,
    langfuse: LangfuseClient,
}

#[derive(Debug, Serialize)]
pub struct IngestResult {
    pub document_id: Uuid,
    pub chunk_count: usize,
}

#[derive(Debug, Serialize)]
pub struct QueryResult {
    pub answer: String,
    pub chunks: Vec<RetrievedChunk>,
    pub cache_hit: bool,
}

impl RagService {
    pub fn new(
        config: AppConfig,
        store: Store,
        redis: ConnectionManager,
        embedding_client: EmbeddingClient,
        llm_client: LlmClient,
        langfuse: LangfuseClient,
    ) -> Self {
        Self {
            config,
            store,
            redis: Arc::new(Mutex::new(redis)),
            embedding_client,
            llm_client,
            langfuse,
        }
    }

    pub async fn ingest(&self, source_id: String, text: String) -> Result<IngestResult> {
        let raw_chunks = chunk_text(&text, 600, 100);
        let mut embedded = Vec::with_capacity(raw_chunks.len());

        for chunk in raw_chunks {
            let embedding = self
                .embedding_client
                .embed_text(&chunk)
                .await
                .context("failed creating chunk embedding")?;
            embedded.push(ChunkInput {
                content: chunk,
                embedding,
            });
        }

        let document_id = self
            .store
            .replace_document_with_chunks(&source_id, &text, &embedded)
            .await
            .context("failed storing ingested document")?;

        self.langfuse
            .trace(
                "ingest",
                json!({ "source_id": source_id }),
                json!({ "document_id": document_id, "chunk_count": embedded.len() }),
                json!({ "kind": "ingest" }),
            )
            .await?;

        Ok(IngestResult {
            document_id,
            chunk_count: embedded.len(),
        })
    }

    pub async fn query(&self, question: String) -> Result<QueryResult> {
        let cache_key = make_cache_key(&question);
        if let Some(answer) = {
            let mut redis = self.redis.lock().await;
            cache::get(&mut redis, &cache_key).await?
        } {
            info!("cache hit for query");
            return Ok(QueryResult {
                answer,
                chunks: Vec::new(),
                cache_hit: true,
            });
        }

        let q_embedding = self
            .embedding_client
            .embed_text(&question)
            .await
            .context("failed creating query embedding")?;
        if q_embedding.len() != self.config.embedding_dim {
            anyhow::bail!(
                "embedding dimension mismatch: expected {}, got {}",
                self.config.embedding_dim,
                q_embedding.len()
            );
        }

        let chunks = self
            .store
            .search_chunks(q_embedding, self.config.top_k)
            .await
            .context("failed searching relevant chunks")?;
        let context = chunks
            .iter()
            .enumerate()
            .map(|(idx, c)| format!("Chunk {} (score {:.3}):\n{}", idx + 1, c.score, c.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let answer = self
            .llm_client
            .complete_with_context(&question, &context)
            .await
            .context("failed generating answer")?;

        {
            let mut redis = self.redis.lock().await;
            cache::set_ex(&mut redis, &cache_key, &answer, 180)
                .await
                .context("failed storing answer in cache")?;
        }

        self.langfuse
            .trace(
                "query",
                json!({ "question": question }),
                json!({ "answer": answer }),
                json!({ "retrieved_chunks": chunks.len(), "kind": "query" }),
            )
            .await?;

        Ok(QueryResult {
            answer,
            chunks,
            cache_hit: false,
        })
    }
}

fn make_cache_key(question: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(question.as_bytes());
    let hash = hasher.finalize();
    format!("rag:answer:{:x}", hash)
}

fn chunk_text(input: &str, max_chars: usize, overlap_chars: usize) -> Vec<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    if trimmed.chars().count() <= max_chars {
        return vec![trimmed.to_owned()];
    }

    let chars: Vec<char> = trimmed.chars().collect();
    let mut out = Vec::new();
    let mut start = 0usize;

    while start < chars.len() {
        let end = (start + max_chars).min(chars.len());
        let chunk: String = chars[start..end].iter().collect();
        out.push(chunk);
        if end == chars.len() {
            break;
        }
        start = end.saturating_sub(overlap_chars);
    }

    out
}
