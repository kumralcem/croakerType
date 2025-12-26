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
    device_path: std::path::PathBuf,
    key_code: u16,
    event_tx: mpsc::Sender<StateEvent>,
}

impl EvdevMonitor {
    pub fn new(config: &Config, event_tx: mpsc::Sender<StateEvent>) -> Result<Self, EvdevError> {
        tracing::debug!("Creating EvdevMonitor");
        // Find keyboard device
        let device_path = Self::find_keyboard_device()?;
        
        // Verify we can open it (but don't store the Device since it doesn't implement Send)
        let _device = Device::open(&device_path)?;
        tracing::info!("Successfully verified device: {:?}", device_path);

        // Parse key name to key code
        let key_code = Self::parse_key_name(&config.hotkeys.push_to_talk_key)?;

        Ok(Self {
            device_path,
            key_code,
            event_tx,
        })
    }

    fn find_keyboard_device() -> Result<std::path::PathBuf, EvdevError> {
        tracing::info!("Starting keyboard device detection");

        // Try event0 first - this is usually the main keyboard
        let primary_device = Path::new("/dev/input/event0");
        match Device::open(primary_device) {
            Ok(_device) => {
                tracing::info!("Successfully opened primary keyboard device: {:?}", primary_device);
                return Ok(primary_device.to_path_buf());
            }
            Err(e) => {
                tracing::warn!("Failed to open primary device {:?}: {} (error kind: {:?})", 
                              primary_device, e, e.kind());
                // Check if file exists and permissions
                if primary_device.exists() {
                    if let Ok(metadata) = std::fs::metadata(primary_device) {
                        tracing::warn!("Device exists but can't open. Permissions: {:?}", metadata.permissions());
                    }
                }
            }
        }

        // Fallback: try other devices
        for i in 1..32 {
            let path_str = format!("/dev/input/event{}", i);
            let path = Path::new(&path_str);
            match Device::open(path) {
                Ok(device) => {
                    tracing::debug!("Successfully opened device: {:?}", path);
                    let device_name = device.name().unwrap_or("unknown");
                    tracing::debug!("Checking device {:?} (name: {:?})", path, device_name);

                    // First check: can we open the device? If so, it's a candidate
                    // We'll be more permissive and try any device that doesn't look like a mouse
                    let name_lower = device_name.to_lowercase();

                    // Skip obvious non-keyboard devices
                    if name_lower.contains("mouse") ||
                       name_lower.contains("touchpad") ||
                       name_lower.contains("trackpoint") ||
                       name_lower.contains("stylus") ||
                       name_lower.contains("wacom") ||
                       name_lower.contains("tablet") {
                        tracing::debug!("Skipping non-keyboard device: {:?}", device_name);
                        continue;
                    }

                    // For now, try ANY device that can be opened and doesn't look like a mouse
                    // We'll be very permissive since different systems have different device setups
                    tracing::info!("Found accessible input device: {:?} (name: {:?})",
                                 path, device_name);

                    // Check capabilities if available, but don't be too strict
                    if let Some(keys) = device.supported_keys() {
                        tracing::debug!("Device has {} supported keys", keys.iter().count());
                        if keys.iter().count() > 0 {
                            tracing::info!("Selecting device with key support: {:?}", path);
                            return Ok(path.to_path_buf());
                        }
                    }

                    // If no key info available, still try the device
                    tracing::info!("Trying device without key info (might work): {:?}", path);
                    return Ok(path.to_path_buf());
                }
                Err(e) => {
                    if path.exists() {
                        tracing::debug!("Device {:?} exists but failed to open: {} (error kind: {:?})", path, e, e.kind());
                    } else {
                        tracing::trace!("Device {:?} doesn't exist (normal)", path);
                    }
                    // Continue checking other devices
                }
            }
        }

        tracing::warn!("No suitable input device found. Checked /dev/input/event0-31");
        tracing::warn!("This may be due to:");
        tracing::warn!("  1. Not being in the 'input' group (run: sudo usermod -aG input $USER)");
        tracing::warn!("  2. Need to log out and back in after adding to input group");
        tracing::warn!("  3. System may need different device detection logic");
        tracing::warn!("  4. Try running: sudo croaker serve (temporary test)");
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
        tracing::info!("Starting evdev monitor for key code: {} on device: {:?}", 
                      self.key_code, self.device_path);

        let key_code = self.key_code;
        let device_path = self.device_path.clone();
        let event_tx = self.event_tx.clone();
        let mut is_recording = false;

        // Run evdev monitoring in a blocking task since Device doesn't implement Send
        tokio::task::spawn_blocking(move || -> Result<(), EvdevError> {
            let mut device = Device::open(&device_path)?;
            tracing::info!("Opened device for monitoring: {:?}", device_path);
            
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

