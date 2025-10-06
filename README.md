# Translucent Click-Through Overlay for X11

A Rust application that creates a translucent, click-through overlay window on Linux (X11) with **stealth mode** for undetectable operation and **AI-powered screenshot analysis**.

## Features

- **True Transparency**: ARGB32 visual for per-pixel alpha
- **Click-Through**: All events pass through to windows below
- **Always on Top**: Stays above all other windows
- **Scrollable Content**: Navigate long text with arrow keys (Up/Down/Left/Right)
- **AI Screenshot Analysis**: Gemini-powered screen analysis
- **YAML Configuration**: Customizable colors, fonts, position, and size
- **Stealth Mode**: Undetectable by window managers and system monitors

## Quick Start

### Installation

```bash
chmod +x install.sh
./install.sh
```

This will build, install, and set up the overlay as a systemd service with auto-start on login.

### Configuration

Create a `overlay.yml` file:

```bash
cp overlay.yml.example overlay.yml
```

Edit the file to customize colors, position, size, font, and API key. See [CONFIG.md](CONFIG.md) for full documentation.

### Set Up Gemini API (Optional)

For AI screenshot analysis:

```bash
export GEMINI_API_KEY="your-api-key-here"
```

Or add `gemini_api_key` to your `overlay.yml`. Get your key from [Google AI Studio](https://makersuite.google.com/app/apikey).

## Usage

### Running

```bash
# Use default config (overlay.yml)
./overlay-x11

# Use custom config file
./overlay-x11 /path/to/config.yml
```

### Controls

- **Ctrl+Alt+E**: Toggle overlay visibility
- **Ctrl+Alt+S**: Take screenshot + AI analysis
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

**Release Mode** (production):

- Silent operation
- Full stealth enabled
- Process masquerading
- Window manager evasion

```bash
# Debug
cargo build

# Release
cargo build --release
```

## Requirements

- Linux with X11 (Linux Mint)
- systemd (for service mode)

## Uninstall

```bash
systemctl --user stop stealth-overlay.service
systemctl --user disable stealth-overlay.service
rm ~/.local/bin/stealth-overlay
rm ~/.config/systemd/user/stealth-overlay.service
systemctl --user daemon-reload
```

## License

Educational purposes.
