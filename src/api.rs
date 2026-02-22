use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::rag::RagService;

pub fn router(rag: Arc<RagService>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ingest", post(ingest))
        .route("/query", post(query))
        .with_state(rag)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

async fn ingest(
    State(rag): State<Arc<RagService>>,
    Json(payload): Json<IngestRequest>,
) -> Result<Json<IngestResponse>, ApiError> {
    let result = rag
        .ingest(payload.source_id, payload.text)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(IngestResponse {
        document_id: result.document_id.to_string(),
        chunk_count: result.chunk_count,
    }))
}

async fn query(
    State(rag): State<Arc<RagService>>,
    Json(payload): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, ApiError> {
    let result = rag.query(payload.question).await.map_err(ApiError::from)?;
    Ok(Json(QueryResponse {
        answer: result.answer,
        chunks: result.chunks,
        cache_hit: result.cache_hit,
    }))
}

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
}

#[derive(Debug, Deserialize)]
struct IngestRequest {
    source_id: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct IngestResponse {
    document_id: String,
    chunk_count: usize,
}

#[derive(Debug, Deserialize)]
struct QueryRequest {
    question: String,
}

#[derive(Debug, Serialize)]
struct QueryResponse {
    answer: String,
    chunks: Vec<crate::store::RetrievedChunk>,
    cache_hit: bool,
}

#[derive(Debug)]
struct ApiError(anyhow::Error);

impl From<anyhow::Error> for ApiError {
    fn from(value: anyhow::Error) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        error!("request failed: {:?}", self.0);
        let body = Json(serde_json::json!({
            "error": self.0.to_string()
        }));
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}
