mod config;
mod evdev_monitor;
mod gemini;
mod modifier_mapper;
mod renderer;
mod xinput2_monitor;

use std::error::Error;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::shape::ConnectionExt as _;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use config::OverlayConfig;
use evdev_monitor::EvdevMonitor;
use modifier_mapper::ModifierMapper;
use renderer::Renderer;
use xinput2_monitor::KeyStateTracker;

// X11 keysyms
const XK_E: u32 = 0x0065; // 'E' key
const XK_S: u32 = 0x0073; // 'S' key
const XK_UP: u32 = 0xff52; // Up arrow
const XK_DOWN: u32 = 0xff54; // Down arrow
const XK_LEFT: u32 = 0xff51; // Left arrow
const XK_RIGHT: u32 = 0xff53; // Right arrow

/// Robust shortcut combination tracker
struct ShortcutTracker {
    ctrl_keycode: Option<u8>,
    shift_keycode: Option<u8>,
    alt_keycode: Option<u8>,
    last_combo_time: Instant,
    combo_timeout: Duration,
    // Track recent modifier presses for timing tolerance
    recent_ctrl: Option<Instant>,
    recent_shift: Option<Instant>,
    recent_alt: Option<Instant>,
}

impl ShortcutTracker {
    fn new() -> Self {
        Self {
            ctrl_keycode: None,
            shift_keycode: None, 
            alt_keycode: None,
            last_combo_time: Instant::now(),
            combo_timeout: Duration::from_millis(500), // 500ms window for combinations
            recent_ctrl: None,
            recent_shift: None,
            recent_alt: None,
        }
    }
    
    fn update_keycodes(&mut self, modifier_mapper: &ModifierMapper) {
        // Get standard modifier keycodes
        self.ctrl_keycode = modifier_mapper.get_keycode(0xffe3).or_else(|| modifier_mapper.get_keycode(0xffe4));
        self.shift_keycode = modifier_mapper.get_keycode(0xffe1).or_else(|| modifier_mapper.get_keycode(0xffe2));
        self.alt_keycode = Some(64); // Use the actual Alt keycode we detected
        
        // If Shift keycode is not detected, use common defaults
        if self.shift_keycode.is_none() {
            self.shift_keycode = Some(50); // Common Shift_L keycode
        }
        
        #[cfg(debug_assertions)]
        println!("Debug: ShortcutTracker keycodes - ctrl={:?}, shift={:?}, alt={:?}", 
                 self.ctrl_keycode, self.shift_keycode, self.alt_keycode);
    }
    
    fn check_combination(&mut self, pressed_keys: &[u8], target_key: u8, need_ctrl: bool, need_shift: bool, need_alt: bool) -> bool {
        let now = Instant::now();
        
        // Check if target key is pressed
        if !pressed_keys.contains(&target_key) {
            return false;
        }
        
        // Update recent modifier timestamps
        if let Some(ctrl_key) = self.ctrl_keycode {
            if pressed_keys.contains(&ctrl_key) {
                self.recent_ctrl = Some(now);
            }
        }
        
        if let Some(shift_key) = self.shift_keycode {
            if pressed_keys.contains(&shift_key) || pressed_keys.contains(&50) || pressed_keys.contains(&62) {
                self.recent_shift = Some(now);
            }
        }
        
        if let Some(alt_key) = self.alt_keycode {
            if pressed_keys.contains(&alt_key) {
                self.recent_alt = Some(now);
            }
        }
        
        let mut has_ctrl = false;
        let mut has_shift = false;
        let mut has_alt = false;
        
        // Check modifiers with timing tolerance
        if need_ctrl {
            has_ctrl = self.ctrl_keycode.map_or(false, |k| pressed_keys.contains(&k))
                || self.recent_ctrl.map_or(false, |t| now.duration_since(t) < Duration::from_millis(300));
        }
        
        if need_shift {
            has_shift = self.shift_keycode.map_or(false, |k| pressed_keys.contains(&k))
                || pressed_keys.contains(&50) || pressed_keys.contains(&62)
                || self.recent_shift.map_or(false, |t| now.duration_since(t) < Duration::from_millis(300));
                
            #[cfg(debug_assertions)]
            if need_shift {
                let shift_currently = self.shift_keycode.map_or(false, |k| pressed_keys.contains(&k))
                    || pressed_keys.contains(&50) || pressed_keys.contains(&62);
                let shift_recent = self.recent_shift.map_or(false, |t| now.duration_since(t) < Duration::from_millis(300));
                println!("Debug: Shift check - currently={}, recent={}, combined={}", 
                         shift_currently, shift_recent, has_shift);
            }
        }
        
        if need_alt {
            has_alt = self.alt_keycode.map_or(false, |k| pressed_keys.contains(&k))
                || self.recent_alt.map_or(false, |t| now.duration_since(t) < Duration::from_millis(300));
        }
        
        // Check if we have the required combination
        let combo_match = (!need_ctrl || has_ctrl) && 
                         (!need_shift || has_shift) && 
                         (!need_alt || has_alt);
        
        #[cfg(debug_assertions)]
        if combo_match {
            println!("Debug: Combination matched! ctrl={}, shift={}, alt={}", has_ctrl, has_shift, has_alt);
        }
        
        if combo_match {
            // Prevent rapid triggering
            if now.duration_since(self.last_combo_time) > Duration::from_millis(200) {
                self.last_combo_time = now;
                return true;
            }
        }
        
        false
    }
    
    fn check_ctrl_shift_e(&mut self, pressed_keys: &[u8], keycode_e: u8) -> bool {
        self.check_combination(pressed_keys, keycode_e, true, true, false)
    }
    
    fn check_ctrl_shift_s(&mut self, pressed_keys: &[u8], keycode_s: u8) -> bool {
        self.check_combination(pressed_keys, keycode_s, true, true, false)
    }
    
    fn check_ctrl_alt_e(&mut self, pressed_keys: &[u8], keycode_e: u8) -> bool {
        self.check_combination(pressed_keys, keycode_e, true, false, true)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let config_path = args.get(1).cloned();

    // Load configuration from file or use defaults
    let mut config = OverlayConfig::load(config_path);

    #[cfg(debug_assertions)]
    println!("Debug: Config loaded: {:?}", config);

    // Setup process stealth features only in release builds
    #[cfg(not(debug_assertions))]
    setup_process_stealth()?;

    #[cfg(debug_assertions)]
    println!("Debug mode: Starting overlay (stealth disabled)");
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

        #[cfg(debug_assertions)]
        println!("Debug: Centering overlay at ({}, {})", config.x, config.y);
    }

    // Open X11 font from config
    let font_id = conn.generate_id()?;
    let font_bytes = config.font.as_bytes();
    if conn.open_font(font_id, font_bytes).is_err() {
        // Fallback to a medium size
        let fallback = b"-misc-fixed-medium-r-normal--15-140-75-75-C-90-iso8859-1";
        if conn.open_font(font_id, fallback).is_err() {
            // Last resort: simple "fixed" font
            conn.open_font(font_id, b"fixed")?;
            #[cfg(debug_assertions)]
            println!("Debug: Using fallback 'fixed' font");
        }
    }

    // Query font metrics for proper line spacing
    let font_info = conn.query_font(font_id)?.reply()?;
    let font_ascent = font_info.font_ascent as u16;
    let font_descent = font_info.font_descent as u16;

    #[cfg(debug_assertions)]
    println!(
        "Font metrics: ascent={}, descent={}, line_height={}",
        font_ascent,
        font_descent,
        font_ascent + font_descent
    );

    // Initialize renderer with font, metrics, and multi-line text for scrolling demo
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

    #[cfg(debug_assertions)]
    println!(
        "Debug: Window created at {}x{} with size {}x{}",
        config.x, config.y, config.width, config.height
    );

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
        Ok(monitor) => {
            #[cfg(debug_assertions)]
            println!("Debug: Using evdev monitoring (no grabbing, fully transparent)");
            Some(monitor)
        }
        Err(e) => {
            #[cfg(debug_assertions)]
            eprintln!("Debug: Evdev monitoring unavailable: {}. This overlay requires evdev access.", e);
            eprintln!("Please ensure you have permission to access /dev/input/event* devices.");
            eprintln!("You may need to add your user to the 'input' group:");
            eprintln!("  sudo usermod -a -G input $USER");
            return Err("Evdev monitoring required but unavailable".into());
        }
    };

    // Get keycodes for our hotkeys
    let keycode_e = modifier_mapper.get_keycode(XK_E).ok_or("E key not found")?;
    let keycode_s = modifier_mapper.get_keycode(XK_S).ok_or("S key not found")?;
    let keycode_up = modifier_mapper.get_keycode(XK_UP).ok_or("Up key not found")?;
    let keycode_down = modifier_mapper.get_keycode(XK_DOWN).ok_or("Down key not found")?;
    let keycode_left = modifier_mapper.get_keycode(XK_LEFT).ok_or("Left key not found")?;
    let keycode_right = modifier_mapper.get_keycode(XK_RIGHT).ok_or("Right key not found")?;

    #[cfg(debug_assertions)]
    println!("Debug: Keycodes mapped - E={}, S={}, Up={}, Down={}, Left={}, Right={}", 
             keycode_e, keycode_s, keycode_up, keycode_down, keycode_left, keycode_right);
             
    // Also log the S keycode specifically for debugging
    println!("🔧 S key mapped to keycode: {}", keycode_s);

    // Track key states for combination detection
    let mut key_tracker = KeyStateTracker::new();
    
    // Initialize robust shortcut tracker
    let mut shortcut_tracker = ShortcutTracker::new();
    shortcut_tracker.update_keycodes(&modifier_mapper);

    // Initial state: visible
    let mut visible = true;
    conn.map_window(win)?;
    conn.flush()?;

    #[cfg(debug_assertions)]
    println!("Debug: Overlay started. Press Ctrl+Shift+E to toggle, Ctrl+Shift+S to screenshot.");
    
    println!("=== OVERLAY CONTROLS ===");
    println!("📋 Toggle Overlay: Hold Ctrl + Shift, then press E");
    println!("📸 Screenshot + AI: Hold Ctrl + Shift, then press S");
    println!("🔍 When overlay is visible: Use arrow keys to scroll");
    println!("========================");

    // Event loop - handle both XInput2 raw events and evdev events
    loop {
        // Handle evdev events if available
        if let Some(ref evdev) = evdev_monitor {
            while let Some(ev) = evdev.try_recv() {
                let x11_keycode = evdev_monitor::evdev_to_x11_keycode(ev.keycode);
                
                #[cfg(debug_assertions)]
                println!("Debug: Evdev event - code={}, x11_keycode={}, pressed={}", 
                         ev.keycode, x11_keycode, ev.pressed);
                
                if ev.pressed {
                    key_tracker.key_pressed(x11_keycode);
                } else {
                    key_tracker.key_released(x11_keycode);
                }

                // Check for hotkey combinations
                if handle_key_event(
                    x11_keycode,
                    ev.pressed,
                    &key_tracker,
                    &mut shortcut_tracker,
                    keycode_e,
                    keycode_s,
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
                // Keyboard layout changed, refresh modifier mapping
                #[cfg(debug_assertions)]
                println!("Debug: Keyboard mapping changed, refreshing...");
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
    key_tracker: &KeyStateTracker,
    shortcut_tracker: &mut ShortcutTracker,
    keycode_e: u8,
    keycode_s: u8,
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
) -> Result<bool, Box<dyn Error>> {
    if !pressed {
        return Ok(false);
    }

    let pressed_keys = key_tracker.get_pressed_keys();
    
    #[cfg(debug_assertions)]
    println!("Debug: Key pressed - keycode={}, E={}, S={}", keycode, keycode_e, keycode_s);
    
    #[cfg(debug_assertions)]
    println!("Debug: Currently pressed keys: {:?}", pressed_keys);
    
    // Show which specific key was just pressed
    if keycode == keycode_e {
        println!("🔤 E key pressed!");
    } else if keycode == keycode_s {
        println!("🔤 S key pressed!");
    } else if keycode == 37 { // Ctrl
        println!("⌨️ Ctrl key pressed!");
    } else if keycode == 50 { // Shift
        println!("⌨️ Shift key pressed!");
    } else {
        println!("🔤 Key {} pressed", keycode);
    }
    
    // Show user-friendly key detection info
    if pressed_keys.len() > 1 {
        let mut detected_mods = Vec::new();
        
        // Check what modifiers are detected
        if shortcut_tracker.ctrl_keycode.map_or(false, |k| pressed_keys.contains(&k)) {
            detected_mods.push("Ctrl");
        }
        
        if shortcut_tracker.shift_keycode.map_or(false, |k| pressed_keys.contains(&k)) ||
           pressed_keys.contains(&50) || pressed_keys.contains(&62) {
            detected_mods.push("Shift");
        }
        
        if shortcut_tracker.alt_keycode.map_or(false, |k| pressed_keys.contains(&k)) {
            detected_mods.push("Alt");
        }
        
        if !detected_mods.is_empty() {
            let mods_str = detected_mods.join(" + ");
            println!("🔍 Detected modifiers: {}", mods_str);
            
            // Show helpful hints
            if mods_str == "Ctrl + Shift" {
                println!("💡 Perfect! Now press E (toggle) or S (screenshot)");
            }
        }
    }

    // Check for Ctrl+Shift+E (toggle overlay)
    if shortcut_tracker.check_ctrl_shift_e(&pressed_keys, keycode_e) {
        println!("✅ Ctrl+Shift+E detected! Toggling overlay...");
        
        if *visible {
            conn.unmap_window(win)?;
            println!("👁️  Overlay hidden");
        } else {
            conn.map_window(win)?;
            println!("👁️  Overlay shown");
        }
        *visible = !*visible;
        conn.flush()?;
        return Ok(true);
    }
    
    // Check for Ctrl+Alt+E (alternative toggle)
    if shortcut_tracker.check_ctrl_alt_e(&pressed_keys, keycode_e) {
        println!("✅ Ctrl+Alt+E detected! Toggling overlay...");
        
        if *visible {
            conn.unmap_window(win)?;
            println!("👁️  Overlay hidden");
        } else {
            conn.map_window(win)?;
            println!("👁️  Overlay shown");
        }
        *visible = !*visible;
        conn.flush()?;
        return Ok(true);
    }

    // Check for Ctrl+Shift+S (screenshot)
    if shortcut_tracker.check_ctrl_shift_s(&pressed_keys, keycode_s) {
        println!("✅ Ctrl+Shift+S detected! Taking screenshot and analyzing...");
        
        // Debug: Show what we're about to do
        #[cfg(debug_assertions)]
        println!("Debug: Starting screenshot process...");

        // Temporarily hide overlay if visible
        if *visible {
            conn.unmap_window(win)?;
            conn.flush()?;
            println!("📷 Hiding overlay for clean screenshot...");
            std::thread::sleep(Duration::from_millis(100));
            
            #[cfg(debug_assertions)]
            println!("Debug: Overlay hidden, starting capture...");
        }

        println!("📷 Capturing screenshot...");
        
        // Debug: Show screenshot attempt
        #[cfg(debug_assertions)]
        println!("Debug: Calling capture_screenshot with {}x{}", screen_width, screen_height);
        
        // Capture screenshot
        match capture_screenshot(conn, root, screen_width, screen_height) {
            Ok(png_data) => {
                println!("✅ Screenshot captured ({} bytes)", png_data.len());
                println!("🤖 Sending to Gemini AI for analysis...");
                
                #[cfg(debug_assertions)]
                println!("Debug: Screenshot successful, checking API key...");
                
                match gemini::get_api_key(config.gemini_api_key.clone()) {
                    Ok(api_key) => {
                        #[cfg(debug_assertions)]
                        println!("Debug: API key found, sending to Gemini...");
                        
                        match gemini::analyze_screenshot_data(&png_data, &api_key) {
                            Ok(analysis) => {
                                println!("✅ AI analysis complete! Displaying result...");

                                let current_offset = renderer.scroll_offset();
                                *renderer = Renderer::new(config.clone())
                                    .with_font(font_id, font_ascent, font_descent)
                                    .with_text(format!("🤖 AI Screenshot Analysis:\n\n{}", analysis))
                                    .with_scroll_offset(current_offset);

                                conn.clear_area(false, win, 0, 0, 0, 0)?;
                                conn.flush()?;
                                
                                // Show overlay with results
                                if !*visible {
                                    conn.map_window(win)?;
                                    *visible = true;
                                    println!("👁️  Overlay shown with AI analysis");
                                }
                                
                                #[cfg(debug_assertions)]
                                println!("Debug: Screenshot process completed successfully");
                            }
                            Err(e) => {
                                println!("{}", e); // Error message is already formatted nicely
                                #[cfg(debug_assertions)]
                                println!("Debug: Gemini analysis failed: {}", e);
                            }
                        }
                    },
                    Err(e) => {
                        println!("{}", e); // Error message is already formatted nicely
                        #[cfg(debug_assertions)]
                        println!("Debug: API key error: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("❌ Screenshot capture failed: {}", e);
                #[cfg(debug_assertions)]
                println!("Debug: Screenshot capture error: {}", e);
            }
        }

        // Restore overlay
        if *visible {
            conn.map_window(win)?;
            conn.flush()?;
            #[cfg(debug_assertions)]
            println!("Debug: Overlay restored");
        }
        return Ok(true);
    }
    
    // Debug: Show when S key is pressed but combination doesn't match
    if keycode == keycode_s {
        #[cfg(debug_assertions)]
        println!("Debug: S key pressed but Ctrl+Shift+S not detected");
        
        // Check individual modifiers
        let has_ctrl = shortcut_tracker.ctrl_keycode.map_or(false, |k| pressed_keys.contains(&k));
        let has_shift = shortcut_tracker.shift_keycode.map_or(false, |k| pressed_keys.contains(&k))
            || pressed_keys.contains(&50) || pressed_keys.contains(&62);
            
        println!("📸 S key detected! Need: Ctrl={}, Shift={}", has_ctrl, has_shift);
        
        if !has_ctrl {
            println!("⚠️  Missing Ctrl! Hold Ctrl+Shift, then press S");
        } else if !has_shift {
            println!("⚠️  Missing Shift! Hold Ctrl+Shift, then press S");
        }
        
        // FALLBACK: Simple combination check for testing
        if has_ctrl && has_shift {
            println!("🔧 Fallback: Attempting screenshot with simple detection...");
            
            // Simple screenshot attempt
            match capture_screenshot(conn, root, screen_width, screen_height) {
                Ok(png_data) => {
                    println!("✅ Fallback screenshot captured ({} bytes)", png_data.len());
                    
                    // Simple text display without Gemini for testing
                    let current_offset = renderer.scroll_offset();
                    *renderer = Renderer::new(config.clone())
                        .with_font(font_id, font_ascent, font_descent)
                        .with_text(format!("📷 Screenshot Test Successful!\n\nCaptured {} bytes at {}x{}\n\nPress Ctrl+Shift+E to toggle overlay", png_data.len(), screen_width, screen_height))
                        .with_scroll_offset(current_offset);

                    conn.clear_area(false, win, 0, 0, 0, 0)?;
                    conn.flush()?;
                    
                    if !*visible {
                        conn.map_window(win)?;
                        *visible = true;
                        println!("👁️  Overlay shown with screenshot test result");
                    }
                }
                Err(e) => {
                    println!("❌ Fallback screenshot failed: {}", e);
                }
            }
            return Ok(true);
        }
    }

    // Handle arrow keys (only when visible)
    if *visible {
        if keycode == keycode_up {
            renderer.scroll_up();
            conn.clear_area(false, win, 0, 0, config.width, config.height)?;
            renderer.render(conn, win)?;
            conn.flush()?;
            #[cfg(debug_assertions)]
            println!("Debug: Scrolled up");
            return Ok(true);
        } else if keycode == keycode_down {
            renderer.scroll_down();
            conn.clear_area(false, win, 0, 0, config.width, config.height)?;
            renderer.render(conn, win)?;
            conn.flush()?;
            #[cfg(debug_assertions)]
            println!("Debug: Scrolled down");
            return Ok(true);
        } else if keycode == keycode_left {
            renderer.scroll_left();
            conn.clear_area(false, win, 0, 0, config.width, config.height)?;
            renderer.render(conn, win)?;
            conn.flush()?;
            #[cfg(debug_assertions)]
            println!("Debug: Scrolled left");
            return Ok(true);
        } else if keycode == keycode_right {
            renderer.scroll_right();
            conn.clear_area(false, win, 0, 0, config.width, config.height)?;
            renderer.render(conn, win)?;
            conn.flush()?;
            #[cfg(debug_assertions)]
            println!("Debug: Scrolled right");
            return Ok(true);
        }
    }

    Ok(false)
}

/// Convert a keysym to a keycode (keep for compatibility)
#[allow(dead_code)]
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
