use crate::config::Config;
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;
use std::sync::Mutex;
use thiserror::Error;
use tokio::time::Duration;

#[derive(Debug, Error)]
pub enum UinputError {
    #[error("Failed to open uinput device: {0}")]
    OpenError(String),
    #[error("Failed to write to uinput: {0}")]
    WriteError(String),
    #[error("Unsupported character: {0}")]
    UnsupportedChar(char),
}

impl From<std::io::Error> for UinputError {
    fn from(e: std::io::Error) -> Self {
        UinputError::OpenError(e.to_string())
    }
}

// Linux input event codes
const KEY_LEFTCTRL: u16 = 29;
const KEY_LEFTSHIFT: u16 = 42;
const KEY_V: u16 = 47;

// Key codes for ASCII printable characters
const KEY_A: u16 = 30;
const KEY_B: u16 = 48;
const KEY_D: u16 = 32;
const KEY_E: u16 = 33;
const KEY_F: u16 = 34;
const KEY_G: u16 = 35;
const KEY_H: u16 = 36;
const KEY_I: u16 = 23;
const KEY_J: u16 = 37;
const KEY_K: u16 = 38;
const KEY_L: u16 = 39;
const KEY_M: u16 = 50;
const KEY_N: u16 = 49;
const KEY_O: u16 = 24;
const KEY_P: u16 = 25;
const KEY_Q: u16 = 16;
const KEY_R: u16 = 17;
const KEY_S: u16 = 31;
const KEY_T: u16 = 20;
const KEY_U: u16 = 22;
const KEY_W: u16 = 17;
const KEY_X: u16 = 45;
const KEY_Y: u16 = 21;
const KEY_Z: u16 = 44;
const KEY_1: u16 = 2;
const KEY_2: u16 = 3;
const KEY_3: u16 = 4;
const KEY_4: u16 = 5;
const KEY_5: u16 = 6;
const KEY_6: u16 = 7;
const KEY_7: u16 = 8;
const KEY_8: u16 = 9;
const KEY_9: u16 = 10;
const KEY_0: u16 = 11;
const KEY_MINUS: u16 = 12;
const KEY_EQUAL: u16 = 13;
const KEY_LEFTBRACE: u16 = 26;
const KEY_RIGHTBRACE: u16 = 27;
const KEY_BACKSLASH: u16 = 43;
const KEY_SEMICOLON: u16 = 39;
const KEY_APOSTROPHE: u16 = 40;
const KEY_GRAVE: u16 = 41;
const KEY_COMMA: u16 = 51;
const KEY_DOT: u16 = 52;
const KEY_SLASH: u16 = 53;
const KEY_SPACE: u16 = 57;
const KEY_ENTER: u16 = 28;
const KEY_TAB: u16 = 15;

// Input event structures
#[repr(C, packed)]
struct InputEvent {
    time: TimeVal,
    type_: u16,
    code: u16,
    value: i32,
}

#[repr(C, packed)]
struct TimeVal {
    tv_sec: i64,
    tv_usec: i64,
}

const EV_KEY: u16 = 1;
const EV_SYN: u16 = 0;
const SYN_REPORT: u16 = 0;
const KEY_PRESS: i32 = 1;
const KEY_RELEASE: i32 = 0;

pub struct UinputKeyboard {
    file: Mutex<std::fs::File>,
    delay_ms: u64,
}

impl UinputKeyboard {
    pub fn new(config: &Config) -> Result<Self, UinputError> {
        let uinput_path = Path::new("/dev/uinput");
        
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(uinput_path)
            .map_err(|e| UinputError::OpenError(e.to_string()))?;

        let fd = file.as_raw_fd();

        // Enable key events
        unsafe {
            let ev_key: libc::c_ulong = 1 << EV_KEY;
            let result = libc::ioctl(fd, uinput_ioctl::UI_SET_EVBIT as libc::c_ulong, ev_key);
            if result < 0 {
                return Err(UinputError::OpenError(format!("ioctl UI_SET_EVBIT failed: {}", std::io::Error::last_os_error())));
            }
        }

        // Enable all key codes we might need (1-255)
        for key_code in 1..=255u16 {
            unsafe {
                let _ = libc::ioctl(fd, uinput_ioctl::UI_SET_KEYBIT as libc::c_ulong, key_code as libc::c_ulong);
            }
        }

        // Create device using the old uinput_user_dev method
        // First write the structure, then call UI_DEV_CREATE
        unsafe {
            let mut uidev = std::mem::zeroed::<libc::uinput_user_dev>();
            
            // Set device name (must be null-terminated, max 80 chars)
            let name_bytes = b"croaker virtual keyboard\0";
            let name_len = (name_bytes.len() - 1).min(79); // -1 to exclude null terminator, max 79 chars
            std::ptr::copy_nonoverlapping(
                name_bytes.as_ptr() as *const libc::c_char,
                uidev.name.as_mut_ptr() as *mut libc::c_char,
                name_len,
            );
            // Ensure null termination
            *uidev.name.as_mut_ptr().add(name_len) = 0;
            
            // Set input device ID
            // Use BUS_VIRTUAL (0x06) for uinput devices
            uidev.id.bustype = 0x06; // BUS_VIRTUAL
            uidev.id.vendor = 0x1;
            uidev.id.product = 0x1;
            uidev.id.version = 1;
            
            // Set ff_effects_max (required field, 0 means no force feedback)
            uidev.ff_effects_max = 0;
            
            // Write the structure to the file descriptor (old method)
            use std::io::Write;
            let uidev_bytes = std::slice::from_raw_parts(
                &uidev as *const _ as *const u8,
                std::mem::size_of::<libc::uinput_user_dev>(),
            );
            if file.write_all(uidev_bytes).is_err() {
                return Err(UinputError::OpenError("Failed to write uinput_user_dev structure".to_string()));
            }
            file.flush().ok();

            // Now create the device
            let result = libc::ioctl(fd, uinput_ioctl::UI_DEV_CREATE as libc::c_ulong);
            if result < 0 {
                let err = std::io::Error::last_os_error();
                return Err(UinputError::OpenError(format!(
                    "ioctl UI_DEV_CREATE failed: {} (errno: {}). Make sure you're in the 'input' group and have logged out/in.",
                    err, err.raw_os_error().unwrap_or(-1)
                )));
            }
            tracing::info!("uinput virtual keyboard device created successfully");
        }

        // Note: Device creation is synchronous, no need to wait

        Ok(Self {
            file: Mutex::new(file),
            delay_ms: config.output.keystroke_delay_ms,
        })
    }

    pub async fn type_text(&self, text: &str) -> Result<(), UinputError> {
        tracing::info!("Typing text via uinput: {} chars", text.len());
        tracing::debug!("Text content: {:?}", text);

        for ch in text.chars() {
            if ch == '\n' {
                self.send_key(KEY_ENTER, true).await?;
                self.send_key(KEY_ENTER, false).await?;
            } else if ch == '\t' {
                self.send_key(KEY_TAB, true).await?;
                self.send_key(KEY_TAB, false).await?;
            } else if ch.is_ascii() {
                let (key_code, needs_shift) = self.char_to_keycode(ch)?;
                
                if needs_shift {
                    self.send_key(KEY_LEFTSHIFT, true).await?;
                }
                
                self.send_key(key_code, true).await?;
                self.send_key(key_code, false).await?;
                
                if needs_shift {
                    self.send_key(KEY_LEFTSHIFT, false).await?;
                }
            } else {
                // Non-ASCII character - caller should use clipboard fallback
                return Err(UinputError::UnsupportedChar(ch));
            }

            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }

        Ok(())
    }

    pub async fn send_paste(&self) -> Result<(), UinputError> {
        tracing::debug!("Sending Ctrl+V keystroke");
        // Send Ctrl+V
        self.send_key(KEY_LEFTCTRL, true).await?;
        tokio::time::sleep(Duration::from_millis(10)).await;
        self.send_key(KEY_V, true).await?;
        tokio::time::sleep(Duration::from_millis(10)).await;
        self.send_key(KEY_V, false).await?;
        tokio::time::sleep(Duration::from_millis(10)).await;
        self.send_key(KEY_LEFTCTRL, false).await?;
        tracing::debug!("Ctrl+V keystroke sent");
        Ok(())
    }

    async fn send_key(&self, code: u16, press: bool) -> Result<(), UinputError> {
        let value = if press { KEY_PRESS } else { KEY_RELEASE };
        
        // Get current time for the event timestamp
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        
        let event = InputEvent {
            time: TimeVal {
                tv_sec: now.as_secs() as i64,
                tv_usec: now.subsec_micros() as i64,
            },
            type_: EV_KEY,
            code,
            value,
        };

        // Use blocking write since uinput is fast
        use std::io::Write;
        let event_bytes = unsafe {
            std::slice::from_raw_parts(
                &event as *const _ as *const u8,
                std::mem::size_of::<InputEvent>(),
            )
        };
        
        let mut file = self.file.lock().unwrap();
        file.write_all(event_bytes).map_err(|e| {
            tracing::error!("Failed to write uinput event for key {}: {}", code, e);
            UinputError::WriteError(e.to_string())
        })?;

        // Send sync event
        let sync_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let sync_event = InputEvent {
            time: TimeVal {
                tv_sec: sync_now.as_secs() as i64,
                tv_usec: sync_now.subsec_micros() as i64,
            },
            type_: EV_SYN,
            code: SYN_REPORT,
            value: 0,
        };

        let sync_bytes = unsafe {
            std::slice::from_raw_parts(
                &sync_event as *const _ as *const u8,
                std::mem::size_of::<InputEvent>(),
            )
        };
        
        file.write_all(sync_bytes).map_err(|e| UinputError::WriteError(e.to_string()))?;
        file.flush().map_err(|e| UinputError::WriteError(e.to_string()))?;

        Ok(())
    }

    fn char_to_keycode(&self, ch: char) -> Result<(u16, bool), UinputError> {
        match ch {
            'a'..='z' => Ok((KEY_A + (ch as u16 - b'a' as u16), false)),
            'A'..='Z' => Ok((KEY_A + (ch.to_ascii_lowercase() as u16 - b'a' as u16), true)),
            '0' => Ok((KEY_0, false)),
            '1'..='9' => Ok((KEY_1 + (ch as u16 - b'1' as u16), false)),
            ' ' => Ok((KEY_SPACE, false)),
            '-' => Ok((KEY_MINUS, false)),
            '=' => Ok((KEY_EQUAL, false)),
            '[' => Ok((KEY_LEFTBRACE, false)),
            ']' => Ok((KEY_RIGHTBRACE, false)),
            '\\' => Ok((KEY_BACKSLASH, false)),
            ';' => Ok((KEY_SEMICOLON, false)),
            '\'' => Ok((KEY_APOSTROPHE, false)),
            '`' => Ok((KEY_GRAVE, false)),
            ',' => Ok((KEY_COMMA, false)),
            '.' => Ok((KEY_DOT, false)),
            '/' => Ok((KEY_SLASH, false)),
            '!' => Ok((KEY_1, true)),
            '@' => Ok((KEY_2, true)),
            '#' => Ok((KEY_3, true)),
            '$' => Ok((KEY_4, true)),
            '%' => Ok((KEY_5, true)),
            '^' => Ok((KEY_6, true)),
            '&' => Ok((KEY_7, true)),
            '*' => Ok((KEY_8, true)),
            '(' => Ok((KEY_9, true)),
            ')' => Ok((KEY_0, true)),
            '_' => Ok((KEY_MINUS, true)),
            '+' => Ok((KEY_EQUAL, true)),
            '{' => Ok((KEY_LEFTBRACE, true)),
            '}' => Ok((KEY_RIGHTBRACE, true)),
            '|' => Ok((KEY_BACKSLASH, true)),
            ':' => Ok((KEY_SEMICOLON, true)),
            '"' => Ok((KEY_APOSTROPHE, true)),
            '~' => Ok((KEY_GRAVE, true)),
            '<' => Ok((KEY_COMMA, true)),
            '>' => Ok((KEY_DOT, true)),
            '?' => Ok((KEY_SLASH, true)),
            _ => Err(UinputError::UnsupportedChar(ch)),
        }
    }
}

impl Drop for UinputKeyboard {
    fn drop(&mut self) {
        if let Ok(file) = self.file.lock() {
            let fd = file.as_raw_fd();
            unsafe {
                let _ = libc::ioctl(fd, uinput_ioctl::UI_DEV_DESTROY as libc::c_ulong);
            }
        }
    }
}

// uinput ioctl constants
mod uinput_ioctl {
    pub const UI_SET_EVBIT: u32 = 0x40045564;
    pub const UI_SET_KEYBIT: u32 = 0x40045565;
    pub const UI_DEV_CREATE: u32 = 0x5501;
    pub const UI_DEV_DESTROY: u32 = 0x5502;
}
