use crate::daemon::state::DaemonState;
use crate::overlay::OverlayMessage;
use ksni::{self, Icon, ToolTip};
use std::sync::{Arc, Mutex};

/// System tray icon for croaker
pub struct CroakerTray {
    state: Arc<Mutex<TrayState>>,
}

struct TrayState {
    daemon_state: DaemonState,
    output_mode: String,
    language: String,
}

impl CroakerTray {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(TrayState {
                daemon_state: DaemonState::Idle,
                output_mode: "Both".to_string(),
                language: "en".to_string(),
            })),
        }
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
        let state = self.state.lock().unwrap();
        let status = match state.daemon_state {
            DaemonState::Idle => "Ready",
            DaemonState::Recording => "● Recording...",
            DaemonState::Processing => "Processing...",
            DaemonState::Outputting => "Outputting...",
        };
        format!("Croaker: {}\nMode: {} | Lang: {}", 
            status, state.output_mode, state.language.to_uppercase())
    }
    
    fn get_color(&self) -> (u8, u8, u8) {
        let state = self.state.lock().unwrap();
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
    
    let tray = CroakerTray::new();
    let state = Arc::clone(&tray.state);
    
    // Spawn tray service using blocking API
    let handle = tray.spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn tray: {}", e))?;
    
    tracing::info!("System tray started");
    
    // Process messages
    while let Ok(msg) = message_rx.recv() {
        {
            let mut tray_state = state.lock().unwrap();
            match msg {
                OverlayMessage::State(daemon_state) => {
                    tray_state.daemon_state = daemon_state;
                }
                OverlayMessage::OutputMode(mode) => {
                    tray_state.output_mode = mode;
                }
                OverlayMessage::Language(lang) => {
                    tray_state.language = lang;
                }
                _ => {}
            }
        }
        // Trigger tray icon update
        handle.update(|_| {});
    }
    
    Ok(())
}
