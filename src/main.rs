mod api;
mod cache;
mod config;
mod embeddings;
mod langfuse;
mod llm_client;
mod rag;
mod store;

use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use axum::Router;
use redis::aio::ConnectionManager;
use sqlx::postgres::PgPoolOptions;
use tracing::info;

use crate::{
    config::AppConfig, embeddings::EmbeddingClient, langfuse::LangfuseClient,
    llm_client::LlmClient, rag::RagService, store::Store,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = AppConfig::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .context("failed to connect to postgres")?;

    let redis_client =
        redis::Client::open(config.redis_url.clone()).context("failed to parse REDIS_URL")?;
    let redis = ConnectionManager::new(redis_client)
        .await
        .context("failed to connect to redis")?;

    let store = Store::new(pool);
    store
        .init_schema(config.embedding_dim as i32)
        .await
        .context("failed to initialize database schema")?;

    let embedding_client = EmbeddingClient::new(
        config.llm_base_url.clone(),
        config.llm_api_key.clone(),
        config.embedding_model.clone(),
    );
    let llm_client = LlmClient::new(
        config.llm_base_url.clone(),
        config.llm_api_key.clone(),
        config.chat_model.clone(),
    );
    let langfuse = LangfuseClient::from_config(&config);

    let rag = Arc::new(RagService::new(
        config.clone(),
        store,
        redis,
        embedding_client,
        llm_client,
        langfuse,
    ));

    let app: Router = api::router(rag);
    let addr: SocketAddr = config.server_addr.parse().context("invalid SERVER_ADDR")?;

    info!("starting server on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context("failed to bind server listener")?;
    axum::serve(listener, app)
        .await
        .context("server exited with error")?;

    Ok(())
}

fn init_tracing() {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info,sqlx=warn".to_owned());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}
