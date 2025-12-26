# croaker Architecture

## Overview

croaker is a speech-to-text daemon that runs in the background and injects transcribed text at the cursor position. It uses Groq's Whisper API for transcription and Groq's LLM API for text cleanup.

## Components

### State Machine

The daemon uses a state machine with the following states:

- **Idle**: Waiting for input
- **Recording**: Capturing audio
- **Processing**: Transcribing and cleaning up text
- **Outputting**: Injecting text into active application

State transitions are triggered by events:
- `StartRecording`: Begin audio capture
- `StopRecording`: Stop capture and start processing
- `Cancel`: Abort current operation
- `ProcessingComplete`: Text ready to output
- `OutputComplete`: Text injection finished
- `ToggleOutputMode`: Change output mode (direct/clipboard/both)
- `ToggleLanguage`: Cycle to next configured language

### Input Sources

1. **evdev (Push-to-talk)**: Monitors `/dev/input/event*` for keyboard events
2. **D-Bus Portal (Toggle)**: Uses `org.freedesktop.portal.GlobalShortcuts` for compositor shortcuts
3. **Unix Socket (CLI)**: IPC interface for command-line control

### Audio Capture

Uses `pw-record` (PipeWire) to capture audio:
- Spawns child process with temp WAV file
- Kills process on stop
- Returns path to WAV file for transcription

### Transcription Pipeline

1. **Whisper API**: Sends audio file to Groq Whisper endpoint
   - Uses currently selected language from language toggle
   - Language can be changed at runtime without restarting daemon
2. **LLM Cleanup**: Sends raw transcription to Groq LLM with cleanup prompt
3. **Output**: Injects cleaned text according to current output mode (direct/clipboard/both)

### Text Output

Text insertion strategy varies by compositor and output mode:

**Output Modes:**
- **`direct`**: Types text directly at cursor position (may fallback to clipboard on Wayland)
- **`clipboard`**: Only copies text to clipboard (no automatic paste)
- **`both`**: Copies to clipboard AND tries to paste/type automatically

**Compositor-specific behavior:**

1. **Wayland (KDE/Hyprland)**: Uses `wtype` for reliable automatic text insertion
   - These compositors support the virtual keyboard protocol
   - Works seamlessly without user intervention
   - In `both` mode, uses `wtype` to paste after copying to clipboard

2. **Wayland (GNOME)**: ⚠️ Limited support
   - GNOME doesn't support virtual keyboard protocol
   - Tries `wtype` first (fails)
   - Falls back to uinput Ctrl+V (may work, but unreliable)
   - If both fail, sends notification asking user to paste manually
   - Text is already in clipboard, user just needs to press Ctrl+V
   - In `clipboard` mode, only copies to clipboard (no paste attempt)

3. **X11**: Uses `/dev/uinput` virtual keyboard
   - Maps ASCII characters to Linux keycodes
   - Handles shift modifier for uppercase/symbols
   - Configurable delay between keystrokes
   - In `both` mode, tries direct typing first, falls back to clipboard paste if needed

4. **Clipboard Fallback**: For non-ASCII characters or when direct typing fails
   - Saves current clipboard (if restore enabled)
   - Copies text to clipboard via `wl-copy`
   - Sends Ctrl+V via uinput (or `wtype` on Wayland)
   - Restores original clipboard (if enabled)

### Visual Feedback

Visual feedback showing recording/processing state:
- **System Tray** (default): Uses StatusNotifierItem D-Bus protocol
  - Shows colored icon based on state (grey=idle, red=recording, orange=processing, green=done)
  - Tooltip displays current mode and language
  - Right-click menu shows status and quit option
  - Portable across Linux DEs (KDE, GNOME with extensions, XFCE, etc.)
- **D-Bus Notifications**: Uses `notify-send` to display state messages
  - Works on all compositors
  - Shows recording/processing/outputting states

## Data Flow

```
User Input (evdev/portal/socket)
    ↓
State Machine (idle → recording)
    ↓
Audio Capture (pw-record → WAV file)
    ↓
State Machine (recording → processing)
    ↓
Whisper API (WAV → raw text)
    ↓
LLM Cleanup (raw text → cleaned text)
    ↓
State Machine (processing → outputting)
    ↓
Text Output (uinput/clipboard)
    ↓
State Machine (outputting → idle)
```

## Error Handling

- All modules use `thiserror` for typed errors
- State machine handles errors gracefully, returning to idle state
- Audio files are cleaned up in all code paths (success, error, cancel)

## Threading Model

- Main thread runs system tray (blocking message loop)
- Daemon runs in separate thread with its own tokio async runtime
- State machine runs in separate tokio task
- Socket server runs in separate tokio task
- Input monitors (evdev, portal) run in separate tokio tasks

## Configuration

- TOML config file at `~/.config/croaker/config.toml`
- API key file at `~/.config/croaker/groq.key` (chmod 600)
- Cleanup prompts in `~/.config/croaker/prompts/`
- Socket at `~/.cache/croaker/croaker.sock`

## Security Considerations

- API key stored in separate file with restricted permissions
- uinput/evdev require `input` group membership
- Socket file permissions should be restricted (TODO)

