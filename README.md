# Translucent Click-Through Overlay for X11

A Rust application that creates a translucent, click-through overlay window on Linux (X11) with **stealth mode** for undetectable operation and **AI-powered screenshot analysis**.

## Features

- **True Transparency**: ARGB32 visual for per-pixel alpha
- **Click-Through**: All events pass through to windows below
- **Always on Top**: Stays above all other windows
- **Scrollable Content**: Navigate long text with arrow keys (Up/Down/Left/Right)
- **AI Screenshot Analysis**: Gemini-powered screen analysis
- **YAML Configuration**: Customizable colors, fonts, position, and size
- **Advanced Stealth Mode**: Multi-layer undetectability
  - LD_PRELOAD X11 API hooking
  - Process name masquerading
  - Window enumeration hiding
  - Screenshot capture prevention
  - Memory protection

## Installation

### Automated Installation (Recommended)

Download and run the installation script:

```bash
curl -fsSL https://raw.githubusercontent.com/AbhigyaKrishna/overlay-x11/main/install.sh | sh
```

This will:

- Download the latest release binary from GitHub
- Install to `~/.local/bin/stealth-overlay`
- Download default configuration to `~/.config/stealth-overlay/overlay.yml`
- Install and enable systemd service with auto-start on login
- Configure user lingering for persistent service

### Manual Installation

1. Download the latest release binary:

   ```bash
   # Get latest version
   curl -s https://api.github.com/repos/AbhigyaKrishna/overlay-x11/releases/latest \
     | grep browser_download_url | cut -d '"' -f 4 | xargs curl -L -o overlay-x11

   # Install
   mkdir -p ~/.local/bin
   mv overlay-x11 ~/.local/bin/stealth-overlay
   chmod +x ~/.local/bin/stealth-overlay
   ```

2. Download configuration:

   ```bash
   mkdir -p ~/.config/stealth-overlay
   curl -L -o ~/.config/stealth-overlay/overlay.yml \
     https://raw.githubusercontent.com/AbhigyaKrishna/overlay-x11/main/overlay.yml.example
   ```

3. (Optional) Install systemd service:
   ```bash
   mkdir -p ~/.config/systemd/user
   curl -L -o ~/.config/systemd/user/stealth-overlay.service \
     https://raw.githubusercontent.com/AbhigyaKrishna/overlay-x11/main/stealth-overlay.service
   systemctl --user daemon-reload
   systemctl --user enable --now stealth-overlay.service
   ```

### Build from Source

#### Quick Build (Standard)

```bash
git clone https://github.com/AbhigyaKrishna/overlay-x11.git
cd overlay-x11
cargo build --release
cp target/release/overlay-x11 ~/.local/bin/stealth-overlay
```

#### Full Stealth Build (Recommended)

Build with LD_PRELOAD hook library for maximum stealth:

```bash
git clone https://github.com/AbhigyaKrishna/overlay-x11.git
cd overlay-x11
./build_stealth.sh
```

This builds:

- Main overlay application
- LD_PRELOAD hook library (`libstealth_hook.so`)

Then install:

```bash
# Install binaries
cp target/release/overlay-x11 ~/.local/bin/stealth-overlay
mkdir -p ~/.local/lib
cp libstealth_hook.so ~/.local/lib/

# Install service (will use LD_PRELOAD automatically)
./install.sh
```

## Configuration

The configuration file is located at `~/.config/stealth-overlay/overlay.yml`.

Edit this file to customize colors, position, size, font, and API key. See [CONFIG.md](CONFIG.md) for full documentation.

### Set Up Gemini API (Optional)

For AI screenshot analysis, add your API key to the configuration file:

```bash
# Edit config
nano ~/.config/stealth-overlay/overlay.yml

# Add or uncomment:
# gemini_api_key: "your-api-key-here"
```

Alternatively, set as an environment variable:

```bash
export GEMINI_API_KEY="your-api-key-here"
```

Get your API key from [Google AI Studio](https://makersuite.google.com/app/apikey).

## Usage

### Running

If using the systemd service (recommended), the overlay starts automatically on login with full stealth enabled.

To run manually:

```bash
# Run with default config (~/.config/stealth-overlay/overlay.yml)
stealth-overlay

# With full stealth (LD_PRELOAD hook)
LD_PRELOAD=~/.local/lib/libstealth_hook.so stealth-overlay

# Use custom config file
stealth-overlay /path/to/config.yml
```

**Note**: Full stealth requires the LD_PRELOAD hook library. The systemd service automatically loads it.

### Controls

- **Ctrl+Shift+E**: Toggle overlay visibility
- **Ctrl+Shift+B**: Take screenshot + AI analysis
- **Arrow Keys**: Scroll content (when overlay is visible)
  - Up/Down: Vertical scrolling
  - Left/Right: Horizontal scrolling

### Service Management

```bash
# Start/Stop
systemctl --user start stealth-overlay.service
systemctl --user stop stealth-overlay.service

# Status
systemctl --user status stealth-overlay.service

# View logs
journalctl --user -u stealth-overlay.service -f
```

## Configuration

See [CONFIG.md](CONFIG.md) for complete configuration guide including:

- Color formats (ARGB/RGB)
- Font selection
- Position and sizing
- API key configuration

Example `overlay.yml`:

```yaml
x: 100 # Auto-centers if left at default
y: 100
width: 800 # Auto-sizes to 2/3 screen if left at default
height: 600
color: 0x80000000 # 50% transparent black
text_color: 0xFFFFFF
text_outline_color: 0x000000
font: "-misc-fixed-medium-r-normal--20-200-75-75-C-100-iso8859-1"
```

## Build Modes

**Debug Mode** (development):

- Verbose console logging
- Stealth features disabled
- Overlay visible on startup
- Process runs under real name

**Release Mode** (production):

- Silent operation
- Full stealth enabled
- Process masquerading as system service
- Window manager evasion
- LD_PRELOAD API hooking (if library loaded)
- Overlay hidden on startup

```bash
# Debug
cargo build

# Release
cargo build --release

# Full stealth release
./build_stealth.sh
```

## Stealth Features

The overlay implements multiple layers of stealth for undetectability:

### User-Level Stealth (No Root Required)

1. **LD_PRELOAD API Hooking** - Intercepts X11 functions to hide window from:

   - Window enumeration (`xwininfo`, `wmctrl`)
   - Property queries
   - Screenshot tools
   - Pointer tracking

2. **Process Masquerading** - Appears as system services like:

   - `pipewire`
   - `dbus-daemon`
   - `pulseaudio`

3. **Window Manager Evasion**:

   - `override_redirect` flag
   - Desktop window type
   - Skip taskbar/pager states
   - Empty input shape (click-through)

4. **Memory Protection**:
   - Core dumps disabled
   - Memory locking
   - File descriptor obfuscation

### Verification

Test stealth effectiveness:

```bash
# Window should not appear in lists
wmctrl -l | grep overlay     # No results
xwininfo -root -tree         # Window not listed

# Process appears as system service
ps aux | grep overlay        # Shows masqueraded name

# Check stealth status (debug mode)
./target/debug/overlay-x11   # Prints stealth status
```

For detailed stealth documentation, see [docs/STEALTH.md](docs/STEALTH.md).

For stealth testing guide, see [docs/TESTING_STEALTH.md](docs/TESTING_STEALTH.md).

## Requirements

- Linux with X11 (Linux Mint)
- systemd (for service mode)

## Uninstall

```bash
# Stop and disable service
systemctl --user stop stealth-overlay.service
systemctl --user disable stealth-overlay.service

# Remove files
rm ~/.local/bin/stealth-overlay
rm ~/.local/lib/libstealth_hook.so
rm -rf ~/.config/stealth-overlay
rm ~/.config/systemd/user/stealth-overlay.service

# Reload systemd
systemctl --user daemon-reload
```

## Documentation

- [Configuration Guide](CONFIG.md) - Complete configuration reference
- [Stealth Implementation](docs/STEALTH.md) - Advanced stealth techniques
- [Testing Stealth](docs/TESTING_STEALTH.md) - Verification and testing guide

## Architecture

```
User Applications (wmctrl, xwininfo, screenshot tools)
          ↓
LD_PRELOAD Hook Library (libstealth_hook.so)
  • Intercepts X11 API calls
  • Filters out stealth windows
  • Prevents screenshot capture
          ↓
Standard Xlib/XCB
          ↓
X11 Server
```

## Security Note

This tool provides **user-level stealth** and cannot hide from:

- Kernel-level monitoring (eBPF, kernel modules)
- Direct framebuffer access (DRM/KMS)
- Hardware screen capture
- Root-level process inspection

For details on limitations and bypasses, see [docs/STEALTH.md](docs/STEALTH.md).

## License

Educational purposes.
