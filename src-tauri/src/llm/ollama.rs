//! Ollama backend implementation.
//!
//! Communicates with a local Ollama server via HTTP.
//! Supports streaming generation through NDJSON parsing.
//!
//! # Default configuration
//! - Base URL: `http://localhost:11434`
//! - Model: `llama3.2:3b` (small, fast, good for RAG)

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::Duration;

use super::backend::{ChatMessage, LlmBackend, LlmError};

const DEFAULT_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "llama3.2:3b";

pub struct OllamaBackend {
    base_url: String,
    model: RwLock<String>,
    completion_model: RwLock<String>,
    client: Client,
}

impl OllamaBackend {
    /// Create a new Ollama backend with default settings.
    pub fn new() -> Self {
        Self::with_config(DEFAULT_BASE_URL, DEFAULT_MODEL, "qwen2.5:1.5b")
    }

    /// Create with a custom base URL and model.
    pub fn with_config(
        base_url: impl Into<String>,
        model: impl Into<String>,
        completion: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            model: RwLock::new(model.into()),
            completion_model: RwLock::new(completion.into()),
            client: Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .expect("failed to build reqwest client"),
        }
    }

    /// Change the active model at runtime.
    pub fn set_model(&self, model: impl Into<String>) {
        if let Ok(mut m) = self.model.write() {
            *m = model.into();
        }
    }

    /// Change the active completion model at runtime.
    pub fn set_completion_model(&self, model: impl Into<String>) {
        if let Ok(mut m) = self.completion_model.write() {
            *m = model.into();
        }
    }

    /// List all models available on the Ollama server.
    pub async fn list_models(&self) -> Result<Vec<OllamaModelInfo>, LlmError> {
        let url = format!("{}/api/tags", self.base_url);
        let resp = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(LlmError::Unavailable(format!(
                "Ollama returned status {}",
                resp.status()
            )));
        }

        let body: TagsResponse = resp
            .json()
            .await
            .map_err(|e| LlmError::Parse(e.to_string()))?;

        Ok(body.models)
    }
}

#[async_trait]
impl LlmBackend for OllamaBackend {
    async fn stream_generate(
        &self,
        messages: Vec<ChatMessage>,
        on_token: Box<dyn Fn(String) + Send>,
    ) -> Result<(), LlmError> {
        let model = self
            .model
            .read()
            .map_err(|e| LlmError::Request(format!("lock error: {e}")))?
            .clone();

        let url = format!("{}/api/chat", self.base_url);

        let request_body = OllamaChatRequest {
            model,
            messages,
            stream: true,
        };

        let resp = self.client.post(&url).json(&request_body).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Request(format!(
                "Ollama returned {status}: {body}"
            )));
        }

        // Stream NDJSON response
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| LlmError::Stream(e.to_string()))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete lines (NDJSON = one JSON object per line)
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                match serde_json::from_str::<OllamaChatChunk>(&line) {
                    Ok(chunk) => {
                        if let Some(msg) = chunk.message {
                            if !msg.content.is_empty() {
                                on_token(msg.content);
                            }
                        }
                        if chunk.done {
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        eprintln!("[Ollama] Failed to parse chunk: {e} — line: {line}");
                    }
                }
            }
        }

        Ok(())
    }

    async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        matches!(
            self.client
                .get(&url)
                .timeout(Duration::from_secs(3))
                .send()
                .await,
            Ok(resp) if resp.status().is_success()
        )
    }

    fn model_name(&self) -> &str {
        // Return the default if lock fails (shouldn't happen in practice)
        DEFAULT_MODEL
    }

    fn completion_model_name(&self) -> &str {
        "qwen2.5:1.5b"
    }

    async fn stream_completion(
        &self,
        prompt: &str,
        on_token: Box<dyn Fn(String) + Send>,
    ) -> Result<(), LlmError> {
        let model = self
            .completion_model
            .read()
            .map_err(|e| LlmError::Request(format!("lock error: {e}")))?
            .clone();

        let url = format!("{}/api/generate", self.base_url);

        let request_body = OllamaGenerateRequest {
            model,
            prompt: prompt.to_string(),
            stream: true,
            raw: true,
        };

        let resp = self.client.post(&url).json(&request_body).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Request(format!(
                "Ollama returned {status}: {body}"
            )));
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| LlmError::Stream(e.to_string()))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                match serde_json::from_str::<OllamaGenerateChunk>(&line) {
                    Ok(chunk) => {
                        let resp_text = chunk.response;
                        if !resp_text.is_empty() {
                            on_token(resp_text);
                        }
                        if chunk.done {
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        eprintln!("[Ollama] Failed to parse generate chunk: {e} — line: {line}");
                    }
                }
            }
        }

        Ok(())
    }
}

// ─── Ollama API Types ────────────────────────────────────────

#[derive(Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    raw: bool,
}

#[derive(Deserialize)]
struct OllamaGenerateChunk {
    #[serde(default)]
    response: String,
    #[serde(default)]
    done: bool,
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

/// A single chunk from the Ollama streaming response.
#[derive(Deserialize)]
struct OllamaChatChunk {
    message: Option<OllamaChatMessage>,
    #[serde(default)]
    done: bool,
}

#[derive(Deserialize)]
struct OllamaChatMessage {
    #[serde(default)]
    content: String,
}

/// Response from GET /api/tags.
#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<OllamaModelInfo>,
}

/// Info about a single model from Ollama's model list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModelInfo {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub digest: String,
}
