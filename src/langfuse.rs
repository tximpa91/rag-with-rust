use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use serde_json::json;
use tracing::warn;
use uuid::Uuid;

use crate::config::AppConfig;

#[derive(Clone)]
pub struct LangfuseClient {
    http: reqwest::Client,
    base_url: Option<String>,
    auth_header: Option<String>,
}

impl LangfuseClient {
    pub fn from_config(config: &AppConfig) -> Self {
        let auth_header = match (
            config.langfuse_public_key.as_ref(),
            config.langfuse_secret_key.as_ref(),
        ) {
            (Some(public), Some(secret)) => {
                let encoded = STANDARD.encode(format!("{public}:{secret}"));
                Some(format!("Basic {encoded}"))
            }
            _ => None,
        };

        Self {
            http: reqwest::Client::new(),
            base_url: config.langfuse_base_url.clone(),
            auth_header,
        }
    }

    pub async fn trace(
        &self,
        name: &str,
        input: serde_json::Value,
        output: serde_json::Value,
        metadata: serde_json::Value,
    ) -> Result<()> {
        let Some(base_url) = self.base_url.as_ref() else {
            return Ok(());
        };
        let Some(auth) = self.auth_header.as_ref() else {
            return Ok(());
        };

        let trace_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let body = json!({
            "batch": [
                {
                    "id": Uuid::new_v4().to_string(),
                    "type": "trace-create",
                    "timestamp": now,
                    "body": {
                        "id": trace_id,
                        "name": name,
                        "input": input,
                        "output": output,
                        "metadata": metadata,
                    }
                }
            ]
        });

        let url = format!("{}/api/public/ingestion", base_url.trim_end_matches('/'));
        let res = self
            .http
            .post(url)
            .header("Authorization", auth)
            .json(&body)
            .send()
            .await;

        match res {
            Ok(response) if response.status().is_success() => Ok(()),
            Ok(response) => {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                warn!("langfuse trace failed with status {}: {}", status, text);
                Ok(())
            }
            Err(err) => {
                warn!("langfuse trace request error: {}", err);
                Ok(())
            }
        }
    }
}
