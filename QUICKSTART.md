# Quick Start Guide

## How It Works

The daemon runs in the background and listens for hotkeys. When you press a button:

1. **Push-to-talk mode**: Hold RightAlt (default) → starts recording → release → stops and processes
2. **Toggle mode**: Press Super+Shift+R (default) → starts recording → press again → stops and processes

The daemon automatically:
- Captures audio via PipeWire
- Transcribes via Groq Whisper API
- Cleans up text via Groq LLM
- Injects text at cursor position
- Shows visual feedback via desktop notifications

## Setup (5 minutes)

```bash
# 1. Install dependencies
sudo dnf install pipewire-utils wl-clipboard

# 2. Add to input group
sudo usermod -aG input $USER
# Log out and back in!

# 3. Set API key
mkdir -p ~/.config/croaker
echo "your-groq-api-key" > ~/.config/croaker/groq.key
chmod 600 ~/.config/croaker/groq.key

# 4. Build
cargo build --release

# 5. Install system-wide (recommended)
sudo cp target/release/croaker /usr/local/bin/
sudo mkdir -p /etc/croaker/prompts
sudo cp config/default_prompt.txt /etc/croaker/prompts/default.txt

# 6. Start daemon
croaker serve
```

## Usage

Once the daemon is running:

- **Hold RightAlt** and speak → release when done
- OR **Press Super+Shift+R** → speak → press again when done

The transcribed text will appear at your cursor!

## Auto-start on Login

**Important**: Make sure you've completed steps 1-5 above (especially installing the binary to `/usr/local/bin/croaker`) before setting up auto-start.

### Step 1: Create systemd user service

```bash
# Create the service directory if it doesn't exist
mkdir -p ~/.config/systemd/user

# Create the service file
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
# Ensure DBUS is available for tray/notifications
Environment="DBUS_SESSION_BUS_ADDRESS=unix:path=%t/bus"

[Install]
WantedBy=default.target
EOF
```

### Step 2: Enable and start the service

```bash
# Reload systemd to recognize the new service
systemctl --user daemon-reload

# Enable auto-start on login (creates symlink)
systemctl --user enable croaker

# Start the service now (don't wait for next login)
systemctl --user start croaker

# Verify it's running
systemctl --user status croaker
```

### Step 3: Verify tray icon appears

After starting the service, you should see a **grey microphone icon** in your system tray. If you don't see it:

1. **Check the logs** for any errors:
   ```bash
   journalctl --user -u croaker -f
   ```
   You should see "System tray started" in the logs.

2. **Wait a few seconds** - The tray icon may take a moment to appear if your desktop environment's tray service wasn't ready yet. The daemon will automatically retry connecting to the tray.

3. **Verify the service is running**:
   ```bash
   systemctl --user is-active croaker
   ```
   Should output: `active`

### Troubleshooting Auto-start

**Service won't start?**
- Make sure the binary exists: `ls -l /usr/local/bin/croaker`
- Check logs: `journalctl --user -u croaker -n 50`
- Verify you're using `--user` (not `sudo systemctl`)

**Tray icon doesn't appear?**
- The daemon includes retry logic - it will keep trying to connect to the tray even if started early in the login sequence
- Check logs: `journalctl --user -u croaker | grep -i tray`
- Make sure your desktop environment supports system tray (StatusNotifierItem protocol)
- The daemon will continue running even if the tray fails - you can still use hotkeys, just without visual feedback

**Service not starting on login?**
- Verify it's enabled: `systemctl --user is-enabled croaker` (should output: `enabled`)
- Check if systemd user services are running: `systemctl --user status`
- Some desktop environments require `loginctl enable-linger $USER` for user services to start on login

## Configuration

Edit `~/.config/croaker/config.toml` (created automatically with defaults):

- `push_to_talk_key`: Change the push-to-talk key (e.g., "LeftAlt", "RightCtrl")
- `toggle_shortcut`: Change the toggle shortcut (e.g., "Super+Shift+V")
- `cleanup_enabled`: Set to `false` to skip LLM cleanup (faster, less polished)
- `overlay.backend`: Overlay backend (`notification` default, `gtk` for pulsing dot, `layer-shell`, `auto`)

### Output Modes

Control how transcribed text is handled:

- `output_mode`: Set to `"direct"`, `"clipboard"`, or `"both"` (default: `"both"`)
- Toggle at runtime: `croaker toggle-output-mode` or `Shift+RightAlt+O`
- Shows notification with current mode

### Language Toggle

Configure multiple languages and switch between them:

- `languages`: List of language codes (e.g., `["en", "tr", "es", "fr", "de"]`)
- Toggle at runtime: `croaker toggle-language` or `Shift+RightAlt+L`
- Shows notification with current language (e.g., "Language: EN")

## Troubleshooting

**Hotkeys not working?**
- Check you're in `input` group: `groups | grep input`
- Check daemon is running: `croaker status`
- Try push-to-talk mode if portal shortcuts don't work
- **Keyboard device not found?** Run with debug logging: `RUST_LOG=debug croaker serve`
- **Key not detected?** Use `evtest` to verify your key codes (optional install):
  - Fedora: `sudo dnf install evtest`
  - Ubuntu/Debian: `sudo apt install evtest`
  - Arch: `sudo pacman -S evtest`
  - Run: `sudo evtest --list` then `sudo evtest /dev/input/eventX`

**No text appearing?**
- Check API key is correct: `cat ~/.config/croaker/groq.key`
- Check logs: `journalctl --user -u croaker -f`
- **GNOME users**: If you see "Text ready! Press Ctrl+V to paste" notification, paste manually - GNOME doesn't support automatic text insertion

**Permission errors?**
- Make sure you logged out/in after adding to `input` group
- Check `/dev/uinput` permissions: `ls -l /dev/uinput`

**GNOME-Specific Notes**

⚠️ **GNOME Limitation**: GNOME doesn't support the virtual keyboard protocol, so automatic text insertion may fail. When you see the notification "Text ready! Press Ctrl+V to paste", the text is already in your clipboard - just paste manually.

**KDE and Hyprland users**: Text insertion works automatically without any manual steps needed.


