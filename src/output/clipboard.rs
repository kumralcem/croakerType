use crate::output::uinput::UinputKeyboard;
use std::sync::Arc;
use thiserror::Error;
use tokio::process::Command as TokioCommand;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("Failed to execute wl-copy: {0}")]
    CopyError(String),
    #[error("Failed to execute wl-paste: {0}")]
    PasteError(String),
    #[error("Uinput error: {0}")]
    UinputError(#[from] crate::output::uinput::UinputError),
}

pub struct ClipboardOutput {
    keyboard: Arc<UinputKeyboard>,
    restore_enabled: bool,
    saved_content: Option<String>,
}

impl ClipboardOutput {
    pub fn new(keyboard: Arc<UinputKeyboard>, restore_enabled: bool) -> Self {
        Self {
            keyboard,
            restore_enabled,
            saved_content: None,
        }
    }

    pub async fn save_current(&mut self) -> Result<(), ClipboardError> {
        if !self.restore_enabled {
            return Ok(());
        }

        let output = TokioCommand::new("wl-paste")
            .output()
            .await
            .map_err(|e| ClipboardError::PasteError(e.to_string()))?;

        if output.status.success() {
            self.saved_content = Some(
                String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .to_string(),
            );
            tracing::debug!("Saved clipboard content: {} chars", self.saved_content.as_ref().unwrap().len());
        }

        Ok(())
    }

    pub async fn copy_and_paste(&mut self, text: &str) -> Result<(), ClipboardError> {
        tracing::info!("Copying {} chars to clipboard and pasting", text.len());
        
        // Save current clipboard
        self.save_current().await?;

        // Copy text to clipboard
        let mut child = TokioCommand::new("wl-copy")
            .arg(text)
            .spawn()
            .map_err(|e| ClipboardError::CopyError(e.to_string()))?;

        let status = child.wait().await.map_err(|e| ClipboardError::CopyError(e.to_string()))?;
        
        if !status.success() {
            return Err(ClipboardError::CopyError("wl-copy failed".to_string()));
        }

        tracing::debug!("Text copied to clipboard, waiting before paste");
        
        // Wait a bit for clipboard to be ready
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // On Wayland, use wtype instead of uinput (uinput doesn't work reliably on Wayland)
        let is_wayland = std::env::var("XDG_SESSION_TYPE")
            .map(|s| s == "wayland")
            .unwrap_or(false);
        
        if is_wayland {
            // Try wtype first (Wayland-native)
            tracing::debug!("Sending Ctrl+V via wtype (Wayland)");
            let wtype_result = TokioCommand::new("wtype")
                .args(&["-M", "ctrl", "-k", "v"])
                .output()
                .await;
            
            match wtype_result {
                Ok(output) if output.status.success() => {
                    tracing::info!("Paste command sent via wtype");
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::warn!("wtype failed: {}", stderr);
                    if stderr.contains("virtual keyboard protocol") {
                        tracing::warn!("GNOME doesn't support virtual keyboard protocol. Trying uinput Ctrl+V as fallback...");
                        // Try uinput anyway - sometimes Ctrl+V works even if typing doesn't
                        if let Err(e) = self.keyboard.send_paste().await {
                            tracing::warn!("uinput Ctrl+V also failed: {}. Text is in clipboard - paste manually with Ctrl+V", e);
                            // Send a notification to remind user to paste
                            let _ = TokioCommand::new("notify-send")
                                .args(&["--app-name=croaker", "--urgency=normal", "croaker", "Text ready! Press Ctrl+V to paste."])
                                .output()
                                .await;
                        } else {
                            tracing::info!("uinput Ctrl+V sent successfully");
                        }
                    } else {
                        tracing::warn!("wtype failed, trying uinput fallback");
                        self.keyboard.send_paste().await?;
                    }
                }
                Err(_) => {
                    tracing::warn!("wtype not found, trying uinput fallback");
                    self.keyboard.send_paste().await?;
                }
            }
        } else {
            // Use uinput on X11
            tracing::debug!("Sending Ctrl+V via uinput (X11)");
            self.keyboard.send_paste().await?;
        }
        
        // Give the paste time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        tracing::info!("Paste command sent");

        Ok(())
    }

    pub async fn copy_to_clipboard(&mut self, text: &str) -> Result<(), ClipboardError> {
        tracing::info!("Copying {} chars to clipboard", text.len());
        
        // Save current clipboard if restore is enabled
        if self.restore_enabled {
            self.save_current().await?;
        }

        // Copy text to clipboard
        let mut child = TokioCommand::new("wl-copy")
            .arg(text)
            .spawn()
            .map_err(|e| ClipboardError::CopyError(e.to_string()))?;

        let status = child.wait().await.map_err(|e| ClipboardError::CopyError(e.to_string()))?;
        
        if !status.success() {
            return Err(ClipboardError::CopyError("wl-copy failed".to_string()));
        }

        tracing::debug!("Text copied to clipboard");
        Ok(())
    }

    pub async fn paste(&mut self) -> Result<(), ClipboardError> {
        tracing::info!("Pasting from clipboard");
        
        // Wait a bit for clipboard to be ready
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // On Wayland, use wtype instead of uinput (uinput doesn't work reliably on Wayland)
        let is_wayland = std::env::var("XDG_SESSION_TYPE")
            .map(|s| s == "wayland")
            .unwrap_or(false);
        
        if is_wayland {
            // Try wtype first (Wayland-native)
            tracing::debug!("Sending Ctrl+V via wtype (Wayland)");
            let wtype_result = TokioCommand::new("wtype")
                .args(&["-M", "ctrl", "-k", "v"])
                .output()
                .await;
            
            match wtype_result {
                Ok(output) if output.status.success() => {
                    tracing::info!("Paste command sent via wtype");
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::warn!("wtype failed: {}", stderr);
                    if stderr.contains("virtual keyboard protocol") {
                        tracing::warn!("GNOME doesn't support virtual keyboard protocol. Trying uinput Ctrl+V as fallback...");
                        if let Err(e) = self.keyboard.send_paste().await {
                            tracing::warn!("uinput Ctrl+V also failed: {}. Text is in clipboard - paste manually with Ctrl+V", e);
                            // Send a notification to remind user to paste
                            let _ = TokioCommand::new("notify-send")
                                .args(&["--app-name=croaker", "--urgency=normal", "croaker", "Text ready! Press Ctrl+V to paste."])
                                .output()
                                .await;
                        } else {
                            tracing::info!("uinput Ctrl+V sent successfully");
                        }
                    } else {
                        tracing::warn!("wtype failed, trying uinput fallback");
                        self.keyboard.send_paste().await?;
                    }
                }
                Err(_) => {
                    tracing::warn!("wtype not found, trying uinput fallback");
                    self.keyboard.send_paste().await?;
                }
            }
        } else {
            // Use uinput on X11
            tracing::debug!("Sending Ctrl+V via uinput (X11)");
            self.keyboard.send_paste().await?;
        }
        
        // Give the paste time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        tracing::info!("Paste command sent");
        Ok(())
    }
}

