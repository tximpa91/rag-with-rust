use anyhow::{bail, Context, Result};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct LlmClient {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
}

impl LlmClient {
    pub fn new(base_url: String, api_key: Option<String>, model: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            api_key,
            model,
        }
    }

    pub async fn complete_with_context(&self, question: &str, context: &str) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let prompt = format!(
            "You are a helpful assistant that answers strictly using the provided context.\n\nContext:\n{context}\n\nQuestion: {question}"
        );

        let req_body = ChatCompletionRequest {
            model: self.model.clone(),
            temperature: 0.2,
            messages: vec![Message {
                role: "user".to_owned(),
                content: prompt,
            }],
        };

        let mut req = self
            .http
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .json(&req_body);
        if let Some(key) = self.api_key.as_ref() {
            req = req.header(AUTHORIZATION, format!("Bearer {key}"));
        }

        let res = req.send().await.context("chat completion request failed")?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            bail!("chat completions API returned {status}: {body}");
        }

        let parsed: ChatCompletionResponse = res
            .json()
            .await
            .context("failed to parse chat completion response JSON")?;
        let first = parsed
            .choices
            .first()
            .context("chat completion response did not include choices")?;
        Ok(first.message.content.clone())
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    temperature: f32,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}
