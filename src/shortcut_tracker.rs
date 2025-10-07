use crate::modifier_mapper::ModifierMapper;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use x11rb::protocol::xproto::Keycode;

/// State machine states for shortcut detection
#[derive(Debug, Clone, PartialEq)]
enum ShortcutState {
    Idle,
    ModifiersPressed {
        ctrl: bool,
        shift: bool,
        alt: bool,
        timestamp: Instant,
    },
    AwaitingTargetKey {
        ctrl: bool,
        shift: bool,
        alt: bool,
        timestamp: Instant,
    },
    Complete {
        shortcut_type: ShortcutType,
        timestamp: Instant,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum ShortcutType {
    CtrlShiftE,
    CtrlShiftQ,
    CtrlQ,
    CtrlAltE,
}

/// State machine-based keyboard shortcut tracker
pub struct ShortcutTracker {
    // Key state tracking
    pressed_keys: HashSet<Keycode>,

    // State machine
    state: ShortcutState,

    // Modifier keycodes
    ctrl_keycodes: Vec<Keycode>,
    shift_keycodes: Vec<Keycode>,
    alt_keycodes: Vec<Keycode>,

    // Target key keycodes
    keycode_e: Option<Keycode>,
    keycode_q: Option<Keycode>,

    // Configuration
    modifier_timeout: Duration, // How long to wait for target key after modifiers
    debounce_timeout: Duration, // Minimum time between shortcut activations
}

impl ShortcutTracker {
    pub fn new() -> Self {
        Self {
            pressed_keys: HashSet::new(),
            state: ShortcutState::Idle,
            ctrl_keycodes: vec![37, 105], // Left Ctrl, Right Ctrl
            shift_keycodes: vec![50, 62], // Left Shift, Right Shift
            alt_keycodes: vec![64, 108],  // Left Alt, Right Alt
            keycode_e: None,
            keycode_q: None,
            modifier_timeout: Duration::from_millis(800), // Generous timeout
            debounce_timeout: Duration::from_millis(300), // Prevent rapid triggers
        }
    }

    /// Track key press event
    pub fn key_pressed(&mut self, keycode: Keycode) {
        self.pressed_keys.insert(keycode);

        #[cfg(debug_assertions)]
        println!(
            "Key {} pressed. Total keys: {:?}",
            keycode, self.pressed_keys
        );

        self.update_state_machine();
    }

    /// Track key release event
    pub fn key_released(&mut self, keycode: Keycode) {
        self.pressed_keys.remove(&keycode);

        #[cfg(debug_assertions)]
        println!(
            "Key {} released. Remaining keys: {:?}",
            keycode, self.pressed_keys
        );

        // Don't update state machine on release - we only care about presses for shortcuts
        // The state will auto-reset after debounce timeout or when checked
    }

    /// Main state machine logic
    fn update_state_machine(&mut self) {
        let now = Instant::now();

        // Check current modifier states
        let ctrl_pressed = self.is_ctrl_pressed();
        let shift_pressed = self.is_shift_pressed();
        let alt_pressed = self.is_alt_pressed();

        match &self.state {
            ShortcutState::Idle => {
                // Transition to ModifiersPressed if any modifier is held
                if ctrl_pressed || shift_pressed || alt_pressed {
                    #[cfg(debug_assertions)]
                    println!(
                        "State: Idle → ModifiersPressed (ctrl={}, shift={}, alt={})",
                        ctrl_pressed, shift_pressed, alt_pressed
                    );

                    self.state = ShortcutState::ModifiersPressed {
                        ctrl: ctrl_pressed,
                        shift: shift_pressed,
                        alt: alt_pressed,
                        timestamp: now,
                    };
                }
            }

            ShortcutState::ModifiersPressed {
                ctrl,
                shift,
                alt,
                timestamp,
            } => {
                // Check if modifiers have been released (go back to idle)
                if !ctrl_pressed && !shift_pressed && !alt_pressed {
                    #[cfg(debug_assertions)]
                    println!("State: ModifiersPressed → Idle (all modifiers released)");

                    self.state = ShortcutState::Idle;
                    return;
                }

                // Check if modifier combination has stabilized
                if ctrl_pressed == *ctrl && shift_pressed == *shift && alt_pressed == *alt {
                    // Same combination for stability check - transition to awaiting target
                    if now.duration_since(*timestamp) > Duration::from_millis(50) {
                        #[cfg(debug_assertions)]
                        println!("State: ModifiersPressed → AwaitingTargetKey (stable for 50ms)");

                        self.state = ShortcutState::AwaitingTargetKey {
                            ctrl: ctrl_pressed,
                            shift: shift_pressed,
                            alt: alt_pressed,
                            timestamp: now,
                        };
                    }
                } else {
                    // Modifier combination changed - update the state
                    #[cfg(debug_assertions)]
                    println!(
                        "State: ModifiersPressed (combination changed: ctrl={}, shift={}, alt={})",
                        ctrl_pressed, shift_pressed, alt_pressed
                    );

                    self.state = ShortcutState::ModifiersPressed {
                        ctrl: ctrl_pressed,
                        shift: shift_pressed,
                        alt: alt_pressed,
                        timestamp: now,
                    };
                }
            }

            ShortcutState::AwaitingTargetKey {
                ctrl,
                shift,
                alt,
                timestamp,
            } => {
                // Check for timeout
                if now.duration_since(*timestamp) > self.modifier_timeout {
                    #[cfg(debug_assertions)]
                    println!("State: AwaitingTargetKey → Idle (timeout)");

                    self.state = ShortcutState::Idle;
                    return;
                }

                // Check if modifiers are still pressed
                if ctrl_pressed != *ctrl || shift_pressed != *shift || alt_pressed != *alt {
                    #[cfg(debug_assertions)]
                    println!("State: AwaitingTargetKey → Idle (modifier state changed)");

                    self.state = ShortcutState::Idle;
                    return;
                }

                // Check for target keys (non-modifiers that were just pressed)
                let non_modifier_keys: HashSet<_> = self
                    .pressed_keys
                    .iter()
                    .filter(|&&k| !self.is_modifier_key(k))
                    .cloned()
                    .collect();

                if !non_modifier_keys.is_empty() {
                    // Target key(s) pressed - determine shortcut type
                    if let Some(shortcut_type) =
                        self.determine_shortcut_type(*ctrl, *shift, *alt, &non_modifier_keys)
                    {
                        #[cfg(debug_assertions)]
                        println!("State: AwaitingTargetKey → Complete ({:?})", shortcut_type);

                        self.state = ShortcutState::Complete {
                            shortcut_type,
                            timestamp: now,
                        };
                    } else {
                        // Unknown combination - back to idle
                        #[cfg(debug_assertions)]
                        println!("State: AwaitingTargetKey → Idle (unknown target key)");

                        self.state = ShortcutState::Idle;
                    }
                }
            }

            ShortcutState::Complete { shortcut_type, .. } => {
                // Stay in Complete state until shortcut is consumed via check_* methods
                // This ensures the shortcut is detected even if keys are released quickly
                #[cfg(debug_assertions)]
                if self.pressed_keys.is_empty() {
                    println!(
                        "State: Complete({:?}) - all keys released, waiting to be consumed",
                        shortcut_type
                    );
                }
            }
        }
    }

    /// Check if specific shortcut combinations are active
    pub fn check_ctrl_shift_e(&mut self, _keycode_e: u8) -> bool {
        let detected = matches!(
            self.state,
            ShortcutState::Complete {
                shortcut_type: ShortcutType::CtrlShiftE,
                ..
            }
        );

        if detected {
            // Reset state after detection to prevent repeated triggers
            self.state = ShortcutState::Idle;
            #[cfg(debug_assertions)]
            println!("[OK] Ctrl+Shift+E consumed, resetting to Idle");
        }

        detected
    }

    pub fn check_ctrl_shift_q(&mut self, _keycode_q: u8) -> bool {
        let detected = matches!(
            self.state,
            ShortcutState::Complete {
                shortcut_type: ShortcutType::CtrlShiftQ,
                ..
            }
        );

        if detected {
            self.state = ShortcutState::Idle;
            #[cfg(debug_assertions)]
            println!("[OK] Ctrl+Shift+Q consumed, resetting to Idle");
        }

        detected
    }

    pub fn check_ctrl_q(&mut self, _keycode_q: u8) -> bool {
        let detected = matches!(
            self.state,
            ShortcutState::Complete {
                shortcut_type: ShortcutType::CtrlQ,
                ..
            }
        );

        if detected {
            self.state = ShortcutState::Idle;
            #[cfg(debug_assertions)]
            println!("[OK] Ctrl+Q consumed, resetting to Idle");
        }

        detected
    }

    pub fn check_ctrl_alt_e(&mut self, _keycode_e: u8) -> bool {
        let detected = matches!(
            self.state,
            ShortcutState::Complete {
                shortcut_type: ShortcutType::CtrlAltE,
                ..
            }
        );

        if detected {
            self.state = ShortcutState::Idle;
            #[cfg(debug_assertions)]
            println!("[OK] Ctrl+Alt+E consumed, resetting to Idle");
        }

        detected
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

    fn is_alt_pressed(&self) -> bool {
        self.alt_keycodes
            .iter()
            .any(|&k| self.pressed_keys.contains(&k))
    }

    fn is_modifier_key(&self, keycode: Keycode) -> bool {
        self.ctrl_keycodes.contains(&keycode)
            || self.shift_keycodes.contains(&keycode)
            || self.alt_keycodes.contains(&keycode)
    }

    fn determine_shortcut_type(
        &self,
        ctrl: bool,
        shift: bool,
        alt: bool,
        target_keys: &HashSet<Keycode>,
    ) -> Option<ShortcutType> {
        if target_keys.len() == 1 {
            let target = *target_keys.iter().next().unwrap();
            let keycode_e = self.keycode_e.unwrap_or(26); // Fallback to typical E key
            let keycode_q = self.keycode_q.unwrap_or(24); // Fallback to typical Q key

            match (ctrl, shift, alt, target) {
                (true, true, false, k) if k == keycode_e => Some(ShortcutType::CtrlShiftE),
                (true, true, false, k) if k == keycode_q => Some(ShortcutType::CtrlShiftQ),
                (true, false, false, k) if k == keycode_q => Some(ShortcutType::CtrlQ),
                (true, false, true, k) if k == keycode_e => Some(ShortcutType::CtrlAltE),
                _ => None,
            }
        } else {
            None // Multiple target keys not supported
        }
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

        if let Some(alt) = modifier_mapper.get_keycode(0xffe9) {
            if !self.alt_keycodes.contains(&alt) {
                self.alt_keycodes.push(alt);
            }
        }
        if let Some(alt_r) = modifier_mapper.get_keycode(0xffea) {
            if !self.alt_keycodes.contains(&alt_r) {
                self.alt_keycodes.push(alt_r);
            }
        }

        // Store target keycodes
        self.keycode_e = modifier_mapper.get_keycode(0x0065); // E key
        self.keycode_q = modifier_mapper.get_keycode(0x0071); // Q key

        #[cfg(debug_assertions)]
        println!(
            "Updated keycodes: ctrl={:?}, shift={:?}, alt={:?}, e={:?}, q={:?}",
            self.ctrl_keycodes,
            self.shift_keycodes,
            self.alt_keycodes,
            self.keycode_e,
            self.keycode_q
        );
    }

    /// Get currently pressed keys
    pub fn get_pressed_keys(&self) -> Vec<Keycode> {
        self.pressed_keys.iter().copied().collect()
    }

    /// Cleanup functions
    pub fn clear_all_keys(&mut self) {
        self.pressed_keys.clear();
        self.state = ShortcutState::Idle;

        #[cfg(debug_assertions)]
        println!("State: → Idle (manual reset)");
    }

    pub fn cleanup_stale_keys(&mut self) {
        if self.pressed_keys.len() > 8 {
            #[cfg(debug_assertions)]
            println!(
                "Warning: Too many pressed keys ({}), performing cleanup",
                self.pressed_keys.len()
            );

            self.clear_all_keys();
        }
    }

    pub fn reset_modifier_states(&mut self) {
        // Only reset if we're in a state that might be stuck
        match self.state {
            ShortcutState::Complete { timestamp, .. } => {
                if timestamp.elapsed() > Duration::from_secs(2) {
                    self.state = ShortcutState::Idle;
                    #[cfg(debug_assertions)]
                    println!("State: Complete → Idle (forced reset - stuck state)");
                }
            }
            ShortcutState::AwaitingTargetKey { timestamp, .. } => {
                if timestamp.elapsed() > Duration::from_secs(5) {
                    self.state = ShortcutState::Idle;
                    #[cfg(debug_assertions)]
                    println!("State: AwaitingTargetKey → Idle (forced reset - stuck state)");
                }
            }
            _ => {} // Don't interfere with normal state transitions
        }
    }

    /// Getters for compatibility
    pub fn ctrl_keycode(&self) -> Option<Keycode> {
        self.ctrl_keycodes.first().copied()
    }

    pub fn shift_keycode(&self) -> Option<Keycode> {
        self.shift_keycodes.first().copied()
    }

    pub fn alt_keycode(&self) -> Option<Keycode> {
        self.alt_keycodes.first().copied()
    }
}
