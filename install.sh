#!/bin/bash
# Installation script for stealth overlay

set -e

echo "Installing binary..."
mkdir -p ~/.local/bin
cp target/release/overlay-x11 ~/.local/bin/stealth-overlay
chmod +x ~/.local/bin/stealth-overlay

echo "Installing systemd user service..."
mkdir -p ~/.config/systemd/user
cp stealth-overlay.service ~/.config/systemd/user/

echo "Reloading systemd daemon..."
systemctl --user daemon-reload

echo "Enabling and starting service..."
systemctl --user enable stealth-overlay.service
systemctl --user start stealth-overlay.service

echo "Enabling user lingering..."
sudo loginctl enable-linger $USER

echo ""
echo "✓ Installation complete!"
echo ""
echo "Service status:"
systemctl --user status stealth-overlay.service --no-pager
echo ""
echo "Commands:"
echo "  Start:   systemctl --user start stealth-overlay.service"
echo "  Stop:    systemctl --user stop stealth-overlay.service"
echo "  Status:  systemctl --user status stealth-overlay.service"
echo "  Logs:    journalctl --user -u stealth-overlay.service -f"
echo ""
echo "Hotkey: Ctrl+Alt+O to toggle overlay visibility"
