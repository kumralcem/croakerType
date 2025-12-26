use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(String),
    #[error("Failed to parse TOML: {0}")]
    ParseError(String),
    #[error("Failed to read API key file: {0}")]
    KeyReadError(String),
    #[error("API key file is empty or invalid")]
    InvalidKey,
}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::ReadError(e.to_string())
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        ConfigError::ParseError(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub hotkeys: HotkeyConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub groq: GroqConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub overlay: OverlayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
}

fn default_language() -> String {
    "en".to_string()
}

fn default_languages() -> Vec<String> {
    vec!["en".to_string(), "tr".to_string(), "es".to_string(), "fr".to_string(), "de".to_string()]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    #[serde(default = "default_push_to_talk_key")]
    pub push_to_talk_key: String,
    #[serde(default = "default_true")]
    pub push_to_talk_enabled: bool,
    #[serde(default = "default_toggle_shortcut")]
    pub toggle_shortcut: String,
    #[serde(default = "default_true")]
    pub toggle_enabled: bool,
    #[serde(default = "default_cancel_shortcut")]
    pub cancel_shortcut: String,
    #[serde(default = "default_output_mode_shortcut")]
    pub output_mode_shortcut: String,
    #[serde(default = "default_language_shortcut")]
    pub language_shortcut: String,
}

fn default_push_to_talk_key() -> String {
    "RightAlt".to_string()
}

fn default_toggle_shortcut() -> String {
    "Super+Shift+R".to_string()
}

fn default_cancel_shortcut() -> String {
    "Escape".to_string()
}

fn default_output_mode_shortcut() -> String {
    "Shift+RightAlt+O".to_string()
}

fn default_language_shortcut() -> String {
    "Shift+RightAlt+L".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    #[serde(default = "default_device")]
    pub device: String,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_device() -> String {
    "default".to_string()
}

fn default_sample_rate() -> u32 {
    16000
}

fn default_format() -> String {
    "s16".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroqConfig {
    #[serde(default = "default_key_file")]
    pub key_file: String,
    #[serde(default = "default_whisper_model")]
    pub whisper_model: String,
    #[serde(default = "default_true")]
    pub cleanup_enabled: bool,
    #[serde(default = "default_cleanup_model")]
    pub cleanup_model: String,
    #[serde(default = "default_cleanup_prompt_file")]
    pub cleanup_prompt_file: String,
}

fn default_key_file() -> String {
    "~/.config/croaker/groq.key".to_string()
}

fn default_whisper_model() -> String {
    "whisper-large-v3-turbo".to_string()
}

fn default_cleanup_model() -> String {
    "llama-3.3-70b-versatile".to_string()
}

fn default_cleanup_prompt_file() -> String {
    "~/.config/croaker/prompts/default.txt".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    Direct,
    Clipboard,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(default = "default_keystroke_delay")]
    pub keystroke_delay_ms: u64,
    #[serde(default = "default_true")]
    pub clipboard_restore: bool,
    #[serde(default = "default_output_mode")]
    pub output_mode: OutputMode,
}

fn default_keystroke_delay() -> u64 {
    5
}

fn default_output_mode() -> OutputMode {
    OutputMode::Both
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default = "default_position")]
    pub position: String,
    #[serde(default = "default_size")]
    pub size: u32,
    #[serde(default = "default_opacity")]
    pub opacity: f64,
}

fn default_backend() -> String {
    "notification".to_string()
}

fn default_position() -> String {
    "top-center".to_string()
}

fn default_size() -> u32 {
    48
}

fn default_opacity() -> f64 {
    0.9
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            hotkeys: HotkeyConfig::default(),
            audio: AudioConfig::default(),
            groq: GroqConfig::default(),
            output: OutputConfig::default(),
            overlay: OverlayConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            languages: default_languages(),
        }
    }
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            push_to_talk_key: default_push_to_talk_key(),
            push_to_talk_enabled: default_true(),
            toggle_shortcut: default_toggle_shortcut(),
            toggle_enabled: default_true(),
            cancel_shortcut: default_cancel_shortcut(),
            output_mode_shortcut: default_output_mode_shortcut(),
            language_shortcut: default_language_shortcut(),
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device: default_device(),
            sample_rate: default_sample_rate(),
            format: default_format(),
        }
    }
}

impl Default for GroqConfig {
    fn default() -> Self {
        Self {
            key_file: default_key_file(),
            whisper_model: default_whisper_model(),
            cleanup_enabled: default_true(),
            cleanup_model: default_cleanup_model(),
            cleanup_prompt_file: default_cleanup_prompt_file(),
        }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            keystroke_delay_ms: default_keystroke_delay(),
            clipboard_restore: default_true(),
            output_mode: default_output_mode(),
        }
    }
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            backend: default_backend(),
            position: default_position(),
            size: default_size(),
            opacity: default_opacity(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::config_path()?;
        
        if !config_path.exists() {
            tracing::info!("Config file not found at {:?}, creating default config file", config_path);
            Self::create_default_config(&config_path)?;
        }

        let contents = fs::read_to_string(&config_path)
            .map_err(|e| ConfigError::ReadError(e.to_string()))?;
        let mut config: Config = toml::from_str(&contents)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;
        
        // Expand paths
        config.groq.key_file = Self::expand_path(&config.groq.key_file)
            .map_err(|e| ConfigError::ReadError(format!("Path expansion error: {}", e)))?;
        config.groq.cleanup_prompt_file = Self::expand_path(&config.groq.cleanup_prompt_file)
            .map_err(|e| ConfigError::ReadError(format!("Path expansion error: {}", e)))?;
        
        Ok(config)
    }

    pub fn create_default_config(config_path: &PathBuf) -> Result<(), ConfigError> {
        // Create config directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ConfigError::ReadError(format!("Failed to create config directory: {}", e)))?;
        }

        let default_config = r#"# croaker Configuration File
# All options are shown below with their default values.
# Uncomment and modify any option you want to change.

[general]
# Language code for transcription (e.g., "en", "es", "fr")
language = "en"
# List of languages to toggle between (use language codes like "en", "tr", "es", "fr", "de", etc.)
languages = ["en", "tr", "es", "fr", "de"]

[hotkeys]
# Push-to-talk key (e.g., "RightAlt", "LeftAlt", "RightCtrl", "LeftCtrl")
push_to_talk_key = "RightAlt"
# Enable push-to-talk mode
push_to_talk_enabled = true
# Toggle shortcut for recording (e.g., "Super+Shift+R")
toggle_shortcut = "Super+Shift+R"
# Enable toggle shortcut mode
toggle_enabled = true
# Cancel shortcut
cancel_shortcut = "Escape"
# Output mode toggle shortcut (cycles between direct/clipboard/both)
output_mode_shortcut = "Shift+RightAlt+O"
# Language toggle shortcut (cycles through configured languages)
language_shortcut = "Shift+RightAlt+L"

[audio]
# Audio device (use "default" for system default)
device = "default"
# Sample rate in Hz
sample_rate = 16000
# Audio format (s16, s24, s32, f32, f64)
format = "s16"

[groq]
# Path to Groq API key file
key_file = "~/.config/croaker/groq.key"
# Whisper model for transcription (use any Groq-supported Whisper model slug)
# Examples: whisper-large-v3-turbo, whisper-large-v3, whisper-medium, etc.
whisper_model = "whisper-large-v3-turbo"
# Enable LLM cleanup of transcription
cleanup_enabled = true
# LLM model for text cleanup (use any Groq-supported model slug)
# Examples: llama-3.3-70b-versatile, llama-3.1-8b-instant, openai/gpt-oss-20b, etc.
# Check https://console.groq.com/docs/models for available models
cleanup_model = "llama-3.3-70b-versatile"
# Path to cleanup prompt file
cleanup_prompt_file = "~/.config/croaker/prompts/default.txt"

[output]
# Delay between keystrokes in milliseconds (for uinput typing)
keystroke_delay_ms = 5
# Restore clipboard after pasting (disabled - user preference)
clipboard_restore = false
# Output mode: "direct" (type directly), "clipboard" (copy to clipboard only), "both" (do both)
output_mode = "both"

[overlay]
# Enable visual overlay
enabled = true
# Overlay backend: "notification" (default, desktop notifications), "gtk" (pulsing dot), "layer-shell", or "auto"
backend = "notification"
# Overlay position: "top-center", "top-left", "top-right", "bottom-center", etc.
position = "top-center"
# Overlay size in pixels
size = 48
# Overlay opacity (0.0 to 1.0)
opacity = 0.9
"#;

        fs::write(config_path, default_config)
            .map_err(|e| ConfigError::ReadError(format!("Failed to write config file: {}", e)))?;
        
        tracing::info!("Created default config file at {:?}", config_path);
        Ok(())
    }

    pub fn config_path() -> Result<PathBuf, ConfigError> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| ConfigError::ReadError("Could not find config directory".to_string()))?;
        Ok(config_dir.join("croaker").join("config.toml"))
    }

    pub fn load_api_key(&self) -> Result<String, ConfigError> {
        // Expand path if it contains ~
        let expanded_path = if self.groq.key_file.starts_with("~/") {
            Self::expand_path(&self.groq.key_file)
                .map_err(|e| ConfigError::KeyReadError(format!("Path expansion failed: {}", e)))?
        } else {
            self.groq.key_file.clone()
        };
        
        let key_path = Path::new(&expanded_path);
        
        if !key_path.exists() {
            return Err(ConfigError::KeyReadError(format!("API key file not found: {:?}", key_path)));
        }

        let key = fs::read_to_string(key_path)
            .map_err(|e| ConfigError::KeyReadError(e.to_string()))?
            .trim()
            .to_string();
        
        if key.is_empty() {
            return Err(ConfigError::InvalidKey);
        }

        Ok(key)
    }

    pub fn load_cleanup_prompt(&self) -> Result<String, ConfigError> {
        // Expand path if it contains ~
        let expanded_path = if self.groq.cleanup_prompt_file.starts_with("~/") {
            Self::expand_path(&self.groq.cleanup_prompt_file)
                .map_err(|e| ConfigError::ReadError(format!("Path expansion failed: {}", e)))?
        } else {
            self.groq.cleanup_prompt_file.clone()
        };
        
        let prompt_path = Path::new(&expanded_path);
        
        if !prompt_path.exists() {
            // Try default prompt from config directory
            let default_prompt = Self::default_prompt_path()
                .map_err(|e| ConfigError::ReadError(e.to_string()))?;
            if default_prompt.exists() {
                return Ok(fs::read_to_string(default_prompt)
                    .map_err(|e| ConfigError::ReadError(e.to_string()))?
                    .trim().to_string());
            }
            // Return hardcoded default if no file exists
            return Ok(Self::default_prompt_text());
        }

        Ok(fs::read_to_string(prompt_path)
            .map_err(|e| ConfigError::ReadError(e.to_string()))?
            .trim().to_string())
    }

    fn default_prompt_path() -> Result<PathBuf, std::io::Error> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not find config directory"
            ))?;
        Ok(config_dir.join("croaker").join("prompts").join("default.txt"))
    }

    fn default_prompt_text() -> String {
        "Clean up this speech-to-text transcription:
- Fix punctuation and capitalization
- Remove filler words (um, uh, like, you know)
- Fix obvious transcription errors
- Preserve meaning and tone

Output only the cleaned text, nothing else.".to_string()
    }

    fn expand_path(path: &str) -> Result<String, std::io::Error> {
        if path.starts_with("~/") {
            let home = dirs::home_dir()
                .ok_or_else(|| std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Could not find home directory"
                ))?;
            Ok(home.join(&path[2..]).to_string_lossy().to_string())
        } else {
            Ok(path.to_string())
        }
    }
}

