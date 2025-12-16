use crate::audio::AudioRecorder;
use crate::config::Config;
use crate::output::clipboard::ClipboardOutput;
use crate::output::uinput::UinputKeyboard;
use crate::transcribe::{CleanupClient, WhisperClient};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonState {
    Idle,
    Recording,
    Processing,
    Outputting,
}

#[derive(Debug)]
pub enum StateEvent {
    StartRecording,
    StopRecording,
    Cancel,
    ProcessingComplete(String),
    OutputComplete,
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("Audio error: {0}")]
    AudioError(#[from] crate::audio::AudioError),
    #[error("Transcription error: {0}")]
    TranscriptionError(#[from] crate::transcribe::whisper::WhisperError),
    #[error("Cleanup error: {0}")]
    CleanupError(#[from] crate::transcribe::cleanup::CleanupError),
    #[error("Output error: {0}")]
    OutputError(#[from] crate::output::uinput::UinputError),
    #[error("Clipboard error: {0}")]
    ClipboardError(#[from] crate::output::clipboard::ClipboardError),
    #[error("Invalid state transition")]
    InvalidTransition,
}

pub struct StateMachine {
    state: DaemonState,
    config: Config,
    audio_recorder: AudioRecorder,
    whisper_client: WhisperClient,
    cleanup_client: CleanupClient,
    keyboard: Arc<UinputKeyboard>,
    clipboard: ClipboardOutput,
    overlay_tx: Option<std::sync::mpsc::Sender<DaemonState>>,
    event_tx: mpsc::Sender<StateEvent>,
    event_rx: mpsc::Receiver<StateEvent>,
    state_tx: Option<mpsc::Sender<DaemonState>>,
}

impl StateMachine {
    pub fn new(config: Config) -> Result<Self, StateError> {
        let api_key = config.load_api_key()
            .map_err(|e| StateError::TranscriptionError(crate::transcribe::whisper::WhisperError::ApiError(e.to_string())))?;

        let whisper_client = WhisperClient::new(config.clone(), api_key.clone());
        let cleanup_client = CleanupClient::new(config.clone(), api_key)
            .map_err(|e| StateError::CleanupError(e))?;
        
        let keyboard = Arc::new(UinputKeyboard::new(&config)?);
        let clipboard = ClipboardOutput::new(keyboard.clone(), config.output.clipboard_restore);

        let (event_tx, event_rx) = mpsc::channel(32);

        let config_clone = config.clone();
        Ok(Self {
            state: DaemonState::Idle,
            config,
            audio_recorder: AudioRecorder::new(config_clone),
            whisper_client,
            cleanup_client,
            keyboard,
            clipboard,
            overlay_tx: None,
            event_tx,
            event_rx,
            state_tx: None,
        })
    }

    pub fn set_state_sender(&mut self, state_tx: mpsc::Sender<DaemonState>) {
        self.state_tx = Some(state_tx);
    }

    pub fn set_overlay_sender(&mut self, overlay_tx: std::sync::mpsc::Sender<DaemonState>) {
        self.overlay_tx = Some(overlay_tx);
    }

    pub fn state(&self) -> DaemonState {
        self.state
    }

    pub fn event_sender(&self) -> mpsc::Sender<StateEvent> {
        self.event_tx.clone()
    }

    fn update_state(&mut self, new_state: DaemonState) {
        self.state = new_state;
        if let Some(ref state_tx) = self.state_tx {
            let _ = state_tx.try_send(self.state);
        }
        
        // Update overlay via channel
        if let Some(ref overlay_tx) = self.overlay_tx {
            let _ = overlay_tx.send(self.state);
        }
    }

    pub async fn handle_event(&mut self, event: StateEvent) -> Result<(), StateError> {
        match (self.state, &event) {
            (DaemonState::Idle, StateEvent::StartRecording) => {
                self.start_recording().await?;
            }
            (DaemonState::Recording, StateEvent::StopRecording) => {
                self.stop_recording().await?;
            }
            (DaemonState::Recording, StateEvent::Cancel) |
            (DaemonState::Processing, StateEvent::Cancel) |
            (DaemonState::Outputting, StateEvent::Cancel) => {
                self.cancel().await?;
            }
            (DaemonState::Processing, StateEvent::ProcessingComplete(text)) => {
                self.output_text(text).await?;
            }
            (DaemonState::Outputting, StateEvent::OutputComplete) => {
                self.update_state(DaemonState::Idle);
            }
            _ => {
                tracing::warn!("Invalid state transition: {:?} -> {:?}", self.state, event);
                return Err(StateError::InvalidTransition);
            }
        }

        Ok(())
    }

    async fn start_recording(&mut self) -> Result<(), StateError> {
        tracing::info!("Starting recording");
        self.audio_recorder.start().await?;
        self.update_state(DaemonState::Recording);
        Ok(())
    }

    async fn stop_recording(&mut self) -> Result<(), StateError> {
        tracing::info!("Stopping recording");
        let wav_path = self.audio_recorder.stop().await?;
        self.update_state(DaemonState::Processing);

        // Spawn transcription task
        let whisper_client = Arc::new(self.whisper_client.clone());
        let cleanup_client = Arc::new(self.cleanup_client.clone());
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let result = Self::process_audio(
                &*whisper_client,
                &*cleanup_client,
                wav_path
            ).await;
            
            match result {
                Ok(text) => {
                    let _ = event_tx.send(StateEvent::ProcessingComplete(text)).await;
                }
                Err(e) => {
                    tracing::error!("Processing failed: {}", e);
                    let _ = event_tx.send(StateEvent::Cancel).await;
                }
            }
        });

        Ok(())
    }

    async fn process_audio(
        whisper_client: &WhisperClient,
        cleanup_client: &CleanupClient,
        wav_path: PathBuf,
    ) -> Result<String, StateError> {
        // Transcribe
        let raw_text = whisper_client.transcribe(&wav_path).await?;

        // Cleanup
        let cleaned_text = cleanup_client.cleanup(&raw_text).await?;

        // Cleanup temp file
        if let Err(e) = tokio::fs::remove_file(&wav_path).await {
            tracing::warn!("Failed to remove audio file: {}", e);
        }

        Ok(cleaned_text)
    }

    async fn output_text(&mut self, text: &str) -> Result<(), StateError> {
        tracing::info!("Outputting text: {} chars", text.len());
        self.update_state(DaemonState::Outputting);

        // On Wayland, uinput often doesn't work reliably, so use clipboard by default
        let is_wayland = std::env::var("XDG_SESSION_TYPE")
            .map(|s| s == "wayland")
            .unwrap_or(false);
        
        // Check if text contains non-ASCII
        let has_non_ascii = text.chars().any(|c| !c.is_ascii());

        if is_wayland || has_non_ascii {
            // Use clipboard method (works reliably on Wayland)
            tracing::debug!("Using clipboard method (Wayland={}, non-ASCII={})", is_wayland, has_non_ascii);
            self.clipboard.copy_and_paste(text).await?;
        } else {
            // Use direct uinput typing (works on X11)
            tracing::debug!("Using uinput method");
            match self.keyboard.type_text(text).await {
                Ok(()) => {}
                Err(crate::output::uinput::UinputError::UnsupportedChar(_)) => {
                    // Fallback to clipboard
                    tracing::debug!("Falling back to clipboard (unsupported char)");
                    self.clipboard.copy_and_paste(text).await?;
                }
                Err(e) => {
                    tracing::warn!("uinput failed, falling back to clipboard: {}", e);
                    self.clipboard.copy_and_paste(text).await?;
                }
            }
        }

        // Clipboard restoration disabled - user requested removal
        // No need to restore clipboard anymore

        // Signal completion
        let _ = self.event_tx.send(StateEvent::OutputComplete).await;
        Ok(())
    }

    async fn cancel(&mut self) -> Result<(), StateError> {
        tracing::info!("Cancelling current operation");
        
        // Cleanup audio
        self.audio_recorder.cleanup(None).await;
        
        // Clipboard restoration disabled - user requested removal
        
        self.update_state(DaemonState::Idle);
        Ok(())
    }

    pub async fn run(mut self) -> Result<(), StateError> {
        while let Some(event) = self.event_rx.recv().await {
            if let Err(e) = self.handle_event(event).await {
                tracing::error!("State machine error: {}", e);
            }
        }

        Ok(())
    }
}


