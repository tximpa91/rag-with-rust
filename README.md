# Rust RAG MVP

Simple Rust-first Retrieval-Augmented Generation MVP with:

- Rust API (`axum`)
- OpenAI-compatible model endpoint (`ai/qwen3.5`)
- Postgres + pgvector
- Redis cache
- Langfuse tracing

## Architecture

- `POST /ingest`: receives `source_id` + `text`, chunks text, embeds each chunk, stores vectors in pgvector.
- `POST /query`: embeds question, retrieves top-k chunks by cosine similarity, calls chat completion with grounded context.
- Redis caches recent answers for repeated questions.
- Langfuse receives trace events for ingest and query operations.

## Project Layout

- `src/main.rs`: app wiring (config, DB, Redis, clients, server)
- `src/api.rs`: HTTP routes (`/health`, `/ingest`, `/query`)
- `src/rag.rs`: ingest/query orchestration
- `src/store.rs`: pgvector schema + retrieval queries
- `src/embeddings.rs`: OpenAI-compatible embeddings client
- `src/llm_client.rs`: OpenAI-compatible chat client
- `src/langfuse.rs`: Langfuse ingestion API client
- `docker-compose.yml`: local stack
- `db/init.sql`: DB bootstrap for pgvector + langfuse DB

## Prerequisites

- Docker + Docker Compose
- Rust 1.85+ (for local `cargo` workflow)

## Local Pre-Commit Hook (fmt + build)

If you want checks before every commit, this repo includes `.githooks/pre-commit`.

Setup:

```bash
chmod +x .githooks/pre-commit
cp .githooks/pre-commit .git/hooks/pre-commit
```

Now every `git commit` runs:

- `cargo fmt --all -- --check`
- `cargo build --all-targets`

If either command fails, the commit is blocked.

## Quick Start

1) Create local env file:

```bash
cp .env.example .env
```

2) Start the stack:

```bash
docker compose up --build
```

3) Health check:

```bash
curl -s http://localhost:8080/health
```

Expected:

```json
{"ok":true}
```

## Ingest Example

```bash
curl -s -X POST http://localhost:8080/ingest \
  -H "Content-Type: application/json" \
  -d '{
    "source_id":"intro-doc",
    "text":"Rust is a systems programming language focused on safety and performance. Pgvector enables vector similarity search in PostgreSQL."
  }'
```

Expected response shape:

```json
{"document_id":"...","chunk_count":1}
```

## Query Example

```bash
curl -s -X POST http://localhost:8080/query \
  -H "Content-Type: application/json" \
  -d '{
    "question":"What does pgvector do?"
  }'
```

Expected response shape:

```json
{
  "answer":"...",
  "chunks":[{"id":"...","content":"...","score":0.9}],
  "cache_hit":false
}
```

Run the same query again and `cache_hit` should become `true`.

## Langfuse Setup Notes

The compose file starts local Langfuse services at `http://localhost:3000`.

To send traces from the Rust app:

1) Open Langfuse UI and create a project.
2) Copy generated public/secret keys.
3) Set in `.env`:

```bash
LANGFUSE_BASE_URL=http://langfuse-web:3000
LANGFUSE_PUBLIC_KEY=pk-lf-...
LANGFUSE_SECRET_KEY=sk-lf-...
```

4) Restart the app service:

```bash
docker compose restart app
```

## Notes

- The app enforces embedding dimension with `EMBEDDING_DIM` (default `1536`).
- Ensure your Qwen container exposes OpenAI-compatible `/v1/embeddings` and `/v1/chat/completions`.
- If your model uses a different embedding size, update `EMBEDDING_DIM` before first startup.