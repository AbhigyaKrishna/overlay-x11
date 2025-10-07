mod config;
mod evdev_monitor;
mod gemini;
mod modifier_mapper;
mod renderer;
mod shortcut_tracker;

use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::shape::ConnectionExt as _;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

use config::OverlayConfig;
use evdev_monitor::EvdevMonitor;
use modifier_mapper::ModifierMapper;
use renderer::Renderer;
use shortcut_tracker::ShortcutTracker;

// Add channel support for background processing
use crossbeam_channel::{Receiver, Sender, unbounded};

// X11 keysyms
const XK_E: u32 = 0x0065; // 'E' key
const XK_Q: u32 = 0x0071; // 'Q' key
const XK_UP: u32 = 0xff52; // Up arrow
const XK_DOWN: u32 = 0xff54; // Down arrow
const XK_LEFT: u32 = 0xff51; // Left arrow
const XK_RIGHT: u32 = 0xff53; // Right arrow

// Structure to hold AI response data
#[derive(Debug, Clone)]
pub struct AiResponse {
    pub content: String,
    pub timestamp: std::time::Instant,
}

fn main() -> Result<(), Box<dyn Error>> {
    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let config_path = args.get(1).cloned();

    // Load configuration from file or use defaults
    let mut config = OverlayConfig::load(config_path);

    #[cfg(not(debug_assertions))]
    setup_process_stealth()?;
    // Connect to the X server
    let (conn, screen_num) = RustConnection::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    // Get screen dimensions
    let screen_width = screen.width_in_pixels;
    let screen_height = screen.height_in_pixels;

    // If width/height are still at defaults, calculate as 2/3 of screen
    if config.width == 800 && config.height == 600 {
        config.width = screen_width * 2 / 3;
        config.height = screen_height * 2 / 3;
    }

    // If position is at defaults (100, 100), center the overlay on screen
    if config.x == 100 && config.y == 100 {
        config.x = ((screen_width - config.width) / 2) as i16;
        config.y = ((screen_height - config.height) / 2) as i16;
    }

    // Open X11 font from config
    let font_id = conn.generate_id()?;
    let font_bytes = config.font.as_bytes();
    if conn.open_font(font_id, font_bytes).is_err() {
        let fallback = b"-misc-fixed-medium-r-normal--15-140-75-75-C-90-iso8859-1";
        if conn.open_font(font_id, fallback).is_err() {
            conn.open_font(font_id, b"fixed")?;
        }
    }

    // Query font metrics for proper line spacing
    let font_info = conn.query_font(font_id)?.reply()?;
    let font_ascent = font_info.font_ascent as u16;
    let font_descent = font_info.font_descent as u16;

    let initial_text = (1..=50)
        .map(|i| {
            format!(
                "Line #{:03} - Screen: {}x{}, Overlay: {}x{} at ({}, {})",
                i, screen_width, screen_height, config.width, config.height, config.x, config.y
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut renderer = Renderer::new(config.clone())
        .with_font(font_id, font_ascent, font_descent)
        .with_text(initial_text)
        .with_scroll_offset(0);

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

    // Make completely undetectable by window manager
    #[cfg(not(debug_assertions))]
    hide_from_window_manager(&conn, win)?;

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

    // Initialize modifier mapper for dynamic modifier detection
    let mut modifier_mapper = ModifierMapper::new(&conn)?;

    #[cfg(debug_assertions)]
    println!("Debug: ModifierMapper initialized");

    // Use evdev monitoring for system-level stealth (no grabbing)
    let evdev_monitor = match EvdevMonitor::new() {
        Ok(monitor) => Some(monitor),
        Err(e) => {
            #[cfg(debug_assertions)]
            {
                eprintln!(
                    "Debug: Evdev monitoring unavailable: {}. This overlay requires evdev access.",
                    e
                );
                eprintln!("Please ensure you have permission to access /dev/input/event* devices.");
                eprintln!("You may need to add your user to the 'input' group:");
                eprintln!("  sudo usermod -a -G input $USER");
            }
            return Err("Evdev monitoring required but unavailable".into());
        }
    };

    // Get keycodes for our hotkeys
    let keycode_e = modifier_mapper.get_keycode(XK_E).ok_or("E key not found")?;
    let keycode_q = modifier_mapper.get_keycode(XK_Q).ok_or("Q key not found")?;
    let keycode_up = modifier_mapper
        .get_keycode(XK_UP)
        .ok_or("Up key not found")?;
    let keycode_down = modifier_mapper
        .get_keycode(XK_DOWN)
        .ok_or("Down key not found")?;
    let keycode_left = modifier_mapper
        .get_keycode(XK_LEFT)
        .ok_or("Left key not found")?;
    let keycode_right = modifier_mapper
        .get_keycode(XK_RIGHT)
        .ok_or("Right key not found")?;

    // Track key states and shortcuts with unified tracker
    let mut shortcut_tracker = ShortcutTracker::new();
    shortcut_tracker.update_keycodes(&modifier_mapper);

    // Add periodic cleanup timer
    let mut last_cleanup = std::time::Instant::now();

    // Create channel for AI responses
    let (ai_sender, ai_receiver): (Sender<AiResponse>, Receiver<AiResponse>) = unbounded();

    // Track screenshot processing state to prevent concurrent requests
    let mut screenshot_processing = false;

    // Track loading state for UI feedback
    let mut loading_message = String::new();
    let mut loading_start_time: Option<std::time::Instant> = None;
    let mut last_loading_update = std::time::Instant::now();

    // Track current processing thread to enable interruption
    let mut current_cancel_flag: Option<Arc<AtomicBool>> = None;

    // Initial state: visible
    let mut visible = true;
    conn.map_window(win)?;
    conn.flush()?;

    #[cfg(debug_assertions)]
    {
        println!("=== OVERLAY CONTROLS ===");
        println!("Toggle Overlay: Hold Ctrl + Shift, then press E");
        println!("Screenshot + AI: Hold Ctrl + Shift, then press Q");
        println!("When overlay is visible: Use arrow keys to scroll");
        println!("========================");
    }

    // Event loop - handle both XInput2 raw events and evdev events
    loop {
        // Periodic cleanup to prevent stuck modifier states (every 5 seconds)
        if last_cleanup.elapsed() > Duration::from_secs(5) {
            shortcut_tracker.cleanup_stale_keys();
            shortcut_tracker.reset_modifier_states();
            last_cleanup = std::time::Instant::now();
        }

        // Check for AI responses (non-blocking)
        if let Ok(response) = ai_receiver.try_recv() {
            // Only process if this response isn't from an interrupted request
            let should_process = current_cancel_flag
                .as_ref()
                .map_or(true, |flag| !flag.load(Ordering::SeqCst));

            if should_process {
                let current_offset = renderer.scroll_offset();
                renderer = Renderer::new(config.clone())
                    .with_font(font_id, font_ascent, font_descent)
                    .with_text(format!("[AI] Screenshot Analysis:\n\n{}", response.content))
                    .with_scroll_offset(current_offset);

                // Clear loading state
                loading_message.clear();
                loading_start_time = None;
                screenshot_processing = false;
                current_cancel_flag = None;

                // Refresh display if visible
                if visible {
                    conn.clear_area(false, win, 0, 0, config.width, config.height)?;
                    renderer.render(&conn, win)?;
                    conn.flush()?;
                }
            } else {
                // Response from interrupted request - discard it
                #[cfg(debug_assertions)]
                println!("[DISCARDED] Response from interrupted AI request");
                current_cancel_flag = None;
            }
        }

        // Update loading animation if processing (every 500ms)
        if screenshot_processing && last_loading_update.elapsed() > Duration::from_millis(500) {
            if let Some(start_time) = loading_start_time {
                let elapsed = start_time.elapsed().as_secs();
                let dots = ".".repeat(((elapsed % 4) + 1) as usize);
                loading_message = format!(
                    "[AI] Processing screenshot{}\\n\\nThis may take a few moments...",
                    dots
                );

                // Update display with loading message
                let current_offset = renderer.scroll_offset();
                let temp_renderer = Renderer::new(config.clone())
                    .with_font(font_id, font_ascent, font_descent)
                    .with_text(loading_message.clone())
                    .with_scroll_offset(current_offset);

                if visible {
                    conn.clear_area(false, win, 0, 0, config.width, config.height)?;
                    temp_renderer.render(&conn, win)?;
                    conn.flush()?;
                }

                last_loading_update = std::time::Instant::now();
            }
        }

        // Handle evdev events if available
        if let Some(ref evdev) = evdev_monitor {
            while let Some(ev) = evdev.try_recv() {
                let x11_keycode = evdev_monitor::evdev_to_x11_keycode(ev.keycode);
                if ev.pressed {
                    shortcut_tracker.key_pressed(x11_keycode);
                } else {
                    shortcut_tracker.key_released(x11_keycode);
                }

                // Check for hotkey combinations
                if handle_key_event(
                    x11_keycode,
                    ev.pressed,
                    &mut shortcut_tracker,
                    keycode_e,
                    keycode_q,
                    keycode_up,
                    keycode_down,
                    keycode_left,
                    keycode_right,
                    &mut visible,
                    &conn,
                    win,
                    &config,
                    &mut renderer,
                    font_id,
                    font_ascent,
                    font_descent,
                    root,
                    screen_width,
                    screen_height,
                    &mut screenshot_processing,
                    &ai_sender,
                    &mut loading_message,
                    &mut loading_start_time,
                    &mut current_cancel_flag,
                )? {
                    // Shortcut was handled, continue
                }
            }
        }

        // Handle X11 events
        match conn.poll_for_event()? {
            Some(Event::Expose(_)) if visible => {
                // Use renderer to draw the overlay
                renderer.render(&conn, win)?;
            }
            Some(Event::MappingNotify(_)) => {
                modifier_mapper.refresh(&conn)?;
            }
            _ => {
                // Small sleep to avoid busy waiting
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

/// Handle key events (both XInput2 and evdev) - returns true if shortcut was handled
#[allow(clippy::too_many_arguments)]
fn handle_key_event(
    keycode: u8,
    pressed: bool,
    shortcut_tracker: &mut ShortcutTracker,
    keycode_e: u8,
    keycode_q: u8,
    keycode_up: u8,
    keycode_down: u8,
    keycode_left: u8,
    keycode_right: u8,
    visible: &mut bool,
    conn: &RustConnection,
    win: Window,
    config: &OverlayConfig,
    renderer: &mut Renderer,
    font_id: Font,
    font_ascent: u16,
    font_descent: u16,
    root: Window,
    screen_width: u16,
    screen_height: u16,
    screenshot_processing: &mut bool,
    ai_sender: &Sender<AiResponse>,
    loading_message: &mut String,
    loading_start_time: &mut Option<std::time::Instant>,
    current_cancel_flag: &mut Option<Arc<AtomicBool>>,
) -> Result<bool, Box<dyn Error>> {
    // Only process shortcut combinations on key press events
    if !pressed {
        if keycode == shortcut_tracker.ctrl_keycode().unwrap_or(0)
            || keycode == shortcut_tracker.shift_keycode().unwrap_or(0)
            || keycode == 37
            || keycode == 105
            || keycode == 50
            || keycode == 62
        {
            shortcut_tracker.reset_modifier_states();
        }
        return Ok(false);
    }

    let pressed_keys = shortcut_tracker.get_pressed_keys();

    // Robust validation: Reset if too many keys detected (prevents stuck states)
    if pressed_keys.len() > 6 {
        shortcut_tracker.reset_modifier_states();
        shortcut_tracker.clear_all_keys();
        return Ok(false);
    }

    if pressed_keys.len() > 1 {
        let mut detected_mods = Vec::new();

        // Check what modifiers are detected
        if shortcut_tracker
            .ctrl_keycode()
            .map_or(false, |k| pressed_keys.contains(&k))
        {
            detected_mods.push("Ctrl");
        }

        if shortcut_tracker
            .shift_keycode()
            .map_or(false, |k| pressed_keys.contains(&k))
            || pressed_keys.contains(&50)
            || pressed_keys.contains(&62)
        {
            detected_mods.push("Shift");
        }
    }

    // Check for Ctrl+Shift+E (toggle overlay)
    if shortcut_tracker.check_ctrl_shift_e(keycode_e) {
        shortcut_tracker.reset_modifier_states();

        if *visible {
            conn.unmap_window(win)?;
        } else {
            conn.map_window(win)?;
        }
        *visible = !*visible;
        conn.flush()?;
        return Ok(true);
    }

    // Check for Ctrl+Shift+Q (screenshot) - IMPROVED VERSION with background processing
    if shortcut_tracker.check_ctrl_shift_q(keycode_q) {
        // If already processing, interrupt the previous request
        if *screenshot_processing {
            if let Some(cancel_flag) = current_cancel_flag.as_ref() {
                cancel_flag.store(true, Ordering::SeqCst);
                #[cfg(debug_assertions)]
                println!("[INTERRUPT] Canceling previous AI request...");
            }
            *current_cancel_flag = None;
            *screenshot_processing = false;
        }

        // Reset states immediately after detection
        shortcut_tracker.reset_modifier_states();

        // Step 1: Check API key before proceeding
        if let Err(e) = gemini::get_api_key(config.gemini_api_key.clone()) {
            // Show API key error on overlay immediately
            *screenshot_processing = false;
            let error_message = format!(
                "[ERROR] API Key Issue\n\n{}\n\nPlease set GEMINI_API_KEY environment variable or add it to overlay.yml",
                e
            );

            let current_offset = renderer.scroll_offset();
            *renderer = Renderer::new(config.clone())
                .with_font(font_id, font_ascent, font_descent)
                .with_text(error_message)
                .with_scroll_offset(current_offset);

            if *visible {
                conn.clear_area(false, win, 0, 0, config.width, config.height)?;
                renderer.render(conn, win)?;
                conn.flush()?;
            }
            return Ok(true);
        }

        // Step 2: Hide overlay immediately
        if *visible {
            conn.unmap_window(win)?;
            conn.flush()?;
            std::thread::sleep(Duration::from_millis(100)); // Brief delay for screen to update
        }

        // Step 3: Capture screenshot
        match capture_screenshot(conn, root, screen_width, screen_height) {
            Ok(png_data) => {
                // Step 4: Show overlay back immediately with loading message
                *screenshot_processing = true;
                *loading_start_time = Some(std::time::Instant::now());
                *loading_message =
                    "[AI] Processing screenshot.\n\nThis may take a few moments...".to_string();

                // Update renderer with loading message
                let current_offset = renderer.scroll_offset();
                *renderer = Renderer::new(config.clone())
                    .with_font(font_id, font_ascent, font_descent)
                    .with_text(loading_message.clone())
                    .with_scroll_offset(current_offset);

                if *visible {
                    conn.map_window(win)?;
                    conn.clear_area(false, win, 0, 0, config.width, config.height)?;
                    renderer.render(conn, win)?;
                    conn.flush()?;
                }

                // Step 5: Create cancellation flag for this request
                let cancel_flag = Arc::new(AtomicBool::new(false));
                *current_cancel_flag = Some(cancel_flag.clone());

                // Step 6: Start background AI processing
                let ai_sender_clone = ai_sender.clone();
                let config_clone = config.clone();
                std::thread::spawn(move || {
                    match process_screenshot_async(png_data, config_clone, cancel_flag) {
                        Ok(analysis) => {
                            let response = AiResponse {
                                content: analysis,
                                timestamp: std::time::Instant::now(),
                            };
                            if let Err(e) = ai_sender_clone.send(response) {
                                #[cfg(debug_assertions)]
                                eprintln!("[ERROR] Failed to send AI response: {}", e);
                            }
                        }
                        Err(e) => {
                            let error_response = AiResponse {
                                content: format!("Error processing screenshot: {}", e),
                                timestamp: std::time::Instant::now(),
                            };
                            if let Err(send_err) = ai_sender_clone.send(error_response) {
                                #[cfg(debug_assertions)]
                                eprintln!("[ERROR] Failed to send error response: {}", send_err);
                            }
                        }
                    }
                });
            }
            Err(e) => {
                #[cfg(debug_assertions)]
                println!("[ERROR] Screenshot capture failed: {}", e);
                // Restore overlay even if screenshot failed
                if *visible {
                    conn.map_window(win)?;
                    conn.flush()?;
                }
            }
        }

        return Ok(true);
    }

    // Handle arrow keys (only when visible)
    if *visible {
        if keycode == keycode_up {
            renderer.scroll_up();
            conn.clear_area(false, win, 0, 0, config.width, config.height)?;
            renderer.render(conn, win)?;
            conn.flush()?;
            return Ok(true);
        } else if keycode == keycode_down {
            renderer.scroll_down();
            conn.clear_area(false, win, 0, 0, config.width, config.height)?;
            renderer.render(conn, win)?;
            conn.flush()?;
            return Ok(true);
        } else if keycode == keycode_left {
            renderer.scroll_left();
            conn.clear_area(false, win, 0, 0, config.width, config.height)?;
            renderer.render(conn, win)?;
            conn.flush()?;
            return Ok(true);
        } else if keycode == keycode_right {
            renderer.scroll_right();
            conn.clear_area(false, win, 0, 0, config.width, config.height)?;
            renderer.render(conn, win)?;
            conn.flush()?;
            return Ok(true);
        }
    }

    Ok(false)
}

/// Capture the root window via GetImage and return PNG data
fn capture_screenshot(
    conn: &RustConnection,
    root: Window,
    width: u16,
    height: u16,
) -> Result<Vec<u8>, Box<dyn Error>> {
    // Request the full screen image in ZPixmap format
    let img = conn
        .get_image(ImageFormat::Z_PIXMAP, root, 0, 0, width, height, !0)?
        .reply()?;
    let data = img.data;

    // Encode to PNG in memory
    let mut png_data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_data, width.into(), height.into());
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;

        // Convert X11 pixel data to RGB
        let mut rgb_buf = Vec::with_capacity((width as usize) * (height as usize) * 3);
        for chunk in data.chunks(4) {
            if chunk.len() >= 3 {
                let b = chunk[0];
                let g = chunk[1];
                let r = chunk[2];
                rgb_buf.extend_from_slice(&[r, g, b]);
            }
        }

        writer.write_image_data(&rgb_buf)?;
    }

    Ok(png_data)
}

/// Process screenshot in background thread
fn process_screenshot_async(
    png_data: Vec<u8>,
    config: OverlayConfig,
    cancel_flag: Arc<AtomicBool>,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    // Check if cancelled before starting
    if cancel_flag.load(Ordering::SeqCst) {
        return Err("Request cancelled".into());
    }

    // Get API key (should already be validated, but check again for safety)
    let api_key = gemini::get_api_key(config.gemini_api_key).map_err(|e| e.to_string())?;

    // Analyze screenshot with cancellation support
    let analysis = gemini::analyze_screenshot_data(&png_data, &api_key, cancel_flag.clone())
        .map_err(|e| e.to_string())?;

    Ok(analysis)
}

/// Setup process-level stealth features
#[cfg(not(debug_assertions))]
fn setup_process_stealth() -> Result<(), Box<dyn Error>> {
    // Change process name to something innocuous
    set_process_name("pipewire")?;

    // Set low priority to avoid detection in system monitors
    unsafe {
        libc::nice(19); // Lowest priority
    }

    Ok(())
}

/// Set process name to masquerade as a kernel worker
#[cfg(not(debug_assertions))]
fn set_process_name(name: &str) -> Result<(), Box<dyn Error>> {
    use std::ffi::CString;

    let name_c = CString::new(name)?;
    unsafe {
        libc::prctl(libc::PR_SET_NAME, name_c.as_ptr(), 0, 0, 0);
    }

    Ok(())
}

/// Hide window from window manager and system panels
#[cfg(not(debug_assertions))]
fn hide_from_window_manager(conn: &RustConnection, win: u32) -> Result<(), Box<dyn Error>> {
    // Remove all window manager hints
    // Don't set WM_NAME (no window title)
    // Don't set WM_CLASS (no application identification)
    // Don't set WM_PROTOCOLS
    // Don't set any ICCCM properties

    // Set window type to desktop to avoid detection
    let net_wm_window_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE")?;
    let net_wm_window_type_desktop = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE_DESKTOP")?;

    conn.change_property(
        PropMode::REPLACE,
        win,
        net_wm_window_type.reply()?.atom,
        AtomEnum::ATOM,
        32,
        1,
        &net_wm_window_type_desktop.reply()?.atom.to_ne_bytes(),
    )?;

    // Skip taskbar and pager
    let net_wm_state = conn.intern_atom(false, b"_NET_WM_STATE")?;
    let skip_taskbar = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_TASKBAR")?;
    let skip_pager = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_PAGER")?;

    let state_atoms = [skip_taskbar.reply()?.atom, skip_pager.reply()?.atom];

    let mut state_bytes = Vec::new();
    for atom in &state_atoms {
        state_bytes.extend_from_slice(&atom.to_ne_bytes());
    }

    conn.change_property(
        PropMode::REPLACE,
        win,
        net_wm_state.reply()?.atom,
        AtomEnum::ATOM,
        32,
        2,
        &state_bytes,
    )?;

    Ok(())
}
