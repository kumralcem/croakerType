pub mod gtk_window;
pub mod layer_shell;
pub mod notification;

use crate::daemon::state::DaemonState;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OverlayError {
    #[error("GTK error: {0}")]
    GtkError(String),
    #[error("Failed to initialize overlay")]
    InitError,
}

pub trait Overlay {
    fn update_state(&self, state: DaemonState);
    fn update_audio_level(&self, level: f32);
    fn show(&self);
    fn hide(&self);
}

pub fn create_overlay(backend: &str) -> Result<Box<dyn Overlay>, OverlayError> {
    match backend {
        "layer-shell" => {
            #[cfg(feature = "layer-shell")]
            {
                layer_shell::LayerShellOverlay::new()
                    .map(|o| Box::new(o) as Box<dyn Overlay>)
            }
            #[cfg(not(feature = "layer-shell"))]
            {
                tracing::warn!("layer-shell feature not enabled, falling back to GTK");
                gtk_window::GtkOverlay::new().map(|o| Box::new(o) as Box<dyn Overlay>)
            }
        }
        "gtk" => {
            gtk_window::GtkOverlay::new().map(|o| Box::new(o) as Box<dyn Overlay>)
        }
        "notification" => {
            notification::NotificationOverlay::new().map(|o| Box::new(o) as Box<dyn Overlay>)
        }
        "auto" => {
            // Try layer-shell first, fallback to GTK
            #[cfg(feature = "layer-shell")]
            {
                if let Ok(overlay) = layer_shell::LayerShellOverlay::new() {
                    return Ok(Box::new(overlay) as Box<dyn Overlay>);
                }
            }
            gtk_window::GtkOverlay::new().map(|o| Box::new(o) as Box<dyn Overlay>)
        }
        _ => Err(OverlayError::InitError),
    }
}

