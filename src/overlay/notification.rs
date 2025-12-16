use crate::daemon::state::DaemonState;
use crate::overlay::{Overlay, OverlayError};
use std::process::Command;
use std::sync::Mutex;

pub struct NotificationOverlay {
    current_notification_id: Mutex<Option<u32>>,
}

impl NotificationOverlay {
    pub fn new() -> Result<Self, OverlayError> {
        Ok(Self {
            current_notification_id: Mutex::new(None),
        })
    }

    fn send_notification(&self, message: &str, urgency: &str) {
        let mut cmd = Command::new("notify-send");
        cmd.arg("--app-name=croaker")
            .arg(format!("--urgency={}", urgency))
            .arg("croaker")
            .arg(message);

        if let Ok(mut id_guard) = self.current_notification_id.lock() {
            if let Some(id) = *id_guard {
                cmd.arg(format!("--replace-id={}", id));
            }
        }

        if let Ok(output) = cmd.output() {
            if let Ok(id_str) = String::from_utf8(output.stdout) {
                if let Ok(id) = id_str.trim().parse::<u32>() {
                    if let Ok(mut id_guard) = self.current_notification_id.lock() {
                        *id_guard = Some(id);
                    }
                }
            }
        }
    }
}

impl Overlay for NotificationOverlay {
    fn update_state(&self, state: DaemonState) {
        let (message, urgency) = match state {
            DaemonState::Recording => ("Recording...", "normal"),
            DaemonState::Processing => ("Processing...", "normal"),
            DaemonState::Outputting => ("Outputting...", "normal"),
            DaemonState::Idle => return,
        };
        
        self.send_notification(message, urgency);
    }

    fn update_audio_level(&self, _level: f32) {
        // Notifications don't support audio level visualization
    }

    fn show(&self) {
        // Notifications are shown automatically
    }

    fn hide(&self) {
        // Close current notification
        if let Ok(id_guard) = self.current_notification_id.lock() {
            if let Some(id) = *id_guard {
                let _ = Command::new("notify-send")
                    .arg(format!("--close={}", id))
                    .output();
            }
        }
    }
}

