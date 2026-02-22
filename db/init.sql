-- Bootstrap DBs used in docker-compose.
-- The Rust app still manages the RAG schema at startup.

CREATE DATABASE langfuse;
\connect rag
CREATE EXTENSION IF NOT EXISTS vector;
