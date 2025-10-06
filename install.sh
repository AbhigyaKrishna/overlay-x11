#!/bin/bash
# Installation script for stealth overlay

set -e

REPO="AbhigyaKrishna/overlay-x11"
BINARY_NAME="overlay-x11"
INSTALL_NAME="stealth-overlay"
RAW_BASE="https://raw.githubusercontent.com/${REPO}/main"

echo "Fetching latest release information..."
LATEST_RELEASE=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest")
DOWNLOAD_URL=$(echo "$LATEST_RELEASE" | grep -oP '"browser_download_url":\s*"\K[^"]+' | grep "${BINARY_NAME}" | head -n1)

if [ -z "$DOWNLOAD_URL" ]; then
    echo "Error: Could not find binary in latest release"
    echo "Please check https://github.com/${REPO}/releases"
    exit 1
fi

VERSION=$(echo "$LATEST_RELEASE" | grep -oP '"tag_name":\s*"\K[^"]+')
echo "Downloading ${BINARY_NAME} ${VERSION}..."

mkdir -p ~/.local/bin
curl -L -o ~/.local/bin/${INSTALL_NAME} "$DOWNLOAD_URL"
chmod +x ~/.local/bin/${INSTALL_NAME}

echo "✓ Binary installed to ~/.local/bin/${INSTALL_NAME}"

echo "Downloading configuration files..."
mkdir -p ~/.config/stealth-overlay
curl -L -o ~/.config/stealth-overlay/overlay.yml "${RAW_BASE}/overlay.yml.example"
echo "✓ Config downloaded to ~/.config/stealth-overlay/overlay.yml"

echo "Installing systemd user service..."
mkdir -p ~/.config/systemd/user
curl -L -o ~/.config/systemd/user/stealth-overlay.service "${RAW_BASE}/stealth-overlay.service"
echo "✓ Systemd service installed"

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
echo "Hotkeys:"
echo "  Ctrl+Alt+E - Toggle overlay visibility"
echo "  Ctrl+Alt+S - Screenshot + AI analysis"
echo "  Arrow Keys - Scroll content (when visible)"
echo ""
echo "Version: ${VERSION}"
