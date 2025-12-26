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

**Important**: croaker copies transcribed text to your clipboard. Automatic pasting is unreliable across all platforms.

**Output Modes:**
- **`direct`**: Attempts to type text directly (often fails - falls back to clipboard)
- **`clipboard`**: Only copies text to clipboard (recommended - user pastes manually with Ctrl+V)
- **`both`**: Copies to clipboard AND attempts automatic paste (may fail - clipboard is reliable)

**How it works:**
- Text is always copied to clipboard using `wl-copy` (Wayland clipboard utility)
- The daemon may attempt automatic pasting using `wtype` (Wayland) or `/dev/uinput` (X11), but this often fails due to:
  - Security policies preventing apps from simulating keyboard input
  - Compositor limitations (GNOME doesn't support virtual keyboard protocol)
  - Application focus issues
- **Recommended workflow**: Use "clipboard" mode and paste manually with Ctrl+V

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

