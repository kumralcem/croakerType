# croaker

Speech-to-text daemon for Linux/Wayland that captures audio, transcribes via Groq Whisper API, cleans up via Groq LLM, and injects text at cursor position.

## Features

- **Two recording modes:**
  - Push-to-talk: Hold a key to record, release to process
  - Toggle: Press once to start, press again to stop and process

- **Processing pipeline:**
  - Audio → Groq Whisper (transcription) → Groq LLM (cleanup) → cleaned text

- **Text output:**
  - **All platforms**: Text is copied to clipboard via `wl-copy` (Wayland clipboard utility)
  - **Note**: Automatic pasting is unreliable across platforms - croaker copies text to clipboard, and you paste manually with Ctrl+V
  - The daemon attempts automatic insertion on some compositors but this often fails - clipboard copy is the reliable method

- **Visual feedback:**
  - Default: System tray icon that changes color based on state (grey=idle, red=recording, orange=processing, green=done)
  - Optional: Desktop notifications (notification backend)
  - Works across all desktop environments (GNOME, KDE, Hyprland)

- **Output modes:**
  - **Direct**: Attempts to type text directly (often fails - falls back to clipboard)
  - **Clipboard**: Copy to clipboard only (recommended - you paste manually with Ctrl+V)
  - **Both**: Copies to clipboard AND attempts automatic paste (may fail - clipboard is reliable)
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

### Start daemon manually

```bash
croaker serve
```

### Auto-start on Login (Recommended)

To have croaker start automatically when you log in:

**Step 1: Install the binary system-wide** (if you haven't already)
```bash
sudo cp target/release/croaker /usr/local/bin/
```

**Step 2: Create systemd user service**
```bash
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/croaker.service << 'EOF'
[Unit]
Description=croaker speech-to-text daemon
After=graphical-session.target sound.target

[Service]
Type=simple
ExecStart=/usr/local/bin/croaker serve
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info
Environment="DBUS_SESSION_BUS_ADDRESS=unix:path=%t/bus"

[Install]
WantedBy=default.target
EOF
```

**Step 3: Enable and start**
```bash
systemctl --user daemon-reload
systemctl --user enable --now croaker
```

**Step 4: Verify**
- Check service status: `systemctl --user status croaker`
- Look for tray icon: You should see a grey microphone icon in your system tray
- Check logs: `journalctl --user -u croaker -f`

**Note**: The daemon includes retry logic for the system tray - if started early in the login sequence, it will automatically retry connecting to the tray until it succeeds. The tray icon should appear within a few seconds of login.

See [QUICKSTART.md](QUICKSTART.md) for detailed troubleshooting.

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

## How It Works

### Text Output

**Important**: croaker copies transcribed text to your clipboard. Automatic pasting is unreliable across platforms, so you should paste manually with Ctrl+V after recording.

- **All platforms**: Text is copied to clipboard using `wl-copy` (Wayland clipboard utility)
- **Automatic pasting**: The daemon attempts automatic insertion on some compositors (KDE, Hyprland) but this often fails or is blocked by security policies
- **Recommended workflow**: Use "clipboard" mode and paste manually with Ctrl+V when you see the tray icon turn green

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

