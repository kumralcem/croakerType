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

### 5. Install (Optional)

```bash
# Copy to a location in your PATH
sudo cp target/release/croaker /usr/local/bin/
```

## Auto-Start Daemon

### systemd User Service

Create `~/.config/systemd/user/croaker.service`:

```ini
[Unit]
Description=croaker speech-to-text daemon
After=graphical-session.target

[Service]
Type=simple
ExecStart=/usr/local/bin/croaker serve
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
```

Enable and start:

```bash
systemctl --user enable croaker
systemctl --user start croaker
```

Check status:

```bash
systemctl --user status croaker
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
croaker toggle    # Toggle recording
croaker status    # Check status  
croaker cancel    # Cancel operation
```

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


