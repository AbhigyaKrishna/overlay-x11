#!/bin/bash
# Local installation script for stealth overlay (builds from source)

set -e

BINARY_NAME="overlay-x11"
INSTALL_NAME="stealth-overlay"

echo "=== Building Stealth Overlay from Source ==="
echo ""

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "Error: Cargo.toml not found. Please run this script from the project root."
    exit 1
fi

# Build main binary
echo "Building main binary..."
cargo build --release
echo "✓ Main binary built"

# Build stealth hook library
echo "Building stealth hook library..."
cd stealth_hook
cargo build --release
cd ..
echo "✓ Stealth hook library built"

# Create installation directories
echo ""
echo "Creating installation directories..."
mkdir -p ~/.local/bin
mkdir -p ~/.local/lib
mkdir -p ~/.config/stealth-overlay
mkdir -p ~/.config/systemd/user

# Install main binary
echo "Installing binary..."
cp target/release/${BINARY_NAME} ~/.local/bin/${INSTALL_NAME}
chmod +x ~/.local/bin/${INSTALL_NAME}
echo "✓ Binary installed to ~/.local/bin/${INSTALL_NAME}"

# Install stealth hook library
echo "Installing stealth hook library..."
cp stealth_hook/target/release/libstealth_hook.so ~/.local/lib/libstealth_hook.so
chmod +x ~/.local/lib/libstealth_hook.so
echo "✓ Stealth hook library installed to ~/.local/lib/libstealth_hook.so"

# Install configuration file
echo "Installing configuration..."
if [ -f "overlay.yml.example" ]; then
    if [ ! -f ~/.config/stealth-overlay/overlay.yml ]; then
        cp overlay.yml.example ~/.config/stealth-overlay/overlay.yml
        echo "✓ Config installed to ~/.config/stealth-overlay/overlay.yml"
    else
        echo "⚠ Config already exists at ~/.config/stealth-overlay/overlay.yml (not overwriting)"
    fi
else
    echo "⚠ overlay.yml.example not found - skipping config installation"
fi

# Install systemd service
echo "Installing systemd user service..."
cp stealth-overlay.service ~/.config/systemd/user/stealth-overlay.service
echo "✓ Systemd service installed"

# Reload systemd daemon
echo ""
echo "Reloading systemd daemon..."
systemctl --user daemon-reload

# Stop service if running
if systemctl --user is-active --quiet stealth-overlay.service; then
    echo "Stopping existing service..."
    systemctl --user stop stealth-overlay.service
fi

# Enable and start service
echo "Enabling and starting service..."
systemctl --user enable stealth-overlay.service
systemctl --user start stealth-overlay.service

# Enable user lingering
echo "Enabling user lingering..."
if command -v loginctl &> /dev/null; then
    sudo loginctl enable-linger $USER 2>/dev/null || echo "⚠ Could not enable lingering (may need sudo)"
else
    echo "⚠ loginctl not found - skipping lingering setup"
fi

echo ""
echo "✓ Installation complete!"
echo ""
echo "Service status:"
systemctl --user status stealth-overlay.service --no-pager || true
echo ""
echo "Commands:"
echo "  Start:   systemctl --user start stealth-overlay.service"
echo "  Stop:    systemctl --user stop stealth-overlay.service"
echo "  Restart: systemctl --user restart stealth-overlay.service"
echo "  Status:  systemctl --user status stealth-overlay.service"
echo "  Logs:    journalctl --user -u stealth-overlay.service -f"
echo ""
echo "Stealth Features:"
echo "  ✓ LD_PRELOAD hook library installed (advanced stealth enabled)"
echo "  ✓ Window enumeration hiding"
echo "  ✓ Screenshot capture prevention"
echo "  ✓ Process name masquerading"
echo ""
echo "Hotkeys:"
echo "  Ctrl+Shift+E - Toggle overlay visibility"
echo "  Ctrl+Shift+B - Screenshot + AI analysis"
echo "  Arrow Keys   - Scroll content (when visible)"
echo ""
echo "Configuration:"
echo "  Edit ~/.config/stealth-overlay/overlay.yml to customize settings"
echo ""
