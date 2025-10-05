# Translucent Click-Through Overlay for X11

A Rust application that creates a translucent, click-through overlay window on Linux (X11) with **stealth mode** for undetectable operation.

## Features

- **True Transparency**: Uses ARGB32 visual for per-pixel alpha
- **Click-Through**: All mouse and keyboard events pass through to windows below using X11 Shape extension with empty input region
- **Always on Top**: Stays above all other windows
- **No Window Decorations**: Uses override_redirect to avoid window manager interference
- **Toggle Hotkey**: Press Ctrl+Alt+O to show/hide the overlay
- **Text Rendering**: Display text on the overlay using X11 core fonts
- **Auto-sizing**: Overlay automatically sizes based on screen dimensions
- **Configurable**: Easy-to-use configuration API
- **🔒 Stealth Mode**: Undetectable by window managers, taskbars, and system monitors

## Stealth Features

The overlay includes advanced stealth capabilities:

- **Process Masquerading**: Appears as `kworker/0:1` (kernel worker)
- **Window Manager Evasion**: No WM_CLASS, WM_NAME, or ICCCM properties
- **Desktop Type**: Disguised as `_NET_WM_WINDOW_TYPE_DESKTOP`
- **Panel Skipping**: Hidden from taskbar and pager (`_NET_WM_STATE_SKIP_TASKBAR`, `_NET_WM_STATE_SKIP_PAGER`)
- **Low Priority**: Runs at nice level 19 to avoid system monitor detection
- **Systemd Integration**: Runs as background service with user lingering

See [STEALTH.md](STEALTH.md) for complete documentation on stealth features.

## Quick Start

### Installation

```bash
chmod +x install.sh
./install.sh
```

This will:

- Build the overlay in release mode
- Install as `~/.local/bin/stealth-overlay`
- Set up systemd user service
- Enable auto-start on login
- Start the service immediately

### Manual Build

```bash
# Debug build (with logging)
cargo build
./target/debug/overlay-x11

# Release build (stealth mode)
cargo build --release
./target/release/overlay-x11
```

## Build Modes

**Debug Mode** (default):

- Verbose console logging
- Stealth features disabled
- Easy debugging and development
- Window visible in window manager

**Release Mode** (`--release`):

- Silent operation (no console output)
- Full stealth features enabled
- Process masquerading as kernel worker
- Complete window manager evasion
- Optimized performance

## How It Works

The click-through functionality is achieved by using the X11 Shape extension with an **empty input region**:

```rust
conn.shape_rectangles(
    SO::SET,
    SK::INPUT,
    ClipOrdering::UNSORTED,
    win,
    0,
    0,
    &[], // empty region = fully click-through
)?;
```

This tells X11 that the window has no input shape, making all pointer and keyboard events pass through to windows below.

## Project Structure

```
src/
├── main.rs      - Application entry point and window management
├── config.rs    - Configuration structure and builder
└── renderer.rs  - Rendering logic
```

## Configuration

The overlay can be configured using the `OverlayConfig` builder:

```rust
let config = OverlayConfig::new()
    .with_position(100, 100)    // X, Y position
    .with_size(800, 600)        // Width, Height
    .with_color(0x80FF0000);    // ARGB color

// Initialize renderer with optional font and text
let renderer = Renderer::new(config)
    .with_font(font_id)
    .with_text("Hello, Overlay!".to_string());
```

### Text Rendering

To render text on the overlay:

1. Open an X11 font:

```rust
let font_id = conn.generate_id()?;
conn.open_font(font_id, b"fixed")?;
```

2. Configure the renderer with font and text:

```rust
let renderer = Renderer::new(config)
    .with_font(font_id)
    .with_text("Your text here".to_string());
```

The text will be rendered in white at position (20, 40) on the overlay.

### Color Format

Colors are specified in ARGB format (32-bit hex):

- `0xAARRGGBB`
  - `AA` = Alpha (transparency): `00` = fully transparent, `FF` = fully opaque
  - `RR` = Red component
  - `GG` = Green component
  - `BB` = Blue component

**Examples:**

- `0x80FF0000` - 50% transparent red
- `0x4000FF00` - 25% transparent green
- `0xCC0000FF` - 80% transparent blue
- `0x60FFFF00` - 38% transparent yellow

## Usage

### Build and Run

```bash
cargo build
cargo run
```

The overlay will:

- Automatically size to 1/4 of your screen (half width × half height)
- Display screen and overlay dimensions as text
- Be togglable with **Ctrl+Alt+O**
- Allow all mouse and keyboard events to pass through

### Customize Configuration

Edit `src/main.rs` and modify the configuration:

```rust
let config = OverlayConfig::new()
    .with_position(0, 0)        // Top-left corner
    .with_size(1920, 1080)      // Full HD size
    .with_color(0x30000000);    // 19% transparent black
```

## Requirements

- Linux with X11
- Rust 1.70+
- X11 Shape extension (standard on most systems)
- systemd (for service mode)

## Service Management

### Start/Stop Service

```bash
# Start
systemctl --user start stealth-overlay.service

# Stop
systemctl --user stop stealth-overlay.service

# Restart
systemctl --user restart stealth-overlay.service

# Status
systemctl --user status stealth-overlay.service

# View logs
journalctl --user -u stealth-overlay.service -f
```

### Uninstall

```bash
systemctl --user stop stealth-overlay.service
systemctl --user disable stealth-overlay.service
rm ~/.local/bin/stealth-overlay
rm ~/.config/systemd/user/stealth-overlay.service
systemctl --user daemon-reload
```

## Controls

- **Ctrl+Alt+O**: Toggle overlay visibility
- **Ctrl+C**: Exit the application (manual mode only)

## Documentation

- [STEALTH.md](STEALTH.md) - Complete stealth features documentation
- [src/config.rs](src/config.rs) - Configuration options
- [src/renderer.rs](src/renderer.rs) - Rendering API

## License

This is example code for educational purposes.
