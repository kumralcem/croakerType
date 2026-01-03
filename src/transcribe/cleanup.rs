use crate::config::Config;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::{timeout, Duration};

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

fn strip_output_tags(s: &str) -> String {
    // The default cleanup prompt asks the model to return ONLY:
    // <output>cleaned text</output>
    //
    // We still defensively support responses that include extra whitespace or text
    // around the tags by extracting the first <output>...</output> span.
    let trimmed = s.trim();
    let open = "<output>";
    let close = "</output>";

    if let Some(open_idx) = trimmed.find(open) {
        let after_open = open_idx + open.len();
        if let Some(close_rel) = trimmed[after_open..].find(close) {
            let close_idx = after_open + close_rel;
            return trimmed[after_open..close_idx].trim().to_string();
        }
    }

    trimmed.to_string()
}

impl CleanupClient {
    pub fn new(config: Config, api_key: String) -> Result<Self, CleanupError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120)) // Increased timeout, but wrapper timeout will catch it first
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

        // Wrap the API call in a timeout to prevent hanging
        let cleanup_timeout = Duration::from_secs(90); // 90 seconds total timeout
        
        let result = timeout(cleanup_timeout, async {
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

            Ok(strip_output_tags(&cleaned_text))
        }).await;

        match result {
            Ok(Ok(text)) => {
                tracing::info!("Cleanup completed: {} chars", text.len());
                Ok(text)
            }
            Ok(Err(e)) => {
                tracing::error!("Cleanup API error: {}", e);
                Err(e)
            }
            Err(_) => {
                tracing::error!("Cleanup request timed out after {} seconds", cleanup_timeout.as_secs());
                Err(CleanupError::ApiError(format!(
                    "Request timed out after {} seconds",
                    cleanup_timeout.as_secs()
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::strip_output_tags;

    #[test]
    fn strip_output_tags_extracts_inner_text() {
        assert_eq!(
            strip_output_tags(" <output>Hello world.</output>\n"),
            "Hello world."
        );
    }

    #[test]
    fn strip_output_tags_falls_back_when_missing() {
        assert_eq!(strip_output_tags("Hello world."), "Hello world.");
    }
}
