pub mod notification;
pub mod tray;

use crate::daemon::state::DaemonState;
use thiserror::Error;

#[derive(Debug, Clone)]
pub enum OverlayMessage {
    State(DaemonState),
    OutputMode(String),
    Language(String),
    AudioLevel(f32),
    Show,
    Hide,
}

#[derive(Debug, Error)]
pub enum OverlayError {
    #[error("Overlay error: {0}")]
    Error(String),
    #[error("Failed to initialize overlay")]
    InitError,
}

pub trait Overlay: Send {
    fn update_state(&self, state: DaemonState);
    fn update_audio_level(&self, level: f32);
    fn update_output_mode(&self, mode: &str);
    fn update_language(&self, language: &str);
    fn show(&self);
    fn hide(&self);
}

pub fn create_overlay(backend: &str) -> Result<Box<dyn Overlay>, OverlayError> {
    match backend {
        "notification" => {
            notification::NotificationOverlay::new().map(|o| Box::new(o) as Box<dyn Overlay>)
        }
        _ => Err(OverlayError::InitError),
    }
}

/// Run the system tray - this blocks and processes messages
pub fn run_tray(message_rx: std::sync::mpsc::Receiver<OverlayMessage>) -> anyhow::Result<()> {
    tray::run_tray(message_rx)
}
