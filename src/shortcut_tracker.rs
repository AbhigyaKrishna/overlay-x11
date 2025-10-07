use crate::modifier_mapper::ModifierMapper;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use x11rb::protocol::xproto::Keycode;

/// Unified key state and shortcut combination tracker
pub struct ShortcutTracker {
    // Key state tracking
    pressed_keys: HashSet<Keycode>,

    // Modifier keycodes
    ctrl_keycode: Option<u8>,
    shift_keycode: Option<u8>,
    alt_keycode: Option<u8>,

    // Timing and debouncing
    last_combo_time: Instant,
    combo_timeout: Duration,

    // Track recent modifier presses for timing tolerance
    recent_ctrl: Option<Instant>,
    recent_shift: Option<Instant>,
    recent_alt: Option<Instant>,
}

impl ShortcutTracker {
    pub fn new() -> Self {
        Self {
            pressed_keys: HashSet::new(),
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

    /// Track key press event
    pub fn key_pressed(&mut self, keycode: Keycode) {
        self.pressed_keys.insert(keycode);
    }

    /// Track key release event
    pub fn key_released(&mut self, keycode: Keycode) {
        self.pressed_keys.remove(&keycode);
    }

    /// Get currently pressed keys
    pub fn get_pressed_keys(&self) -> Vec<Keycode> {
        self.pressed_keys.iter().copied().collect()
    }

    /// Clear all key states
    pub fn clear_all_keys(&mut self) {
        self.pressed_keys.clear();

        #[cfg(debug_assertions)]
        println!("Debug: All key states cleared");
    }

    /// Periodic cleanup to prevent stuck keys
    pub fn cleanup_stale_keys(&mut self) {
        if self.pressed_keys.len() > 3 {
            self.pressed_keys.clear();
            println!("Cleaned up potentially stuck keys");
        }
    }

    /// Reset modifier states for clean detection
    pub fn reset_modifier_states(&mut self) {
        self.recent_ctrl = None;
        self.recent_shift = None;
        self.recent_alt = None;
        self.last_combo_time = Instant::now();

        #[cfg(debug_assertions)]
        println!("Debug: Modifier states reset");
    }

    /// Update keycodes from modifier mapper
    pub fn update_keycodes(&mut self, modifier_mapper: &ModifierMapper) {
        // Get standard modifier keycodes
        self.ctrl_keycode = modifier_mapper
            .get_keycode(0xffe3)
            .or_else(|| modifier_mapper.get_keycode(0xffe4));
        self.shift_keycode = modifier_mapper
            .get_keycode(0xffe1)
            .or_else(|| modifier_mapper.get_keycode(0xffe2));
        self.alt_keycode = Some(64); // Use the actual Alt keycode we detected

        // If Shift keycode is not detected, use common defaults
        if self.shift_keycode.is_none() {
            self.shift_keycode = Some(50); // Common Shift_L keycode
        }

        #[cfg(debug_assertions)]
        println!(
            "Debug: ShortcutTracker keycodes - ctrl={:?}, shift={:?}, alt={:?}",
            self.ctrl_keycode, self.shift_keycode, self.alt_keycode
        );
    }

    /// Get ctrl keycode
    pub fn ctrl_keycode(&self) -> Option<u8> {
        self.ctrl_keycode
    }

    /// Get shift keycode
    pub fn shift_keycode(&self) -> Option<u8> {
        self.shift_keycode
    }

    /// Get alt keycode
    pub fn alt_keycode(&self) -> Option<u8> {
        self.alt_keycode
    }

    /// Check if a specific combination is pressed
    fn check_combination(
        &mut self,
        target_key: u8,
        need_ctrl: bool,
        need_shift: bool,
        need_alt: bool,
    ) -> bool {
        let now = Instant::now();

        let pressed_keys: Vec<u8> = self.pressed_keys.iter().copied().collect();

        // Check if target key is pressed
        if !pressed_keys.contains(&target_key) {
            return false;
        }

        // FIX 1: More generous timing window for robust detection
        let timing_tolerance = Duration::from_millis(250); // Increased for better reliability

        // Update recent modifier timestamps with multiple keycode support
        if let Some(ctrl_key) = self.ctrl_keycode {
            if pressed_keys.contains(&ctrl_key)
                || pressed_keys.contains(&37)
                || pressed_keys.contains(&105)
            {
                self.recent_ctrl = Some(now);
            }
        }

        if let Some(shift_key) = self.shift_keycode {
            if pressed_keys.contains(&shift_key)
                || pressed_keys.contains(&50)
                || pressed_keys.contains(&62)
            {
                self.recent_shift = Some(now);
            }
        }

        if let Some(alt_key) = self.alt_keycode {
            if pressed_keys.contains(&alt_key)
                || pressed_keys.contains(&64)
                || pressed_keys.contains(&108)
            {
                self.recent_alt = Some(now);
            }
        }

        let mut has_ctrl = false;
        let mut has_shift = false;
        let mut has_alt = false;

        // FIX 2: More robust modifier checking with multiple keycode support
        if need_ctrl {
            has_ctrl = self.ctrl_keycode.map_or(false, |k| pressed_keys.contains(&k))
                || pressed_keys.contains(&37) || pressed_keys.contains(&105) // Left/Right Ctrl
                || self.recent_ctrl.map_or(false, |t| now.duration_since(t) < timing_tolerance);
        }

        if need_shift {
            has_shift = self.shift_keycode.map_or(false, |k| pressed_keys.contains(&k))
                || pressed_keys.contains(&50) || pressed_keys.contains(&62) // Left/Right Shift
                || self.recent_shift.map_or(false, |t| now.duration_since(t) < timing_tolerance);

            #[cfg(debug_assertions)]
            if need_shift {
                let shift_currently = self
                    .shift_keycode
                    .map_or(false, |k| pressed_keys.contains(&k))
                    || pressed_keys.contains(&50)
                    || pressed_keys.contains(&62);
                let shift_recent = self
                    .recent_shift
                    .map_or(false, |t| now.duration_since(t) < timing_tolerance);
                println!(
                    "Debug: Shift check - currently={}, recent={}, combined={}",
                    shift_currently, shift_recent, has_shift
                );
            }
        }

        if need_alt {
            has_alt = self.alt_keycode.map_or(false, |k| pressed_keys.contains(&k))
                || pressed_keys.contains(&64) || pressed_keys.contains(&108) // Left/Right Alt
                || self.recent_alt.map_or(false, |t| now.duration_since(t) < timing_tolerance);
        }

        // Check if we have the required combination
        let combo_match =
            (!need_ctrl || has_ctrl) && (!need_shift || has_shift) && (!need_alt || has_alt);

        #[cfg(debug_assertions)]
        if combo_match {
            println!(
                "Debug: Combination matched! ctrl={}, shift={}, alt={}",
                has_ctrl, has_shift, has_alt
            );
        }

        if combo_match {
            // FIX 3: Robust debounce time to prevent rapid triggering and ensure clean detection
            if now.duration_since(self.last_combo_time) > Duration::from_millis(500) {
                // Increased for robustness
                self.last_combo_time = now;

                #[cfg(debug_assertions)]
                println!("Debug: Shortcut accepted after debounce period");

                return true;
            } else {
                #[cfg(debug_assertions)]
                println!(
                    "Debug: Shortcut blocked by debounce ({}ms ago)",
                    now.duration_since(self.last_combo_time).as_millis()
                );
            }
        }

        false
    }

    /// Check for Ctrl+Shift+E combination
    pub fn check_ctrl_shift_e(&mut self, keycode_e: u8) -> bool {
        self.check_combination(keycode_e, true, true, false)
    }

    /// Check for Ctrl+Shift+Q combination
    pub fn check_ctrl_shift_q(&mut self, keycode_q: u8) -> bool {
        self.check_combination(keycode_q, true, true, false)
    }

    /// Check for Ctrl+Q combination
    pub fn check_ctrl_q(&mut self, keycode_q: u8) -> bool {
        self.check_combination(keycode_q, true, false, false)
    }

    /// Check for Ctrl+Alt+E combination
    pub fn check_ctrl_alt_e(&mut self, keycode_e: u8) -> bool {
        self.check_combination(keycode_e, true, false, true)
    }
}
