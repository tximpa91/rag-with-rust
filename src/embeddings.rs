use anyhow::{bail, Context, Result};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct EmbeddingClient {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
}

impl EmbeddingClient {
    pub fn new(base_url: String, api_key: Option<String>, model: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            api_key,
            model,
        }
    }

    pub async fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/embeddings", self.base_url.trim_end_matches('/'));
        let payload = EmbeddingRequest {
            model: self.model.clone(),
            input: text.to_owned(),
        };

        let mut req = self
            .http
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .json(&payload);

        if let Some(key) = self.api_key.as_ref() {
            req = req.header(AUTHORIZATION, format!("Bearer {key}"));
        }

        let res = req.send().await.context("embedding request failed")?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            bail!("embedding API returned {status}: {body}");
        }

        let parsed: EmbeddingResponse = res
            .json()
            .await
            .context("failed to parse embeddings response JSON")?;
        let first = parsed
            .data
            .first()
            .context("embeddings response did not include any vectors")?;

        Ok(first.embedding.clone())
    }
}

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: String,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}
