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

**Optional (recommended for debugging hotkeys):** install `evtest`:

```bash
# Fedora
sudo dnf install evtest

# Ubuntu/Debian
sudo apt install evtest

# Arch
sudo pacman -S evtest
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
# Ensure DBUS is available for tray/notifications
Environment="DBUS_SESSION_BUS_ADDRESS=unix:path=%t/bus"

[Install]
WantedBy=default.target
EOF

# Note: Make sure the binary is installed at /usr/local/bin/croaker
# If you're using a different path, update ExecStart accordingly
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

**Verify the tray icon appears:**

After starting the service, you should see a **grey microphone icon** in your system tray. The daemon includes retry logic - if the tray service isn't ready yet when croaker starts, it will automatically retry connecting until it succeeds. The tray icon should appear within a few seconds.

If the tray icon doesn't appear:
- Check logs: `journalctl --user -u croaker | grep -i tray`
- Verify service is running: `systemctl --user is-active croaker`
- The daemon will continue running even if the tray fails - you can still use hotkeys, just without visual feedback

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

If the system tray icon isn't appearing:

1. **Check if service is running**: `systemctl --user is-active croaker`
2. **Check logs**: `journalctl --user -u croaker | grep -i tray`
   - You should see "System tray started" if the tray connected successfully
   - If you see errors, the daemon will automatically retry connecting to the tray
3. **Wait a few seconds** - If croaker started early in the login sequence, the tray service might not be ready yet. The daemon includes retry logic and will connect automatically once the tray is available.
4. **Configure overlay backend** in `~/.config/croaker/config.toml`:
   - `"tray"` (default): System tray icon that changes color based on state (grey=idle, red=recording, orange=processing, green=done)
   - `"notification"`: Desktop notifications

Note: Make sure your desktop environment supports the StatusNotifierItem protocol (most modern DEs like KDE, GNOME with extensions, XFCE do). The daemon will continue running even if the tray fails - you can still use hotkeys, just without visual feedback.

### Auto-start issues

**Service won't start on login?**
- Verify it's enabled: `systemctl --user is-enabled croaker` (should output: `enabled`)
- Check if systemd user services are running: `systemctl --user status`
- Some desktop environments require `loginctl enable-linger $USER` for user services to start on login
- Check logs: `journalctl --user -u croaker -n 50`

**Service won't start at all?**
- Make sure the binary exists: `ls -l /usr/local/bin/croaker`
- Verify you're using `--user` (not `sudo systemctl`)
- Check logs: `journalctl --user -u croaker -n 50`

**Tray icon doesn't appear after auto-start?**
- The daemon includes retry logic - it will keep trying to connect to the tray even if started early in the login sequence
- Check logs: `journalctl --user -u croaker | grep -i tray`
- The tray icon should appear within a few seconds of login
- The daemon will continue running even if the tray fails - you can still use hotkeys

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
   # Install evtest if needed (optional debugging tool)
   # Fedora: sudo dnf install evtest
   # Ubuntu/Debian: sudo apt install evtest
   # Arch: sudo pacman -S evtest

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

### Text Output Behavior

**Important**: croaker copies transcribed text to your clipboard. Automatic pasting is unreliable across all platforms, so you should paste manually with Ctrl+V.

**How it works:**
- Text is always copied to clipboard using `wl-copy` (Wayland clipboard utility)
- The daemon may attempt automatic pasting on some compositors, but this often fails due to security policies or compositor limitations
- **Recommended**: Use "clipboard" output mode and paste manually with Ctrl+V when the tray icon turns green

**Visual Feedback**: croaker shows visual feedback via system tray icon (default) or desktop notifications, displaying the current state through color changes (grey=idle, red=recording, orange=processing, green=done).

### Compilation Warnings

The project compiles with some warnings (unused imports, deprecated macros, unused variables). These are harmless and don't affect functionality. They're mostly cleanup items that can be addressed in future versions.


