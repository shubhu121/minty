//! Trait definition for LLM backends.
//!
//! All LLM providers (Ollama, OpenAI, local GGUF, etc.) implement this
//! trait so the rest of the app remains backend-agnostic.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Errors ──────────────────────────────────────────────────

/// Errors from LLM operations.
#[derive(Debug, Error)]
pub enum LlmError {
    #[error("LLM backend not available: {0}")]
    Unavailable(String),

    #[error("request failed: {0}")]
    Request(String),

    #[error("stream error: {0}")]
    Stream(String),

    #[error("model not found: {0}")]
    ModelNotFound(String),

    #[error("parse error: {0}")]
    Parse(String),
}

impl From<reqwest::Error> for LlmError {
    fn from(e: reqwest::Error) -> Self {
        Self::Request(e.to_string())
    }
}

impl From<LlmError> for String {
    fn from(e: LlmError) -> Self {
        e.to_string()
    }
}

// ─── Types ───────────────────────────────────────────────────

/// A single message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn stream_generate(
        &self,
        messages: Vec<ChatMessage>,
        on_token: Box<dyn Fn(String) + Send>,
    ) -> Result<(), LlmError>;

    async fn stream_completion(
        &self,
        prompt: &str,
        on_token: Box<dyn Fn(String) + Send>,
    ) -> Result<(), LlmError>;

    async fn is_available(&self) -> bool;
    fn model_name(&self) -> &str;
    fn completion_model_name(&self) -> &str;
}
