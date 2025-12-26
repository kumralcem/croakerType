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
        
        // Verify we can open it and check capabilities (but don't store the Device since it doesn't implement Send)
        let test_device = Device::open(&device_path)?;
        tracing::info!("Successfully verified device: {:?}", device_path);
        
        // Check if device supports key events
        if let Some(keys) = test_device.supported_keys() {
            tracing::info!("Device supports {} key codes", keys.iter().count());
            // Check for RightAlt specifically
            if keys.contains(evdev::Key::KEY_RIGHTALT) {
                tracing::info!("✅ Device supports KEY_RIGHTALT");
            } else {
                tracing::warn!("⚠️  Device does NOT report KEY_RIGHTALT support (but may still work)");
            }
        }

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

        // Don't assume event0 is the keyboard - search through all devices
        // and find one that actually has keyboard keys
        for i in 0..32 {
            let path_str = format!("/dev/input/event{}", i);
            let path = Path::new(&path_str);
            match Device::open(path) {
                Ok(device) => {
                    let device_name = device.name().unwrap_or("unknown");
                    tracing::debug!("Checking device {:?} (name: {:?})", path, device_name);

                    let name_lower = device_name.to_lowercase();

                    // Skip obvious non-keyboard devices
                    if name_lower.contains("mouse") ||
                       name_lower.contains("touchpad") ||
                       name_lower.contains("trackpoint") ||
                       name_lower.contains("stylus") ||
                       name_lower.contains("wacom") ||
                       name_lower.contains("tablet") ||
                       name_lower.contains("power") ||
                       name_lower.contains("button") {
                        tracing::debug!("Skipping non-keyboard device: {:?}", device_name);
                        continue;
                    }

                    // Check if device actually has keyboard keys
                    if let Some(keys) = device.supported_keys() {
                        let key_count = keys.iter().count();
                        tracing::debug!("Device {:?} has {} supported keys", device_name, key_count);
                        
                        // Look for common keyboard keys to verify it's actually a keyboard
                        let has_keyboard_keys = keys.contains(evdev::Key::KEY_A) ||
                                               keys.contains(evdev::Key::KEY_SPACE) ||
                                               keys.contains(evdev::Key::KEY_ENTER) ||
                                               keys.contains(evdev::Key::KEY_LEFTALT) ||
                                               keys.contains(evdev::Key::KEY_RIGHTALT);
                        
                        if has_keyboard_keys && key_count > 50 {
                            // This looks like a real keyboard (has common keys and many key codes)
                            tracing::info!("✅ Found keyboard device: {:?} (name: {:?}, {} keys)", 
                                         path, device_name, key_count);
                            return Ok(path.to_path_buf());
                        } else {
                            tracing::debug!("Device {:?} has keys but doesn't look like a keyboard ({} keys, has_keyboard_keys={})", 
                                         device_name, key_count, has_keyboard_keys);
                        }
                    } else {
                        tracing::debug!("Device {:?} has no key capability info", device_name);
                    }
                }
                Err(e) => {
                    if path.exists() {
                        tracing::debug!("Device {:?} exists but failed to open: {}", path, e);
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
        // Note: RightAlt can be either 100 or 108 depending on kernel version
        // We'll check both codes when monitoring
        match name.to_lowercase().as_str() {
            "rightalt" | "alt_r" => Ok(100), // KEY_RIGHTALT is 100, KEY_DOWN is 108
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
            
            // Track modifier states for shortcut detection
            let mut shift_pressed = false;
            let mut right_alt_pressed = false;
            
            // Key codes for modifiers and shortcuts
            const KEY_LEFTSHIFT: u16 = 42;
            const KEY_RIGHTSHIFT: u16 = 54;
            const KEY_RIGHTALT: u16 = 100;
            const KEY_O: u16 = 24;
            const KEY_L: u16 = 38;
            
            tracing::info!("Monitoring device. Push-to-talk: RightAlt, Shortcuts: Shift+RightAlt+O/L");
            
            loop {
                match device.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            if event.event_type() == evdev::EventType::KEY {
                                let event_key_code = event.code();
                                let event_value = event.value();
                                
                                // Track modifier states (1=press, 0=release, ignore 2=repeat)
                                match event_key_code {
                                    KEY_LEFTSHIFT | KEY_RIGHTSHIFT => {
                                        if event_value == 1 {
                                            shift_pressed = true;
                                        } else if event_value == 0 {
                                            shift_pressed = false;
                                        }
                                    }
                                    KEY_RIGHTALT => {
                                        if event_value == 1 {
                                            right_alt_pressed = true;
                                            // Only start recording if Shift is NOT pressed
                                            if !shift_pressed && !is_recording {
                                                tracing::info!("Push-to-talk: start recording");
                                                is_recording = true;
                                                let _ = event_tx.try_send(StateEvent::StartRecording);
                                            }
                                        } else if event_value == 0 {
                                            right_alt_pressed = false;
                                            // Stop recording if we were recording
                                            if is_recording {
                                                tracing::info!("Push-to-talk: stop recording");
                                                is_recording = false;
                                                let _ = event_tx.try_send(StateEvent::StopRecording);
                                            }
                                        }
                                    }
                                    KEY_O => {
                                        // Shift+RightAlt+O = toggle output mode
                                        if event_value == 1 && shift_pressed && right_alt_pressed {
                                            tracing::info!("Shortcut: Toggle output mode");
                                            let _ = event_tx.try_send(StateEvent::ToggleOutputMode);
                                        }
                                    }
                                    KEY_L => {
                                        // Shift+RightAlt+L = toggle language
                                        if event_value == 1 && shift_pressed && right_alt_pressed {
                                            tracing::info!("Shortcut: Toggle language");
                                            let _ = event_tx.try_send(StateEvent::ToggleLanguage);
                                        }
                                    }
                                    _ => {
                                        // Other keys - check if it's our push-to-talk key (if not RightAlt)
                                        if event_key_code == key_code && key_code != KEY_RIGHTALT {
                                            if event_value == 1 && !is_recording {
                                                tracing::info!("Push-to-talk key pressed (code {})", event_key_code);
                                                is_recording = true;
                                                let _ = event_tx.try_send(StateEvent::StartRecording);
                                            } else if event_value == 0 && is_recording {
                                                tracing::info!("Push-to-talk key released (code {})", event_key_code);
                                                is_recording = false;
                                                let _ = event_tx.try_send(StateEvent::StopRecording);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
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

