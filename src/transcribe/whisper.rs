use crate::config::Config;
use reqwest::multipart;
use reqwest::Client;
use std::path::Path;
use thiserror::Error;
use tokio::fs;

#[derive(Debug, Error)]
pub enum WhisperError {
    #[error("Failed to read audio file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("API returned error: {0}")]
    ApiError(String),
    #[error("Invalid response format")]
    InvalidResponse,
}

#[derive(Clone)]
pub struct WhisperClient {
    client: Client,
    config: Config,
    api_key: String,
}

#[derive(Debug, serde::Deserialize)]
struct WhisperResponse {
    text: String,
}

impl WhisperClient {
    pub fn new(config: Config, api_key: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            client,
            config,
            api_key,
        }
    }

    pub async fn transcribe(&self, audio_path: &Path) -> Result<String, WhisperError> {
        tracing::info!("Transcribing audio file: {:?}", audio_path);

        // Read audio file
        let audio_data = fs::read(audio_path).await?;

        // Create multipart form
        let file_part = multipart::Part::bytes(audio_data)
            .file_name("audio.wav")
            .mime_str("audio/wav")?;

        let mut form = multipart::Form::new()
            .text("model", self.config.groq.whisper_model.clone())
            .part("file", file_part);

        // Add language if specified
        if !self.config.general.language.is_empty() {
            form = form.text("language", self.config.general.language.clone());
        }

        // Make request
        let response = self
            .client
            .post("https://api.groq.com/openai/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await?;

        // Check status
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(WhisperError::ApiError(format!(
                "HTTP {}: {}",
                status,
                error_text
            )));
        }

        // Parse response
        let whisper_response: WhisperResponse = response.json().await?;
        
        tracing::info!("Transcription completed: {} chars", whisper_response.text.len());
        Ok(whisper_response.text)
    }
}

