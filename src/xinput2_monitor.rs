use std::error::Error;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

/// XInput2 raw event monitoring for transparent key detection
/// NOTE: XInput2 integration is complex with x11rb, so we primarily use evdev
pub struct XInput2Monitor {
    enabled: bool,
    xinput_opcode: u8,
}

impl XInput2Monitor {
    /// Try to set up XInput2 monitoring
    /// Currently disabled in favor of evdev monitoring
    #[allow(dead_code)]
    pub fn new(_conn: &RustConnection, _root: Window) -> Result<Self, Box<dyn Error>> {
        // XInput2 API in x11rb requires careful setup
        // For maximum stealth, we use evdev instead
        Err("XInput2 monitoring not implemented - using evdev".into())
    }

    /// Check if XInput2 monitoring is enabled
    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the XInput opcode for event filtering
    #[allow(dead_code)]
    pub fn opcode(&self) -> u8 {
        self.xinput_opcode
    }

    /// Parse a raw key press event
    #[allow(dead_code)]
    pub fn parse_raw_key_press(
        data: &[u8],
    ) -> Result<(Keycode, u16), Box<dyn Error>> {
        if data.len() < 32 {
            return Err("Invalid raw key event data".into());
        }

        let keycode = u32::from_ne_bytes([data[16], data[17], data[18], data[19]]) as u8;
        let deviceid = u16::from_ne_bytes([data[10], data[11]]);

        Ok((keycode, deviceid))
    }

    /// Parse a raw key release event (same structure as press)
    #[allow(dead_code)]
    pub fn parse_raw_key_release(
        data: &[u8],
    ) -> Result<(Keycode, u16), Box<dyn Error>> {
        Self::parse_raw_key_press(data)
    }
}

/// Key state tracker to track which keys are currently pressed
pub struct KeyStateTracker {
    pressed_keys: std::collections::HashSet<Keycode>,
}

impl KeyStateTracker {
    pub fn new() -> Self {
        KeyStateTracker {
            pressed_keys: std::collections::HashSet::new(),
        }
    }

    pub fn key_pressed(&mut self, keycode: Keycode) {
        self.pressed_keys.insert(keycode);
    }

    pub fn key_released(&mut self, keycode: Keycode) {
        self.pressed_keys.remove(&keycode);
    }

    #[allow(dead_code)]
    pub fn is_key_pressed(&self, keycode: Keycode) -> bool {
        self.pressed_keys.contains(&keycode)
    }

    pub fn get_pressed_keys(&self) -> Vec<Keycode> {
        self.pressed_keys.iter().copied().collect()
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.pressed_keys.clear();
    }
    
    // Add cleanup methods for better key state management
    pub fn clear_all_keys(&mut self) {
        self.pressed_keys.clear();
        
        #[cfg(debug_assertions)]
        println!("Debug: All key states cleared");
    }
    
    // Periodic cleanup to prevent stuck keys
    pub fn cleanup_stale_keys(&mut self) {
        // Remove keys that might be stuck due to missed release events
        // This is a safety mechanism
        if self.pressed_keys.len() > 3 {
            self.pressed_keys.clear();
            println!("🔧 Cleaned up potentially stuck keys");
        }
    }
}

impl Default for KeyStateTracker {
    fn default() -> Self {
        Self::new()
    }
}
