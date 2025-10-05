# Translucent Click-Through Overlay for X11

A Rust application that creates a translucent, click-through overlay window on Linux (X11).

## Features

- **True Transparency**: Uses ARGB32 visual for per-pixel alpha
- **Click-Through**: All mouse and keyboard events pass through to windows below using X11 Shape extension with empty input region
- **Always on Top**: Stays above all other windows
- **No Window Decorations**: Uses override_redirect to avoid window manager interference
- **Toggle Hotkey**: Press Ctrl+Alt+O to show/hide the overlay
- **Text Rendering**: Display text on the overlay using X11 core fonts
- **Auto-sizing**: Overlay automatically sizes to 1/4 of screen (half width × half height)
- **Configurable**: Easy-to-use configuration API

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

## Controls

- **Ctrl+Alt+O**: Toggle overlay visibility
- **Ctrl+C**: Exit the application

## License

This is example code for educational purposes.
