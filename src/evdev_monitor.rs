use evdev::{Device, EventType, InputEventKind, Key};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

/// Linux evdev direct monitoring for system-level stealth
pub struct EvdevMonitor {
    receiver: Receiver<EvdevEvent>,
}

#[derive(Debug, Clone)]
pub struct EvdevEvent {
    pub keycode: u16,
    pub pressed: bool,
}

impl EvdevMonitor {
    /// Create a new evdev monitor
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let (sender, receiver) = channel();

        // Find all keyboard devices
        let devices = Self::find_keyboard_devices()?;

        if devices.is_empty() {
            return Err("No keyboard devices found".into());
        }

        #[cfg(debug_assertions)]
        println!("Debug: Found {} keyboard device(s)", devices.len());

        // Spawn monitoring thread
        thread::spawn(move || {
            if let Err(e) = Self::monitor_loop(devices, sender) {
                #[cfg(debug_assertions)]
                eprintln!("Debug: Evdev monitor error: {}", e);
            }
        });

        Ok(EvdevMonitor { receiver })
    }

    /// Find all keyboard input devices
    fn find_keyboard_devices() -> Result<Vec<Device>, Box<dyn Error>> {
        let mut keyboards = Vec::new();

        // Enumerate /dev/input/event* devices
        for entry in fs::read_dir("/dev/input")? {
            let entry = entry?;
            let path = entry.path();

            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with("event") {
                    // Try to open the device
                    if let Ok(device) = Device::open(&path) {
                        // Check if it's a keyboard device
                        if Self::is_keyboard_device(&device) {
                            #[cfg(debug_assertions)]
                            println!(
                                "Debug: Found keyboard: {} at {:?}",
                                device.name().unwrap_or("Unknown"),
                                path
                            );
                            keyboards.push(device);
                        }
                    }
                }
            }
        }

        Ok(keyboards)
    }

    /// Check if a device is a keyboard
    fn is_keyboard_device(device: &Device) -> bool {
        // A keyboard should support key events
        if !device.supported_events().contains(EventType::KEY) {
            return false;
        }

        // Check if it has typical keyboard keys
        if let Some(keys) = device.supported_keys() {
            // Check for common keyboard keys
            keys.contains(Key::KEY_A)
                || keys.contains(Key::KEY_ENTER)
                || keys.contains(Key::KEY_SPACE)
        } else {
            false
        }
    }

    /// Main monitoring loop (runs in separate thread)
    fn monitor_loop(
        devices: Vec<Device>,
        sender: Sender<EvdevEvent>,
    ) -> Result<(), Box<dyn Error>> {
        // Convert to mutable devices
        let mut devices: HashMap<_, _> = devices
            .into_iter()
            .enumerate()
            .map(|(i, d)| (i, d))
            .collect();

        loop {
            // Poll each device
            for (_id, device) in devices.iter_mut() {
                // Fetch events without blocking
                while let Ok(events) = device.fetch_events() {
                    for event in events {
                        if let InputEventKind::Key(key) = event.kind() {
                            let keycode = key.code();
                            let pressed = event.value() == 1;

                            let ev = EvdevEvent { keycode, pressed };

                            // Send event (ignore errors if receiver is dropped)
                            let _ = sender.send(ev);
                        }
                    }
                }
            }

            // Small sleep to avoid busy-waiting
            thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Try to receive an event (non-blocking)
    pub fn try_recv(&self) -> Option<EvdevEvent> {
        self.receiver.try_recv().ok()
    }

    /// Receive an event (blocking)
    pub fn recv(&self) -> Result<EvdevEvent, Box<dyn Error>> {
        self.receiver
            .recv()
            .map_err(|e| Box::new(e) as Box<dyn Error>)
    }
}

/// Map evdev key codes to X11 keycodes
/// Note: This is an approximation - exact mapping may vary
pub fn evdev_to_x11_keycode(evdev_code: u16) -> u8 {
    // X11 keycodes are typically evdev codes + 8
    // This is the standard mapping used by most X servers
    if evdev_code < 248 {
        (evdev_code + 8) as u8
    } else {
        0
    }
}

/// Common key codes for convenience
#[allow(dead_code)]
pub mod keycodes {
    pub const KEY_E: u16 = 18;
    pub const KEY_S: u16 = 31;
    pub const KEY_UP: u16 = 103;
    pub const KEY_DOWN: u16 = 108;
    pub const KEY_LEFT: u16 = 105;
    pub const KEY_RIGHT: u16 = 106;
    pub const KEY_LEFTCTRL: u16 = 29;
    pub const KEY_RIGHTCTRL: u16 = 97;
    pub const KEY_LEFTALT: u16 = 56;
    pub const KEY_RIGHTALT: u16 = 100;
}
