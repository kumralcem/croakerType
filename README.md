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
  - Default: Desktop notifications showing recording/processing state
  - Optional: Pulsing dot overlay with audio level visualization (GTK backend)
  - Works across all desktop environments (GNOME, KDE, Hyprland)

## Installation

### Dependencies

```bash
# Fedora
sudo dnf install pipewire-utils wl-clipboard

# Add user to input group (required for uinput and evdev)
sudo usermod -aG input $USER
# Log out and back in
```

### Build

```bash
cargo build --release
```

## Configuration

Create `~/.config/croaker/config.toml`:

```toml
[general]
language = "en"

[hotkeys]
push_to_talk_key = "RightAlt"
push_to_talk_enabled = true
toggle_shortcut = "Super+Shift+R"
toggle_enabled = true
cancel_shortcut = "Escape"

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

[overlay]
enabled = true
backend = "notification"  # Options: "notification" (default), "gtk" (pulsing dot), "layer-shell", "auto"
position = "top-center"
size = 48
opacity = 0.9
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
croaker toggle    # Toggle recording
croaker cancel    # Cancel current operation
croaker status    # Get current state
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

croaker provides visual feedback via overlay backends:

- **`notification`** (default): Desktop notifications showing state (Recording, Processing, Outputting). Works on all compositors.
- **`gtk`**: Floating pulsing dot indicator with audio level visualization. Shows a colored dot that pulses with audio input during recording. Works on GNOME and other GTK-based environments.
- **`layer-shell`**: Layer-shell overlay for wlroots compositors (Hyprland, Sway). Requires feature flag.
- **`auto`**: Automatically selects the best available backend.

Configure via `overlay.backend` in your config file.

### Build Notes

The project compiles with some warnings (mostly unused imports and deprecated macros). These are harmless and don't affect functionality.

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for system design details.

## License

MIT

