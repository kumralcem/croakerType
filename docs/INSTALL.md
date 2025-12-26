# Installation Guide

## Quick Start

### 1. Install System Dependencies

```bash
# Fedora
sudo dnf install pipewire-utils wl-clipboard

# Ubuntu/Debian  
sudo apt install pipewire-utils wl-clipboard

# Arch
sudo pacman -S pipewire-utils wl-clipboard
```

### 2. Add User to Input Group

```bash
sudo usermod -aG input $USER
```

**Important**: Log out and back in for group membership to take effect.

### 3. Set Up Configuration

```bash
# Create config directory
mkdir -p ~/.config/croaker/prompts

# Create API key file (get your key from https://groq.com)
echo "your-groq-api-key-here" > ~/.config/croaker/groq.key
chmod 600 ~/.config/croaker/groq.key

# Copy default prompt
cp config/default_prompt.txt ~/.config/croaker/prompts/default.txt
```

### 4. Build

```bash
cd /home/cem/Sync/Projects/croakerType
cargo build --release
```

The binary will be at `target/release/croaker`.

### 5. System-Wide Installation (Recommended)

For a complete installation with auto-startup:

```bash
# Install the binary system-wide
sudo cp target/release/croaker /usr/local/bin/

# Create system configuration directory
sudo mkdir -p /etc/croaker

# Copy default prompt template
sudo cp config/default_prompt.txt /etc/croaker/prompts/default.txt

# Set proper permissions (readable by all users, but API key stays user-specific)
sudo chmod 755 /usr/local/bin/croaker
```

## Auto-Start Daemon

### systemd User Service (Recommended)

Create the user service directory and service file:

```bash
# Create systemd user directory
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

[Install]
WantedBy=default.target
EOF
```

Enable and start the service:

```bash
# Reload systemd daemon
systemctl --user daemon-reload

# Enable auto-start on login
systemctl --user enable croaker

# Start the service now
systemctl --user start croaker

# Check status
systemctl --user status croaker
```

### Desktop Environment Integration

For better integration with your desktop environment, you can also add croaker to your startup applications:

**GNOME/KDE:**
- System Settings → Startup Applications → Add croaker
- Command: `/usr/local/bin/croaker serve`

**Manual .desktop file:**
Create `~/.config/autostart/croaker.desktop`:

```ini
[Desktop Entry]
Type=Application
Name=croaker
Comment=Speech-to-text daemon
Exec=/usr/local/bin/croaker serve
Terminal=false
Categories=Utility;
```

### Stopping the Service

```bash
# Stop the service
systemctl --user stop croaker

# Disable auto-start
systemctl --user disable croaker
```

## Testing

### 1. Check Configuration

```bash
croaker configure
```

### 2. Start Daemon Manually

```bash
croaker serve
```

### 3. Test Hotkeys

- **Push-to-talk**: Hold RightAlt (or configured key) and speak
- **Toggle**: Press Super+Shift+R (or configured shortcut) to start, press again to stop

### 4. Test CLI Commands

In another terminal:

```bash
croaker toggle              # Toggle recording
croaker status              # Check status  
croaker cancel              # Cancel operation
croaker toggle-output-mode  # Toggle output mode (direct/clipboard/both) - default hotkey: Shift+RightAlt+O
croaker toggle-language     # Toggle language (cycles through configured languages) - default hotkey: Shift+RightAlt+L
```

### 5. Test Output Modes

Try different output modes to see how text is handled:

```bash
# Switch to clipboard-only mode
croaker toggle-output-mode  # Shows notification: "Output Mode: Clipboard"

# Switch to direct typing mode
croaker toggle-output-mode  # Shows notification: "Output Mode: Direct"

# Switch to both modes
croaker toggle-output-mode  # Shows notification: "Output Mode: Both"
```

### 6. Test Language Toggle

If you've configured multiple languages in `~/.config/croaker/config.toml`:

```bash
# Toggle to next language
croaker toggle-language  # Shows notification: "Language: ES" (or next language)

# Continue toggling cycles through all configured languages
croaker toggle-language  # Shows notification: "Language: FR"
```

The selected language will be used for the next transcription.

## Troubleshooting

### "Permission denied" accessing /dev/uinput

Make sure you're in the `input` group:

```bash
groups | grep input
```

If not listed, add yourself and log out/in:

```bash
sudo usermod -aG input $USER
```

### "Daemon is not running"

Start the daemon:

```bash
croaker serve
```

Or enable the systemd service:

```bash
systemctl --user start croaker
```

### Portal shortcuts not working

Portal shortcuts require:
- GNOME 45+
- KDE Plasma
- wlroots-based compositors (Hyprland, Sway)

For older compositors, use push-to-talk mode instead.

### Visual feedback not showing

If notifications aren't appearing, check that your desktop environment's notification daemon is running. You can configure the overlay backend in `~/.config/croaker/config.toml`:
- `"notification"`: Desktop notifications (default, works on all compositors)
- `"gtk"`: Floating pulsing dot indicator with audio level visualization
- `"layer-shell"`: Layer-shell overlay (wlroots compositors, requires feature flag)
- `"auto"`: Automatically selects the best available backend

### Keyboard Device Detection Issues

If you see `"Failed to start evdev monitor: No keyboard device found"` in the logs:

1. **Check Input Group Membership**:
   ```bash
   groups | grep input
   ```
   If not listed, add yourself and log out/in:
   ```bash
   sudo usermod -aG input $USER
   ```

2. **Verify Device Permissions**:
   ```bash
   ls -l /dev/input/event*
   ```
   Devices should be readable by the `input` group.

3. **Test Device Detection**:
   ```bash
   # Install evtest if needed
   sudo dnf install evtest

   # List available input devices
   sudo evtest --list

   # Test a specific device (replace X with device number)
   sudo evtest /dev/input/eventX
   ```

4. **Key Code Detection**: Different environments may map keys differently. If push-to-talk doesn't work:
   - Use `evtest` to find your desired key's code
   - Update `push_to_talk_key` in `~/.config/croaker/config.toml`
   - Supported keys: `RightAlt`, `LeftAlt`, `RightCtrl`, `LeftCtrl`

### Portal Shortcuts Not Working (Alternative)

If push-to-talk mode fails, portal shortcuts provide an alternative:

Portal shortcuts require:
- GNOME 45+
- KDE Plasma
- wlroots-based compositors (Hyprland, Sway)

For older compositors, use push-to-talk mode instead.

### GNOME-Specific Issues

**⚠️ Important**: GNOME has limitations that affect croaker:

1. **Text Insertion**: GNOME doesn't support the virtual keyboard protocol, so automatic text insertion may fail. The daemon will:
   - Try `wtype` first (will fail on GNOME)
   - Fall back to uinput Ctrl+V (may work, but unreliable)
   - If both fail, send a notification asking you to paste manually with Ctrl+V

   **Workaround**: When you see the "Text ready! Press Ctrl+V to paste" notification, manually paste with Ctrl+V. The text is already in your clipboard.

2. **Visual Feedback**: croaker shows visual feedback via desktop notifications, displaying the current state (Recording, Processing, Outputting). This works on all compositors (GNOME, KDE, Hyprland).

**KDE and Hyprland users**: Text insertion works automatically without manual intervention, as these compositors properly support the virtual keyboard protocol.

### Compilation Warnings

The project compiles with some warnings (unused imports, deprecated macros, unused variables). These are harmless and don't affect functionality. They're mostly cleanup items that can be addressed in future versions.


