# croaker Setup Guide

## Prerequisites

### System Dependencies

```bash
# Fedora
sudo dnf install pipewire-utils wl-clipboard

# Ubuntu/Debian
sudo apt install pipewire-utils wl-clipboard

# Arch
sudo pacman -S pipewire-utils wl-clipboard
```

### User Permissions

Add your user to the `input` group (required for uinput and evdev):

```bash
sudo usermod -aG input $USER
```

**Important**: You must log out and back in for group membership to take effect.

### Groq API Key

1. Sign up at https://groq.com
2. Get your API key from the dashboard
3. Create the key file:

```bash
mkdir -p ~/.config/croaker
echo "your-api-key-here" > ~/.config/croaker/groq.key
chmod 600 ~/.config/croaker/groq.key
```

## Configuration

### Basic Config

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
# Output mode toggle shortcut (cycles between direct/clipboard/both)
output_mode_shortcut = "Shift+RightAlt+O"
# Language toggle shortcut (cycles through configured languages)
language_shortcut = "Shift+RightAlt+L"

[groq]
key_file = "~/.config/croaker/groq.key"
whisper_model = "whisper-large-v3-turbo"
cleanup_enabled = true
cleanup_model = "llama-3.3-70b-versatile"

[output]
# Output mode: "direct" (type directly), "clipboard" (copy to clipboard only), "both" (do both)
output_mode = "both"

[overlay]
enabled = true
backend = "tray"  # Options: "tray" (default, system tray icon), "notification"
```

### Custom Cleanup Prompt

Create `~/.config/croaker/prompts/default.txt`:

```
Clean up this speech-to-text transcription:
- Fix punctuation and capitalization
- Remove filler words (um, uh, like, you know)
- Fix obvious transcription errors
- Preserve meaning and tone

Output only the cleaned text, nothing else.
```

## Running

### Start Daemon

```bash
croaker serve
```

### Test Configuration

```bash
croaker configure
```

This will check:
- API key file exists
- User is in `input` group
- Dependencies are installed

### Control Daemon

```bash
# Toggle recording
croaker toggle

# Cancel current operation
croaker cancel

# Check status
croaker status

# Toggle output mode (direct/clipboard/both)
croaker toggle-output-mode

# Toggle language (cycles through configured languages)
croaker toggle-language
```

### Output Modes

croaker supports three output modes that control how transcribed text is handled:

- **`direct`**: Attempts to type text directly (often fails - falls back to clipboard)
- **`clipboard`**: Only copies text to clipboard (recommended - you paste manually with Ctrl+V)
- **`both`**: Copies to clipboard AND attempts automatic paste (may fail - clipboard is reliable)

**Note**: Automatic pasting is unreliable across platforms. The recommended workflow is to use "clipboard" mode and paste manually with Ctrl+V when you see the tray icon turn green.

Toggle between modes using:
- CLI: `croaker toggle-output-mode`
- Hotkey: `Shift+RightAlt+O` (default, configurable)
- Socket command: `echo "toggle-output-mode" | nc -U ~/.cache/croaker/croaker.sock`

A notification will appear showing the current mode.

### Language Toggle

Configure multiple languages in `~/.config/croaker/config.toml`:

```toml
[general]
languages = ["en", "es", "fr", "de", "it", "pt"]  # Add any language codes
```

Toggle between languages using:
- CLI: `croaker toggle-language`
- Hotkey: `Shift+RightAlt+L` (default, configurable)
- Socket command: `echo "toggle-language" | nc -U ~/.cache/croaker/croaker.sock`

A notification will appear showing the current language (e.g., "Language: EN"). The selected language is used for the next transcription.

## Troubleshooting

### "Permission denied" when accessing /dev/uinput

Make sure you're in the `input` group and have logged out/in:

```bash
groups | grep input
```

### "Daemon is not running"

Start the daemon first:

```bash
croaker serve
```

### Portal shortcuts not working

Make sure your compositor supports `org.freedesktop.portal.GlobalShortcuts`:
- GNOME 45+
- KDE Plasma
- wlroots-based compositors (Hyprland, Sway)

For older compositors, use push-to-talk mode instead.

### Visual feedback not showing

If the system tray icon isn't appearing, check that your desktop environment supports the StatusNotifierItem protocol. You can configure the feedback backend in your config file:
- `tray` (default): System tray icon that changes color based on state (grey=idle, red=recording, orange=processing, green=done). Portable across Linux DEs.
- `notification`: Desktop notifications (may clutter notification history)

### Text Output Behavior

**Important**: croaker copies transcribed text to your clipboard. Automatic pasting is unreliable across all platforms, so you should paste manually with Ctrl+V.

The daemon may attempt automatic pasting on some compositors, but this often fails due to security policies or compositor limitations. The reliable workflow is:
1. Record your speech (tray icon turns red)
2. Wait for processing (tray icon turns orange, then green)
3. Paste manually with Ctrl+V (text is already in your clipboard)


## Auto-start

### systemd user service

Create `~/.config/systemd/user/croaker.service`:

```ini
[Unit]
Description=croaker speech-to-text daemon
After=graphical-session.target

[Service]
Type=simple
ExecStart=/usr/local/bin/croaker serve
Restart=on-failure

[Install]
WantedBy=default.target
```

Enable:

```bash
systemctl --user enable croaker
systemctl --user start croaker
```

