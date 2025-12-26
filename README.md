# croaker

Speech-to-text daemon for Linux/Wayland that captures audio, transcribes via Groq Whisper API, cleans up via Groq LLM, and injects text at cursor position.

## Features

- **Two recording modes:**
  - Push-to-talk: Hold a key to record, release to process
  - Toggle: Press once to start, press again to stop and process

- **Processing pipeline:**
  - Audio → Groq Whisper (transcription) → Groq LLM (cleanup) → cleaned text

- **Text injection:**
  - **Wayland (KDE/Hyprland)**: Uses `wtype` for reliable automatic text insertion
  - **Wayland (GNOME)**: GNOME doesn't support virtual keyboard protocol - automatic insertion may fail, manual paste (Ctrl+V) may be required
  - **X11**: Uses `/dev/uinput` virtual keyboard
  - **Non-ASCII fallback**: `wl-copy` + synthetic Ctrl+V

- **Visual feedback:**
  - Default: System tray icon that changes color based on state (grey=idle, red=recording, orange=processing, green=done)
  - Optional: Desktop notifications (notification backend)
  - Works across all desktop environments (GNOME, KDE, Hyprland)

- **Output modes:**
  - **Direct**: Type text directly at cursor position
  - **Clipboard**: Copy to clipboard only (no automatic paste)
  - **Both**: Copy to clipboard AND automatically paste/type
  - Toggle at runtime with `Shift+RightAlt+O` or `croaker toggle-output-mode`

- **Multi-language support:**
  - Configure multiple languages in config (default includes English, Turkish, Spanish, French, German)
  - Toggle between languages at runtime with `Shift+RightAlt+L` or `croaker toggle-language`
  - Tray tooltip shows current language
  - Selected language is used for next transcription

## Installation

### Dependencies

```bash
# Fedora
sudo dnf install pipewire-utils wl-clipboard openssl-devel

# Ubuntu/Debian
sudo apt install pipewire-utils wl-clipboard libssl-dev

# Arch
sudo pacman -S pipewire-utils wl-clipboard openssl

# Add user to input group (required for uinput and evdev)
sudo usermod -aG input $USER
# Log out and back in for group membership to take effect
```

### Build

```bash
cargo build --release
```

### System Installation (Recommended)

```bash
# Install binary system-wide
sudo cp target/release/croaker /usr/local/bin/

# Install default configuration
sudo mkdir -p /etc/croaker/prompts
sudo cp config/default_prompt.txt /etc/croaker/prompts/default.txt
```

## Configuration

Create `~/.config/croaker/config.toml`:

```toml
[general]
language = "en"
# List of languages to toggle between (use language codes like "en", "tr", "es", "fr", "de", etc.)
languages = ["en", "tr", "es", "fr", "de"]

[hotkeys]
push_to_talk_key = "RightAlt"
push_to_talk_enabled = true
toggle_shortcut = "Super+Shift+R"
toggle_enabled = true
cancel_shortcut = "Escape"
# Output mode toggle shortcut (cycles between direct/clipboard/both)
output_mode_shortcut = "Shift+RightAlt+O"
# Language toggle shortcut (cycles through configured languages)
language_shortcut = "Shift+RightAlt+L"

[audio]
device = "default"
sample_rate = 16000
format = "s16"

[groq]
key_file = "~/.config/croaker/groq.key"
whisper_model = "whisper-large-v3-turbo"
cleanup_enabled = true
cleanup_model = "llama-3.3-70b-versatile"
cleanup_prompt_file = "~/.config/croaker/prompts/default.txt"

[output]
keystroke_delay_ms = 5
clipboard_restore = true
# Output mode: "direct" (type directly), "clipboard" (copy to clipboard only), "both" (do both)
output_mode = "both"

[overlay]
enabled = true
backend = "tray"  # Options: "tray" (default, system tray icon), "notification" (desktop notifications)
```

Create `~/.config/croaker/groq.key` with your Groq API key:

```bash
echo "your-api-key-here" > ~/.config/croaker/groq.key
chmod 600 ~/.config/croaker/groq.key
```

## Usage

### Start daemon

```bash
croaker serve
```

### Control daemon

```bash
croaker toggle              # Toggle recording
croaker cancel              # Cancel current operation
croaker status              # Get current state
croaker toggle-output-mode  # Toggle output mode (direct/clipboard/both)
croaker toggle-language     # Toggle language (cycles through configured languages)
```

### Configure

```bash
croaker configure  # Interactive setup wizard
```

## Compositor Compatibility

### Text Insertion

- **KDE Plasma**: ✅ Full support - automatic text insertion works reliably via `wtype`
- **Hyprland/Sway**: ✅ Full support - automatic text insertion works reliably via `wtype`
- **GNOME**: ⚠️ **Limited support** - GNOME doesn't support the virtual keyboard protocol. Automatic insertion may fail, and you may need to paste manually with Ctrl+V. The daemon will notify you when text is ready.

### Visual Feedback

croaker provides visual feedback via the system tray:

- **`tray`** (default): System tray icon that changes color based on state:
  - Grey: Idle (ready to record)
  - Red: Recording
  - Orange: Processing
  - Green: Done/Outputting
  - Tooltip shows current mode and language
  - Right-click menu shows status and quit option
- **`notification`**: Desktop notifications showing state (Recording, Processing, Outputting). Works on all compositors but can clutter notification history.

Configure via `overlay.backend` in your config file.

### Build Notes

The project compiles with some warnings (mostly unused imports and deprecated macros). These are harmless and don't affect functionality.

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for system design details.

