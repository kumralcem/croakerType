use crate::config::Config;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CleanupError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("API returned error: {0}")]
    ApiError(String),
    #[error("Invalid response format")]
    InvalidResponse,
    #[error("Failed to load cleanup prompt: {0}")]
    PromptError(#[from] crate::config::ConfigError),
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: String,
}

#[derive(Clone)]
pub struct CleanupClient {
    client: Client,
    config: Config,
    api_key: String,
    prompt: String,
}

impl CleanupClient {
    pub fn new(config: Config, api_key: String) -> Result<Self, CleanupError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");
        
        let prompt = config.load_cleanup_prompt()?;

        Ok(Self {
            client,
            config,
            api_key,
            prompt,
        })
    }

    pub async fn cleanup(&self, text: &str) -> Result<String, CleanupError> {
        if !self.config.groq.cleanup_enabled {
            tracing::debug!("Cleanup disabled, returning original text");
            return Ok(text.to_string());
        }

        tracing::info!("Cleaning up transcription: {} chars", text.len());

        let request = ChatRequest {
            model: self.config.groq.cleanup_model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: self.prompt.clone(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: text.to_string(),
                },
            ],
            temperature: Some(self.config.groq.cleanup_temperature),
        };

        let response = self
            .client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        // Check status
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CleanupError::ApiError(format!(
                "HTTP {}: {}",
                status,
                error_text
            )));
        }

        // Parse response
        let chat_response: ChatResponse = response.json().await?;
        
        let cleaned_text = chat_response
            .choices
            .first()
            .and_then(|c| Some(c.message.content.clone()))
            .ok_or(CleanupError::InvalidResponse)?;

        tracing::info!("Cleanup completed: {} chars", cleaned_text.len());
        Ok(cleaned_text.trim().to_string())
    }
}

