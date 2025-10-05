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

fn main() -> Result<(), Box<dyn Error>> {
    // Create configuration
    let config = OverlayConfig::new()
        .with_position(100, 100)
        .with_size(800, 600)
        .with_color(0x80FF0000); // 50% transparent red

    // Initialize renderer
    let renderer = Renderer::new(config.clone());

    // Connect to the X server
    let (conn, screen_num) = RustConnection::connect(None)?;
    let screen = &conn.setup().roots[screen_num];

    // Find a 32-bit (ARGB) visual for transparency
    let mut argb_visual = None;
    for depth in &screen.allowed_depths {
        if depth.depth == 32 {
            for v in &depth.visuals {
                argb_visual = Some(v.visual_id);
                break;
            }
        }
    }
    let visual_id = argb_visual.ok_or("No ARGB32 visual found")?;

    // Create a simple colormap for the ARGB visual
    let colormap = conn.generate_id()?;
    conn.create_colormap(ColormapAlloc::NONE, colormap, screen.root, visual_id)?;

    // Create the overlay window
    let win = conn.generate_id()?;
    let cw_values = CreateWindowAux::new()
        .background_pixel(0) // fully transparent
        .border_pixel(0)
        .colormap(colormap)
        .override_redirect(1) // no window manager decoration, no focus
        .event_mask(EventMask::EXPOSURE);

    conn.create_window(
        32, // depth
        win,
        screen.root,
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

    // Map (show) the window
    conn.map_window(win)?;
    conn.flush()?;

    // Simple loop to repaint semi‐transparent overlay
    loop {
        // Poll for events with timeout
        match conn.poll_for_event()? {
            Some(Event::Expose(_)) => {
                // Use renderer to draw the overlay
                renderer.render(&conn, win)?;
            }
            _ => {
                // Small sleep to avoid busy waiting
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}
