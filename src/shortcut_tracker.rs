use crate::modifier_mapper::ModifierMapper;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use x11rb::protocol::xproto::Keycode;

/// Simple shortcut tracker without debouncing
pub struct ShortcutTracker {
    // Key state tracking
    pressed_keys: HashSet<Keycode>,
    
    // Modifier keycodes
    ctrl_keycodes: Vec<Keycode>,
    shift_keycodes: Vec<Keycode>,
    
    // Target key keycodes
    keycode_e: Option<Keycode>,
    keycode_q: Option<Keycode>,
    
    // Simple state tracking for immediate response
    last_trigger_time: Option<Instant>,
}

impl ShortcutTracker {
    pub fn new() -> Self {
        Self {
            pressed_keys: HashSet::new(),
            ctrl_keycodes: vec![37, 105], // Left Ctrl, Right Ctrl
            shift_keycodes: vec![50, 62], // Left Shift, Right Shift
            keycode_e: None,
            keycode_q: None,
            last_trigger_time: None,
        }
    }

    /// Track key press event
    pub fn key_pressed(&mut self, keycode: Keycode) {
        self.pressed_keys.insert(keycode);
    }

    /// Track key release event
    pub fn key_released(&mut self, keycode: Keycode) {
        self.pressed_keys.remove(&keycode);
    }

    /// Check if Ctrl+Shift+E is currently pressed (instant detection)
    pub fn check_ctrl_shift_e(&mut self, keycode_e: u8) -> bool {
        let keycode_e = keycode_e as Keycode;
        
        // Check if all required keys are currently pressed
        let ctrl_pressed = self.is_ctrl_pressed();
        let shift_pressed = self.is_shift_pressed();
        let e_pressed = self.pressed_keys.contains(&keycode_e);
        
        // Immediate detection without debouncing
        if ctrl_pressed && shift_pressed && e_pressed {
            // Optional: Prevent extremely rapid triggering (1ms minimum)
            let now = Instant::now();
            if let Some(last_time) = self.last_trigger_time {
                if now.duration_since(last_time) < Duration::from_millis(1) {
                    return false;
                }
            }
            
            self.last_trigger_time = Some(now);
            return true;
        }
        
        false
    }

    /// Check if Ctrl+Shift+Q is currently pressed (instant detection)
    pub fn check_ctrl_shift_q(&mut self, keycode_q: u8) -> bool {
        let keycode_q = keycode_q as Keycode;
        
        // Check if all required keys are currently pressed
        let ctrl_pressed = self.is_ctrl_pressed();
        let shift_pressed = self.is_shift_pressed();
        let q_pressed = self.pressed_keys.contains(&keycode_q);
        
        // Immediate detection without debouncing
        if ctrl_pressed && shift_pressed && q_pressed {
            // Optional: Prevent extremely rapid triggering (1ms minimum)
            let now = Instant::now();
            if let Some(last_time) = self.last_trigger_time {
                if now.duration_since(last_time) < Duration::from_millis(1) {
                    return false;
                }
            }
            
            self.last_trigger_time = Some(now);
            return true;
        }
        
        false
    }

    /// Helper functions
    fn is_ctrl_pressed(&self) -> bool {
        self.ctrl_keycodes
            .iter()
            .any(|&k| self.pressed_keys.contains(&k))
    }

    fn is_shift_pressed(&self) -> bool {
        self.shift_keycodes
            .iter()
            .any(|&k| self.pressed_keys.contains(&k))
    }

    /// Update keycodes from modifier mapper
    pub fn update_keycodes(&mut self, modifier_mapper: &ModifierMapper) {
        // Update with detected keycodes
        if let Some(ctrl) = modifier_mapper.get_keycode(0xffe3) {
            if !self.ctrl_keycodes.contains(&ctrl) {
                self.ctrl_keycodes.push(ctrl);
            }
        }
        if let Some(ctrl_r) = modifier_mapper.get_keycode(0xffe4) {
            if !self.ctrl_keycodes.contains(&ctrl_r) {
                self.ctrl_keycodes.push(ctrl_r);
            }
        }
        if let Some(shift) = modifier_mapper.get_keycode(0xffe1) {
            if !self.shift_keycodes.contains(&shift) {
                self.shift_keycodes.push(shift);
            }
        }
        if let Some(shift_r) = modifier_mapper.get_keycode(0xffe2) {
            if !self.shift_keycodes.contains(&shift_r) {
                self.shift_keycodes.push(shift_r);
            }
        }
        
        self.keycode_e = modifier_mapper.get_keycode(0x0065);
        self.keycode_q = modifier_mapper.get_keycode(0x0071);
    }

    /// Get currently pressed keys
    pub fn get_pressed_keys(&self) -> Vec<Keycode> {
        self.pressed_keys.iter().copied().collect()
    }

    /// Cleanup functions (simplified)
    pub fn clear_all_keys(&mut self) {
        self.pressed_keys.clear();
    }

    pub fn cleanup_stale_keys(&mut self) {
        // Only clear if we have an unreasonable number of keys
        if self.pressed_keys.len() > 10 {
            self.clear_all_keys();
        }
    }

    pub fn reset_modifier_states(&mut self) {
        // Simple reset - no complex state machine
        // Only clear the timing to allow immediate next trigger
        if let Some(last_time) = self.last_trigger_time {
            if last_time.elapsed() > Duration::from_millis(100) {
                self.last_trigger_time = None;
            }
        }
    }

    /// Getters for compatibility
    pub fn ctrl_keycode(&self) -> Option<Keycode> {
        self.ctrl_keycodes.first().copied()
    }

    pub fn shift_keycode(&self) -> Option<Keycode> {
        self.shift_keycodes.first().copied()
    }
}
