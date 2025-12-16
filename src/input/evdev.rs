use crate::config::Config;
use crate::daemon::state::StateEvent;
use evdev::Device;
use std::path::Path;
use thiserror::Error;
use tokio::sync::mpsc;

#[derive(Debug, Error)]
pub enum EvdevError {
    #[error("Failed to open device: {0}")]
    OpenError(#[from] std::io::Error),
    #[error("No keyboard device found")]
    NoDevice,
    #[error("Failed to parse key name: {0}")]
    ParseKeyError(String),
}

pub struct EvdevMonitor {
    device: Device,
    key_code: u16,
    event_tx: mpsc::Sender<StateEvent>,
}

impl EvdevMonitor {
    pub fn new(config: &Config, event_tx: mpsc::Sender<StateEvent>) -> Result<Self, EvdevError> {
        // Find keyboard device
        let device_path = Self::find_keyboard_device()?;
        let device = Device::open(&device_path)?;

        // Parse key name to key code
        let key_code = Self::parse_key_name(&config.hotkeys.push_to_talk_key)?;

        Ok(Self {
            device,
            key_code,
            event_tx,
        })
    }

    fn find_keyboard_device() -> Result<std::path::PathBuf, EvdevError> {
        // Enumerate /dev/input/event*
        for i in 0..32 {
            let path_str = format!("/dev/input/event{}", i);
            let path = Path::new(&path_str);
            if let Ok(device) = Device::open(path) {
                // Check if it has keyboard capabilities
                if device.supported_keys().map_or(false, |keys| {
                    keys.contains(evdev::Key::KEY_A)
                }) {
                    tracing::info!("Found keyboard device: {:?}", path);
                    return Ok(path.to_path_buf());
                }
            }
        }

        Err(EvdevError::NoDevice)
    }

    fn parse_key_name(name: &str) -> Result<u16, EvdevError> {
        // Map common key names to evdev key codes
        // This is a simplified mapping - full implementation would use evdev's key name parsing
        match name.to_lowercase().as_str() {
            "rightalt" | "alt_r" => Ok(100), // KEY_RIGHTALT (code 100, not 108!)
            "leftalt" | "alt_l" => Ok(56),  // KEY_LEFTALT
            "rightctrl" | "ctrl_r" => Ok(97), // KEY_RIGHTCTRL
            "leftctrl" | "ctrl_l" => Ok(29), // KEY_LEFTCTRL
            "rightshift" | "shift_r" => Ok(54), // KEY_RIGHTSHIFT
            "leftshift" | "shift_l" => Ok(42), // KEY_LEFTSHIFT
            _ => Err(EvdevError::ParseKeyError(format!("Unknown key: {}", name))),
        }
    }

    pub async fn monitor(&mut self) -> Result<(), EvdevError> {
        tracing::info!("Starting evdev monitor for key code: {}", self.key_code);

        let key_code = self.key_code;
        let event_tx = self.event_tx.clone();
        let mut is_recording = false;

        // Run evdev monitoring in a blocking task since Device doesn't implement Send
        let device_path = Self::find_keyboard_device()?;
        
        tokio::task::spawn_blocking(move || -> Result<(), EvdevError> {
            let mut device = Device::open(&device_path)?;
            
            loop {
                match device.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            if event.event_type() == evdev::EventType::KEY {
                                let event_key_code = event.code();
                                if event_key_code == key_code {
                                    let value = event.value();
                                    
                                    // value: 0 = release, 1 = press, 2 = repeat
                                    if value == 1 && !is_recording {
                                        // Key pressed - start recording
                                        tracing::debug!("Push-to-talk key pressed");
                                        is_recording = true;
                                        let _ = event_tx.try_send(StateEvent::StartRecording);
                                    } else if value == 0 && is_recording {
                                        // Key released - stop recording
                                        tracing::debug!("Push-to-talk key released");
                                        is_recording = false;
                                        let _ = event_tx.try_send(StateEvent::StopRecording);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No events available, wait a bit
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(e) => {
                        tracing::error!("evdev error: {}", e);
                        return Err(EvdevError::OpenError(e));
                    }
                }
            }
        }).await.map_err(|e| EvdevError::OpenError(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Task error: {}", e)
        )))??;

        Ok(())
    }
}

