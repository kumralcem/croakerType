use crate::config::Config;
use reqwest::multipart;
use reqwest::Client;
use std::path::Path;
use thiserror::Error;
use tokio::fs;
use tokio::time::{timeout, Duration};

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
    language: String,
}

#[derive(Debug, serde::Deserialize)]
struct WhisperResponse {
    text: String,
}

impl WhisperClient {
    pub fn new(config: Config, api_key: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120)) // Increased timeout, but wrapper timeout will catch it first
            .build()
            .expect("Failed to create HTTP client");
        
        let language = config.general.language.clone();
        
        Self {
            client,
            config,
            api_key,
            language,
        }
    }

    pub async fn transcribe(&self, audio_path: &Path) -> Result<String, WhisperError> {
        self.transcribe_with_language(audio_path, &self.language).await
    }

    pub async fn transcribe_with_language(&self, audio_path: &Path, language: &str) -> Result<String, WhisperError> {
        tracing::info!("Transcribing audio file: {:?} (language: {})", audio_path, language);

        // Wrap the API call in a timeout to prevent hanging
        let transcription_timeout = Duration::from_secs(90); // 90 seconds total timeout
        
        let result = timeout(transcription_timeout, async {
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
            if !language.is_empty() {
                form = form.text("language", language.to_string());
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
            
            Ok(whisper_response.text)
        }).await;

        match result {
            Ok(Ok(text)) => {
                tracing::info!("Transcription completed: {} chars", text.len());
                Ok(text)
            }
            Ok(Err(e)) => {
                tracing::error!("Transcription API error: {}", e);
                Err(e)
            }
            Err(_) => {
                tracing::error!("Transcription request timed out after {} seconds", transcription_timeout.as_secs());
                Err(WhisperError::ApiError(format!(
                    "Request timed out after {} seconds",
                    transcription_timeout.as_secs()
                )))
            }
        }
    }
}

