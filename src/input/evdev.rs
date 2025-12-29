use crate::config::Config;
use crate::daemon::state::StateEvent;
use evdev::{Device, Key};
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
    #[error("Failed to parse shortcut: {0}")]
    ParseShortcutError(String),
}

#[derive(Debug, Clone)]
struct ParsedShortcut {
    needs_shift: bool,
    modifier_key_code: Option<u16>, // RightAlt, LeftAlt, RightCtrl, LeftCtrl, etc.
    main_key_code: u16, // O, L, etc.
}

pub struct EvdevMonitor {
    device_path: std::path::PathBuf,
    key_code: u16,
    output_mode_shortcut: Option<ParsedShortcut>,
    language_shortcut: Option<ParsedShortcut>,
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
        
        // Check if device supports key events and log key codes for debugging
        if let Some(keys) = test_device.supported_keys() {
            tracing::info!("Device supports {} key codes", keys.iter().count());
            
            // Diagnostic: Check key codes for all shortcuts
            let key_map = [
                ("RightCtrl", Key::KEY_RIGHTCTRL),
                ("LeftCtrl", Key::KEY_LEFTCTRL),
                ("RightAlt", Key::KEY_RIGHTALT),
                ("LeftAlt", Key::KEY_LEFTALT),
                ("RightShift", Key::KEY_RIGHTSHIFT),
                ("LeftShift", Key::KEY_LEFTSHIFT),
                ("O", Key::KEY_O),
                ("L", Key::KEY_L),
            ];
            
            tracing::info!("Keyboard key code diagnostics:");
            for (name, key) in key_map.iter() {
                if keys.contains(*key) {
                    tracing::info!("  ✅ {}: code {}", name, key.code());
                } else {
                    tracing::warn!("  ⚠️  {}: NOT SUPPORTED (expected code {})", name, key.code());
                }
            }
        }

        // Parse key name to key code
        let key_code = Self::parse_key_name(&config.hotkeys.push_to_talk_key)?;

        // Parse shortcuts from config
        let output_mode_shortcut = Self::parse_shortcut(&config.hotkeys.output_mode_shortcut)?;
        let language_shortcut = Self::parse_shortcut(&config.hotkeys.language_shortcut)?;

        if let Some(ref shortcut) = output_mode_shortcut {
            tracing::info!("Output mode shortcut: {:?}", shortcut);
        }
        if let Some(ref shortcut) = language_shortcut {
            tracing::info!("Language shortcut: {:?}", shortcut);
        }

        Ok(Self {
            device_path,
            key_code,
            output_mode_shortcut,
            language_shortcut,
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
        // Use evdev::Key enum to get the correct code for this system
        let key = match name.to_lowercase().as_str() {
            "rightalt" | "alt_r" => Key::KEY_RIGHTALT,
            "leftalt" | "alt_l" => Key::KEY_LEFTALT,
            "rightctrl" | "ctrl_r" => Key::KEY_RIGHTCTRL,
            "leftctrl" | "ctrl_l" => Key::KEY_LEFTCTRL,
            "rightshift" | "shift_r" => Key::KEY_RIGHTSHIFT,
            "leftshift" | "shift_l" => Key::KEY_LEFTSHIFT,
            _ => return Err(EvdevError::ParseKeyError(format!("Unknown key: {}", name))),
        };
        let code = key.code();
        tracing::debug!("Parsed key '{}' to code {}", name, code);
        Ok(code)
    }

    fn parse_shortcut(shortcut_str: &str) -> Result<Option<ParsedShortcut>, EvdevError> {
        if shortcut_str.is_empty() {
            return Ok(None);
        }

        let parts: Vec<&str> = shortcut_str.split('+').map(|s| s.trim()).collect();
        if parts.is_empty() {
            return Ok(None);
        }

        let mut needs_shift = false;
        let mut modifier_key_code = None;
        let mut main_key_code = None;

        for part in parts {
            let part_lower = part.to_lowercase();
            match part_lower.as_str() {
                "shift" => {
                    needs_shift = true;
                }
                "rightalt" | "alt_r" => {
                    modifier_key_code = Some(Key::KEY_RIGHTALT.code());
                }
                "leftalt" | "alt_l" => {
                    modifier_key_code = Some(Key::KEY_LEFTALT.code());
                }
                "rightctrl" | "ctrl_r" => {
                    modifier_key_code = Some(Key::KEY_RIGHTCTRL.code());
                }
                "leftctrl" | "ctrl_l" => {
                    modifier_key_code = Some(Key::KEY_LEFTCTRL.code());
                }
                "o" => {
                    main_key_code = Some(Key::KEY_O.code());
                }
                "l" => {
                    main_key_code = Some(Key::KEY_L.code());
                }
                _ => {
                    // Try to parse as a single character key
                    if part.len() == 1 {
                        let ch = part.chars().next().unwrap().to_ascii_lowercase();
                        if ch.is_ascii_alphabetic() {
                            // Use evdev::Key enum for letter keys
                            let key = match ch {
                                'a' => Key::KEY_A,
                                'b' => Key::KEY_B,
                                'c' => Key::KEY_C,
                                'd' => Key::KEY_D,
                                'e' => Key::KEY_E,
                                'f' => Key::KEY_F,
                                'g' => Key::KEY_G,
                                'h' => Key::KEY_H,
                                'i' => Key::KEY_I,
                                'j' => Key::KEY_J,
                                'k' => Key::KEY_K,
                                'l' => Key::KEY_L,
                                'm' => Key::KEY_M,
                                'n' => Key::KEY_N,
                                'o' => Key::KEY_O,
                                'p' => Key::KEY_P,
                                'q' => Key::KEY_Q,
                                'r' => Key::KEY_R,
                                's' => Key::KEY_S,
                                't' => Key::KEY_T,
                                'u' => Key::KEY_U,
                                'v' => Key::KEY_V,
                                'w' => Key::KEY_W,
                                'x' => Key::KEY_X,
                                'y' => Key::KEY_Y,
                                'z' => Key::KEY_Z,
                                _ => return Err(EvdevError::ParseShortcutError(format!("Unknown key in shortcut: {}", part))),
                            };
                            main_key_code = Some(key.code());
                        } else {
                            return Err(EvdevError::ParseShortcutError(format!("Unknown key in shortcut: {}", part)));
                        }
                    } else {
                        return Err(EvdevError::ParseShortcutError(format!("Unknown modifier/key in shortcut: {}", part)));
                    }
                }
            }
        }

        if let Some(key_code) = main_key_code {
            Ok(Some(ParsedShortcut {
                needs_shift,
                modifier_key_code,
                main_key_code: key_code,
            }))
        } else {
            Err(EvdevError::ParseShortcutError(format!("Shortcut missing main key: {}", shortcut_str)))
        }
    }

    pub async fn monitor(&mut self) -> Result<(), EvdevError> {
        tracing::info!("Starting evdev monitor for key code: {} on device: {:?}", 
                      self.key_code, self.device_path);

        let key_code = self.key_code;
        let device_path = self.device_path.clone();
        let event_tx = self.event_tx.clone();
        let output_mode_shortcut = self.output_mode_shortcut.clone();
        let language_shortcut = self.language_shortcut.clone();
        let mut is_recording = false;

        // Run evdev monitoring in a blocking task since Device doesn't implement Send
        tokio::task::spawn_blocking(move || -> Result<(), EvdevError> {
            let mut device = Device::open(&device_path)?;
            tracing::info!("Opened device for monitoring: {:?}", device_path);
            
            // Track modifier states for shortcut detection
            let mut shift_pressed = false;
            let mut modifier_pressed: Option<u16> = None; // Track which modifier is pressed (RightAlt, LeftAlt, RightCtrl, etc.)
            
            // Use evdev::Key enum to get correct key codes for this system
            let key_leftshift = Key::KEY_LEFTSHIFT.code();
            let key_rightshift = Key::KEY_RIGHTSHIFT.code();
            let key_rightalt = Key::KEY_RIGHTALT.code();
            let key_leftalt = Key::KEY_LEFTALT.code();
            let key_rightctrl = Key::KEY_RIGHTCTRL.code();
            let key_leftctrl = Key::KEY_LEFTCTRL.code();
            
            tracing::info!("Monitoring device. Push-to-talk key code: {}", key_code);
            tracing::info!("Modifier key codes - Shift: L={} R={}, Alt: L={} R={}, Ctrl: L={} R={}", 
                key_leftshift, key_rightshift, key_leftalt, key_rightalt, key_leftctrl, key_rightctrl);
            if let Some(ref shortcut) = output_mode_shortcut {
                tracing::info!("Output mode shortcut configured - modifier code: {:?}, main key code: {}", 
                    shortcut.modifier_key_code, shortcut.main_key_code);
            }
            if let Some(ref shortcut) = language_shortcut {
                tracing::info!("Language shortcut configured - modifier code: {:?}, main key code: {}", 
                    shortcut.modifier_key_code, shortcut.main_key_code);
            }
            
            loop {
                match device.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            if event.event_type() == evdev::EventType::KEY {
                                let event_key_code = event.code();
                                let event_value = event.value();
                                
                                // Track modifier states (1=press, 0=release, ignore 2=repeat)
                                match event_key_code {
                                    code if code == key_leftshift || code == key_rightshift => {
                                        if event_value == 1 {
                                            shift_pressed = true;
                                        } else if event_value == 0 {
                                            shift_pressed = false;
                                        }
                                    }
                                    code if code == key_rightalt || code == key_leftalt || 
                                           code == key_rightctrl || code == key_leftctrl => {
                                        if event_value == 1 {
                                            modifier_pressed = Some(event_key_code);
                                            // Only start recording if this is our push-to-talk key and Shift is NOT pressed
                                            if event_key_code == key_code && !shift_pressed && !is_recording {
                                                tracing::info!("Push-to-talk: start recording");
                                                is_recording = true;
                                                let _ = event_tx.try_send(StateEvent::StartRecording);
                                            }
                                        } else if event_value == 0 {
                                            if modifier_pressed == Some(event_key_code) {
                                                modifier_pressed = None;
                                            }
                                            // Stop recording if we were recording and this is our push-to-talk key
                                            if event_key_code == key_code && is_recording {
                                                tracing::info!("Push-to-talk: stop recording");
                                                is_recording = false;
                                                let _ = event_tx.try_send(StateEvent::StopRecording);
                                            }
                                        }
                                    }
                                    _ => {
                                        // Check for output mode shortcut
                                        if let Some(ref shortcut) = output_mode_shortcut {
                                            if event_key_code == shortcut.main_key_code && event_value == 1 {
                                                let shift_ok = !shortcut.needs_shift || shift_pressed;
                                                let modifier_ok = shortcut.modifier_key_code.is_none() || 
                                                    modifier_pressed == shortcut.modifier_key_code;
                                                if shift_ok && modifier_ok {
                                                    tracing::info!("Shortcut: Toggle output mode");
                                                    let _ = event_tx.try_send(StateEvent::ToggleOutputMode);
                                                }
                                            }
                                        }
                                        
                                        // Check for language shortcut
                                        if let Some(ref shortcut) = language_shortcut {
                                            if event_key_code == shortcut.main_key_code && event_value == 1 {
                                                let shift_ok = !shortcut.needs_shift || shift_pressed;
                                                let modifier_ok = shortcut.modifier_key_code.is_none() || 
                                                    modifier_pressed == shortcut.modifier_key_code;
                                                if shift_ok && modifier_ok {
                                                    tracing::info!("Shortcut: Toggle language");
                                                    let _ = event_tx.try_send(StateEvent::ToggleLanguage);
                                                }
                                            }
                                        }
                                        
                                        // Check if it's our push-to-talk key (for keys that aren't modifiers)
                                        if event_key_code == key_code && 
                                           key_code != key_rightalt && 
                                           key_code != key_leftalt &&
                                           key_code != key_rightctrl &&
                                           key_code != key_leftctrl {
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

