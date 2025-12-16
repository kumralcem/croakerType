use crate::config::Config;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tempfile::NamedTempFile;
use thiserror::Error;
use tokio::fs;
use tokio::time::Duration;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Failed to spawn pw-record: {0}")]
    SpawnError(String),
    #[error("Recording process terminated unexpectedly")]
    ProcessTerminated,
    #[error("Failed to read audio file: {0}")]
    ReadError(String),
    #[error("Failed to create temp file: {0}")]
    TempFileError(String),
}

impl From<std::io::Error> for AudioError {
    fn from(e: std::io::Error) -> Self {
        AudioError::SpawnError(e.to_string())
    }
}

pub struct AudioRecorder {
    config: Config,
    process: Option<Child>,
    temp_file: Option<NamedTempFile>,
}

impl AudioRecorder {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            process: None,
            temp_file: None,
        }
    }

    pub async fn start(&mut self) -> Result<(), AudioError> {
        if self.process.is_some() {
            tracing::warn!("Recording already in progress");
            return Ok(());
        }

        // Create temporary WAV file
        let temp_file = NamedTempFile::new().map_err(|e| AudioError::TempFileError(e.to_string()))?;
        let wav_path = temp_file.path().to_path_buf();
        self.temp_file = Some(temp_file);

        // Build pw-record command
        // Note: --target=auto (default) will auto-select the default recording source
        // Remove --target=0 as that means "don't link" and won't record anything!
        let mut cmd = Command::new("pw-record");
        cmd.arg("--format=s16")
            .arg(&format!("--rate={}", self.config.audio.sample_rate))
            .arg("--channels=1")
            .arg(wav_path.to_string_lossy().as_ref())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped()); // Capture stderr for debugging

        tracing::debug!("Starting pw-record: {:?}", cmd);

        let child = cmd.spawn().map_err(|e| AudioError::SpawnError(e.to_string()))?;
        self.process = Some(child);

        tracing::info!("Audio recording started");
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<PathBuf, AudioError> {
        let mut process = self.process.take().ok_or_else(|| {
            AudioError::ProcessTerminated
        })?;

        // Send SIGINT to gracefully stop recording and flush the file
        // This is better than SIGKILL which doesn't give pw-record time to write
        if let Err(e) = process.kill() {
            tracing::warn!("Failed to send signal to pw-record: {}", e);
        }
        
        // Wait for process to finish and flush the file
        // Use blocking wait in a spawn_blocking to avoid blocking the async runtime
        let wait_result = tokio::task::spawn_blocking(move || process.wait()).await;
        if let Ok(Ok(status)) = wait_result {
            tracing::debug!("pw-record exited with status: {:?}", status);
        }

        // Give pw-record time to flush the file to disk
        tokio::time::sleep(Duration::from_millis(500)).await;

        let temp_file = self.temp_file.take().ok_or_else(|| {
            AudioError::ProcessTerminated
        })?;

        let wav_path = temp_file.path().to_path_buf();

        // Verify file exists and has content
        let metadata = fs::metadata(&wav_path).await.map_err(|e| AudioError::ReadError(e.to_string()))?;
        if metadata.len() == 0 {
            return Err(AudioError::ReadError("Audio file is empty".to_string()));
        }

        tracing::info!("Audio recording stopped, file size: {} bytes", metadata.len());

        // Persist the temp file so it can be read later
        // This prevents the file from being deleted when temp_file is dropped
        temp_file.keep().map_err(|e| AudioError::ReadError(format!("Failed to persist temp file: {}", e)))?;

        Ok(wav_path)
    }

    pub fn is_recording(&self) -> bool {
        self.process.is_some()
    }

    pub async fn cleanup(&mut self, wav_path: Option<&PathBuf>) {
        // Kill any running process
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }

        // Clean up temp file
        if let Some(temp_file) = self.temp_file.take() {
            let _ = temp_file.close();
        }

        // Remove WAV file if provided
        if let Some(path) = wav_path {
            if let Err(e) = fs::remove_file(path).await {
                tracing::warn!("Failed to remove audio file {:?}: {}", path, e);
            }
        }
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        // Cleanup on drop
        if self.process.is_some() {
            tracing::warn!("AudioRecorder dropped while recording");
        }
    }
}

