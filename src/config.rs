use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub server_addr: String,
    pub database_url: String,
    pub redis_url: String,
    pub llm_base_url: String,
    pub llm_api_key: Option<String>,
    pub chat_model: String,
    pub embedding_model: String,
    pub embedding_dim: usize,
    pub top_k: i64,
    pub langfuse_base_url: Option<String>,
    pub langfuse_public_key: Option<String>,
    pub langfuse_secret_key: Option<String>,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            server_addr: var_or("SERVER_ADDR", "0.0.0.0:8080"),
            database_url: req_var("DATABASE_URL")?,
            redis_url: var_or("REDIS_URL", "redis://127.0.0.1:6379/"),
            llm_base_url: var_or("LLM_BASE_URL", "http://127.0.0.1:8000/v1"),
            llm_api_key: std::env::var("LLM_API_KEY").ok(),
            chat_model: var_or("CHAT_MODEL", "qwen3.5"),
            embedding_model: var_or("EMBEDDING_MODEL", "text-embedding-3-small"),
            embedding_dim: var_or("EMBEDDING_DIM", "1536")
                .parse()
                .context("EMBEDDING_DIM must be a positive integer")?,
            top_k: var_or("RAG_TOP_K", "4")
                .parse()
                .context("RAG_TOP_K must be an integer")?,
            langfuse_base_url: std::env::var("LANGFUSE_BASE_URL").ok(),
            langfuse_public_key: std::env::var("LANGFUSE_PUBLIC_KEY").ok(),
            langfuse_secret_key: std::env::var("LANGFUSE_SECRET_KEY").ok(),
        })
    }
}

fn var_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn req_var(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("{key} must be set"))
}
