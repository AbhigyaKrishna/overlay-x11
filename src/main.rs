mod config;
mod renderer;

use std::error::Error;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::shape::ConnectionExt as _;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use config::OverlayConfig;
use renderer::Renderer;

// X11 keysym for 'O' key
const XK_O: u32 = 0x006f;

fn main() -> Result<(), Box<dyn Error>> {
    // Connect to the X server
    let (conn, screen_num) = RustConnection::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    // Get screen dimensions
    let screen_width = screen.width_in_pixels;
    let screen_height = screen.height_in_pixels;

    // Calculate overlay size as 1/4 of screen (half width, half height)
    let overlay_width = screen_width / 2;
    let overlay_height = screen_height / 2;

    // Create configuration with calculated dimensions
    let config = OverlayConfig::new()
        .with_position(100, 100)
        .with_size(overlay_width, overlay_height)
        .with_color(0x801c1c1c); // 50% transparent gray

    // Open a built-in X11 font
    let font_id = conn.generate_id()?;
    // Try to open a common fixed-width font, fallback to "fixed" if not available
    let font_name = b"-misc-fixed-medium-r-normal--13-120-75-75-C-70-iso8859-1";
    if conn.open_font(font_id, font_name).is_err() {
        // Fallback to the simple "fixed" font
        conn.open_font(font_id, b"fixed")?;
    }

    // Initialize renderer with font and text
    let renderer = Renderer::new(config.clone())
        .with_font(font_id)
        .with_text(format!(
            "Screen: {}x{}\nOverlay: {}x{} (1/4 screen)",
            screen_width, screen_height, overlay_width, overlay_height
        ));

    // Find a 32-bit (ARGB) visual for transparency
    let visual_id = screen
        .allowed_depths
        .iter()
        .find(|d| d.depth == 32)
        .and_then(|d| d.visuals.first())
        .map(|v| v.visual_id)
        .ok_or("No ARGB32 visual found")?;

    // Create a simple colormap for the ARGB visual
    let colormap = conn.generate_id()?;
    conn.create_colormap(ColormapAlloc::NONE, colormap, root, visual_id)?;

    // Create the overlay window
    let win = conn.generate_id()?;
    let cw_values = CreateWindowAux::new()
        .background_pixel(0) // fully transparent
        .border_pixel(0)
        .colormap(colormap)
        .override_redirect(1) // no window manager decoration, no focus
        .event_mask(EventMask::EXPOSURE | EventMask::KEY_PRESS);

    conn.create_window(
        32, // depth
        win,
        root,
        config.x,
        config.y,
        config.width,
        config.height,
        0, // border
        WindowClass::INPUT_OUTPUT,
        visual_id,
        &cw_values,
    )?;

    // Raise above all windows
    conn.configure_window(win, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE))?;

    // Make the window input-transparent via the Shape extension
    use x11rb::protocol::shape::{SK, SO};

    // Set an empty input shape to make the window click-through
    // Using shape_rectangles with an empty set makes the window fully click-through
    conn.shape_rectangles(
        SO::SET,
        SK::INPUT,
        ClipOrdering::UNSORTED,
        win,
        0,
        0,
        &[], // empty region = fully click-through
    )?;

    // Grab Ctrl+Alt+O on the root window
    let keycode_o = get_keycode(&conn, XK_O)?;
    let modifiers = ModMask::CONTROL | ModMask::M1; // M1 = Alt

    // Grab the key combination globally
    conn.grab_key(
        false,           // owner_events
        root,            // grab_window
        modifiers,       // modifiers
        keycode_o,       // key
        GrabMode::ASYNC, // pointer_mode
        GrabMode::ASYNC, // keyboard_mode
    )?;

    // Initial state: visible
    let mut visible = true;
    conn.map_window(win)?;
    conn.flush()?;

    println!("Overlay started. Press Ctrl+Alt+O to toggle visibility.");

    // Event loop
    loop {
        match conn.poll_for_event()? {
            Some(Event::Expose(_)) if visible => {
                // Use renderer to draw the overlay
                renderer.render(&conn, win)?;
            }
            Some(Event::KeyPress(k)) if k.detail == keycode_o => {
                // Check if the modifiers match (Ctrl+Alt)
                if k.state.contains(ModMask::CONTROL) && k.state.contains(ModMask::M1) {
                    // Toggle visibility
                    if visible {
                        conn.unmap_window(win)?;
                        println!("Overlay hidden");
                    } else {
                        conn.map_window(win)?;
                        println!("Overlay shown");
                    }
                    visible = !visible;
                    conn.flush()?;
                }
            }
            _ => {
                // Small sleep to avoid busy waiting
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    // Cleanup (this code is unreachable but good practice)
    // conn.close_font(font_id)?;
}

/// Convert a keysym to a keycode
fn get_keycode(conn: &RustConnection, keysym: u32) -> Result<Keycode, Box<dyn Error>> {
    let setup = conn.setup();
    let min_keycode = setup.min_keycode;
    let max_keycode = setup.max_keycode;

    let mapping = conn
        .get_keyboard_mapping(min_keycode, max_keycode - min_keycode + 1)?
        .reply()?;

    let keysyms_per_keycode = mapping.keysyms_per_keycode as usize;

    for (i, chunk) in mapping.keysyms.chunks(keysyms_per_keycode).enumerate() {
        if chunk.contains(&keysym) {
            return Ok(min_keycode + i as u8);
        }
    }

    Err(format!("Keysym 0x{:x} not found", keysym).into())
}
