#!/bin/bash
# Installation script for stealth overlay

set -e

REPO="AbhigyaKrishna/overlay-x11"
BINARY_NAME="overlay-x11"
INSTALL_NAME="stealth-overlay"
RAW_BASE="https://raw.githubusercontent.com/${REPO}/main"

echo "Fetching latest release information..."
LATEST_RELEASE=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest")
BINARY_URL=$(echo "$LATEST_RELEASE" | grep -oP '"browser_download_url":\s*"\K[^"]+' | grep "${BINARY_NAME}" | head -n1)
HOOK_URL=$(echo "$LATEST_RELEASE" | grep -oP '"browser_download_url":\s*"\K[^"]+' | grep "libstealth_hook.so" | head -n1)

if [ -z "$BINARY_URL" ]; then
    echo "Error: Could not find binary in latest release"
    echo "Please check https://github.com/${REPO}/releases"
    exit 1
fi

VERSION=$(echo "$LATEST_RELEASE" | grep -oP '"tag_name":\s*"\K[^"]+')
echo "Downloading ${BINARY_NAME} ${VERSION}..."

mkdir -p ~/.local/bin
mkdir -p ~/.local/lib

# Download main binary
curl -L -o ~/.local/bin/${INSTALL_NAME} "$BINARY_URL"
chmod +x ~/.local/bin/${INSTALL_NAME}
echo "✓ Binary installed to ~/.local/bin/${INSTALL_NAME}"

# Download stealth hook library if available
if [ -n "$HOOK_URL" ]; then
    curl -L -o ~/.local/lib/libstealth_hook.so "$HOOK_URL"
    chmod +x ~/.local/lib/libstealth_hook.so
    echo "✓ Stealth hook library installed to ~/.local/lib/libstealth_hook.so"
else
    echo "⚠ Stealth hook library not found in release - stealth features will be limited"
fi

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
echo "Stealth Features:"
if [ -f ~/.local/lib/libstealth_hook.so ]; then
    echo "  ✓ LD_PRELOAD hook library installed (advanced stealth enabled)"
    echo "  ✓ Window enumeration hiding"
    echo "  ✓ Screenshot capture prevention"
    echo "  ✓ Process name masquerading"
else
    echo "  ⚠ Basic stealth only (hook library not installed)"
fi
echo ""
echo "Hotkeys:"
echo "  Ctrl+Shift+E - Toggle overlay visibility"
echo "  Ctrl+Shift+B - Screenshot + AI analysis"
echo "  Arrow Keys   - Scroll content (when visible)"
echo ""
echo "Version: ${VERSION}"
