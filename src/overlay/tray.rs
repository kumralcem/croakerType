use crate::daemon::state::DaemonState;
use crate::overlay::OverlayMessage;
use ksni::{self, Icon, ToolTip};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// System tray icon for croaker
pub struct CroakerTray {
    state: Arc<Mutex<TrayState>>,
}

struct TrayState {
    daemon_state: DaemonState,
    output_mode: String,
    language: String,
    temporary_message: Option<(String, Instant)>,
    flash_until: Option<Instant>,
}

impl CroakerTray {
    pub fn new() -> Self {
        Self::with_state(Arc::new(Mutex::new(TrayState {
            daemon_state: DaemonState::Idle,
            output_mode: "Both".to_string(),
            language: "en".to_string(),
            temporary_message: None,
            flash_until: None,
        })))
    }

    fn with_state(state: Arc<Mutex<TrayState>>) -> Self {
        Self { state }
    }
    
    fn get_icon_name(&self) -> String {
        let state = self.state.lock().unwrap();
        match state.daemon_state {
            DaemonState::Idle => "audio-input-microphone".to_string(),
            DaemonState::Recording => "media-record".to_string(),
            DaemonState::Processing => "view-refresh".to_string(),
            DaemonState::Outputting => "dialog-ok".to_string(),
        }
    }
    
    fn get_tooltip(&self) -> String {
        let mut state = self.state.lock().unwrap();
        
        // Clear expired temporary messages
        if let Some((_, timestamp)) = state.temporary_message {
            if timestamp.elapsed() > Duration::from_secs(3) {
                state.temporary_message = None;
            }
        }
        
        let status = match state.daemon_state {
            DaemonState::Idle => "Ready",
            DaemonState::Recording => "● Recording...",
            DaemonState::Processing => "Processing...",
            DaemonState::Outputting => "Outputting...",
        };
        
        // Show temporary message if present, otherwise show normal tooltip
        if let Some((ref msg, _)) = state.temporary_message {
            format!("{}\n\nCroaker: {}\nMode: {} | Lang: {}", 
                msg, status, state.output_mode, state.language.to_uppercase())
        } else {
            format!("Croaker: {}\nMode: {} | Lang: {}", 
                status, state.output_mode, state.language.to_uppercase())
        }
    }
    
    fn show_temporary_message(state: &Arc<Mutex<TrayState>>, message: String) {
        let mut tray_state = state.lock().unwrap();
        tray_state.temporary_message = Some((message, Instant::now()));
    }
    
    fn get_color(&self) -> (u8, u8, u8) {
        let state = self.state.lock().unwrap();
        
        // Flash bright blue when mode changes
        if let Some(flash_time) = state.flash_until {
            if flash_time.elapsed() < Duration::from_millis(500) {
                return (100, 150, 255); // Bright blue flash
            }
        }
        
        match state.daemon_state {
            DaemonState::Idle => (128, 128, 128),      // Grey
            DaemonState::Recording => (255, 60, 60),   // Red
            DaemonState::Processing => (255, 180, 60), // Orange
            DaemonState::Outputting => (60, 200, 60),  // Green
        }
    }
}

impl ksni::Tray for CroakerTray {
    fn id(&self) -> String {
        "croaker".to_string()
    }
    
    fn icon_name(&self) -> String {
        self.get_icon_name()
    }
    
    fn title(&self) -> String {
        "Croaker".to_string()
    }
    
    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Croaker".to_string(),
            description: self.get_tooltip(),
            icon_name: self.get_icon_name(),
            icon_pixmap: vec![],
        }
    }
    
    fn icon_pixmap(&self) -> Vec<Icon> {
        // Create a simple 22x22 colored circle icon
        let (r, g, b) = self.get_color();
        let size = 22;
        let mut argb_data = Vec::with_capacity(size * size * 4);
        
        let center = size as f32 / 2.0;
        let radius = center - 2.0;
        
        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 - center;
                let dy = y as f32 - center;
                let dist = (dx * dx + dy * dy).sqrt();
                
                if dist <= radius {
                    // Inside circle - use state color
                    argb_data.push(255); // A
                    argb_data.push(r);   // R
                    argb_data.push(g);   // G
                    argb_data.push(b);   // B
                } else if dist <= radius + 1.0 {
                    // Anti-aliased edge
                    let alpha = ((radius + 1.0 - dist) * 255.0) as u8;
                    argb_data.push(alpha);
                    argb_data.push(r);
                    argb_data.push(g);
                    argb_data.push(b);
                } else {
                    // Outside circle - transparent
                    argb_data.push(0);
                    argb_data.push(0);
                    argb_data.push(0);
                    argb_data.push(0);
                }
            }
        }
        
        vec![Icon {
            width: size as i32,
            height: size as i32,
            data: argb_data,
        }]
    }
    
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        
        let state = self.state.lock().unwrap();
        let status_text = match state.daemon_state {
            DaemonState::Idle => format!("Ready | {} | [{}]", state.output_mode, state.language.to_uppercase()),
            DaemonState::Recording => "● Recording...".to_string(),
            DaemonState::Processing => "◐ Processing...".to_string(),
            DaemonState::Outputting => "✓ Outputting...".to_string(),
        };
        drop(state);
        
        vec![
            StandardItem {
                label: status_text,
                enabled: false,
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".to_string(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }.into(),
        ]
    }
}

/// Run the system tray. This blocks and processes messages.
pub fn run_tray(message_rx: std::sync::mpsc::Receiver<OverlayMessage>) -> anyhow::Result<()> {
    use ksni::blocking::TrayMethods;
    
    // NOTE: When croaker is auto-started very early in a login session, the StatusNotifierWatcher
    // (tray host) might not be available yet. If we try to spawn the tray once and give up, the
    // daemon continues running but the user never sees the tray icon. We keep retrying until it
    // succeeds, while still processing messages and sending mode-change notifications.
    let state = Arc::new(Mutex::new(TrayState {
        daemon_state: DaemonState::Idle,
        output_mode: "Both".to_string(),
        language: "en".to_string(),
        temporary_message: None,
        flash_until: None,
    }));

    let mut tray_handle = None;
    let mut spawn_backoff = Duration::from_millis(200);
    let mut last_spawn_attempt = Instant::now() - spawn_backoff;
    let mut warned_missing_dbus = false;
    
    // Process messages with timeout to periodically check for expired temporary messages
    loop {
        // Try to spawn (or re-spawn) the tray service if it's not up yet.
        if tray_handle.is_none() && last_spawn_attempt.elapsed() >= spawn_backoff {
            last_spawn_attempt = Instant::now();

            if !warned_missing_dbus && std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
                warned_missing_dbus = true;
                tracing::warn!(
                    "DBUS_SESSION_BUS_ADDRESS is not set; tray/notifications may not work if croaker was started outside your desktop session (use systemd --user or XDG autostart)"
                );
            }

            let tray = CroakerTray::with_state(Arc::clone(&state));
            match tray.spawn() {
                Ok(handle) => {
                    tracing::info!("System tray started");
                    tray_handle = Some(handle);
                    spawn_backoff = Duration::from_millis(200);
                }
                Err(e) => {
                    // Common during early autostart: StatusNotifierWatcher isn't available yet.
                    // Keep retrying with backoff.
                    tracing::debug!("Tray not available yet (will retry): {}", e);
                    spawn_backoff = spawn_backoff.saturating_mul(2).min(Duration::from_secs(5));
                }
            }
        }

        // Check for messages with timeout to allow periodic updates
        match message_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(msg) => {
                {
                    let mut tray_state = state.lock().unwrap();
                    match msg {
                        OverlayMessage::State(daemon_state) => {
                            tray_state.daemon_state = daemon_state;
                        }
                        OverlayMessage::OutputMode(mode) => {
                            tray_state.output_mode = mode.clone();
                            // Show temporary message for mode change (in tooltip)
                            tray_state.temporary_message = Some((format!("Output mode: {}", mode), Instant::now()));
                            // Flash the icon to indicate change
                            tray_state.flash_until = Some(Instant::now() + Duration::from_millis(500));
                            // Show a brief notification that appears near the tray icon
                            drop(tray_state);
                            let _ = std::process::Command::new("notify-send")
                                .args(&[
                                    "--app-name=croaker",
                                    "--urgency=low",
                                    "--expire-time=2000",
                                    "--hint=int:transient:1",
                                    "--hint=string:x-croaker-tray:true",
                                    "croaker",
                                    &format!("Output mode: {}", mode)
                                ])
                                .spawn();
                        }
                        OverlayMessage::Language(lang) => {
                            tray_state.language = lang.clone();
                            // Show temporary message for language change (in tooltip)
                            tray_state.temporary_message = Some((format!("Language: {}", lang.to_uppercase()), Instant::now()));
                            // Flash the icon to indicate change
                            tray_state.flash_until = Some(Instant::now() + Duration::from_millis(500));
                            // Show a brief notification that appears near the tray icon
                            drop(tray_state);
                            let _ = std::process::Command::new("notify-send")
                                .args(&[
                                    "--app-name=croaker",
                                    "--urgency=low",
                                    "--expire-time=2000",
                                    "--hint=int:transient:1",
                                    "--hint=string:x-croaker-tray:true",
                                    "croaker",
                                    &format!("Language: {}", lang.to_uppercase())
                                ])
                                .spawn();
                        }
                        _ => {}
                    }
                }
                // Trigger tray icon update
                if let Some(handle) = tray_handle.as_ref() {
                    handle.update(|_| {});
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Check for expired messages/flash and update if needed
                let needs_update = {
                    let mut tray_state = state.lock().unwrap();
                    let mut updated = false;
                    
                    // Clear expired temporary message
                    if let Some((_, timestamp)) = tray_state.temporary_message {
                        if timestamp.elapsed() > Duration::from_secs(3) {
                            tray_state.temporary_message = None;
                            updated = true;
                        }
                    }
                    
                    // Clear expired flash
                    if let Some(flash_time) = tray_state.flash_until {
                        if flash_time.elapsed() >= Duration::from_millis(500) {
                            tray_state.flash_until = None;
                            updated = true;
                        } else {
                            // Still flashing, need to update to show flash
                            updated = true;
                        }
                    }
                    
                    updated
                };
                if needs_update {
                    if let Some(handle) = tray_handle.as_ref() {
                        handle.update(|_| {});
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }
    
    Ok(())
}
