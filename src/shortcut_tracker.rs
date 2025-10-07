use crate::modifier_mapper::ModifierMapper;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use x11rb::protocol::xproto::Keycode;

/// State machine states for shortcut detection
#[derive(Debug, Clone, PartialEq)]
enum ShortcutState {
    Idle,
    AwaitingTargetKey {
        ctrl: bool,
        shift: bool,
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

    // Target key keycodes
    keycode_e: Option<Keycode>,
    keycode_q: Option<Keycode>,

    // Configuration
    modifier_timeout: Duration, // Timeout for awaiting target key after modifiers
}

impl ShortcutTracker {
    pub fn new() -> Self {
        Self {
            pressed_keys: HashSet::new(),
            state: ShortcutState::Idle,
            ctrl_keycodes: vec![37, 105], // Left Ctrl, Right Ctrl
            shift_keycodes: vec![50, 62], // Left Shift, Right Shift
            keycode_e: None,
            keycode_q: None,
            modifier_timeout: Duration::from_millis(500), // Responsive timeout
        }
    }

    /// Track key press event
    pub fn key_pressed(&mut self, keycode: Keycode) {
        self.pressed_keys.insert(keycode);
        self.update_state_machine();
    }

    /// Track key release event
    pub fn key_released(&mut self, keycode: Keycode) {
        self.pressed_keys.remove(&keycode);
    }

    /// Main state machine logic
    fn update_state_machine(&mut self) {
        let now = Instant::now();

        // Check current modifier states
        let ctrl_pressed = self.is_ctrl_pressed();
        let shift_pressed = self.is_shift_pressed();

        match &self.state {
            ShortcutState::Idle => {
                if ctrl_pressed && shift_pressed {
                    self.state = ShortcutState::AwaitingTargetKey {
                        ctrl: ctrl_pressed,
                        shift: shift_pressed,
                        timestamp: now,
                    };
                }
            }

            ShortcutState::AwaitingTargetKey {
                ctrl,
                shift,
                timestamp,
            } => {
                if now.duration_since(*timestamp) > self.modifier_timeout {
                    self.state = ShortcutState::Idle;
                    return;
                }

                if ctrl_pressed != *ctrl || shift_pressed != *shift {
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
                    if let Some(shortcut_type) =
                        self.determine_shortcut_type(*ctrl, *shift, &non_modifier_keys)
                    {
                        self.state = ShortcutState::Complete {
                            shortcut_type,
                            timestamp: now,
                        };
                    } else {
                        self.state = ShortcutState::Idle;
                    }
                }
            }

            ShortcutState::Complete { .. } => {}
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
            if self.is_ctrl_pressed() && self.is_shift_pressed() {
                self.state = ShortcutState::AwaitingTargetKey {
                    ctrl: true,
                    shift: true,
                    timestamp: Instant::now(),
                };
            } else {
                self.state = ShortcutState::Idle;
            }
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
            if self.is_ctrl_pressed() && self.is_shift_pressed() {
                self.state = ShortcutState::AwaitingTargetKey {
                    ctrl: true,
                    shift: true,
                    timestamp: Instant::now(),
                };
            } else {
                self.state = ShortcutState::Idle;
            }
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

    fn is_modifier_key(&self, keycode: Keycode) -> bool {
        self.ctrl_keycodes.contains(&keycode) || self.shift_keycodes.contains(&keycode)
    }

    fn determine_shortcut_type(
        &self,
        ctrl: bool,
        shift: bool,
        target_keys: &HashSet<Keycode>,
    ) -> Option<ShortcutType> {
        if target_keys.len() == 1 {
            let target = *target_keys.iter().next().unwrap();
            let keycode_e = self.keycode_e.unwrap_or(26);
            let keycode_q = self.keycode_q.unwrap_or(24);

            match (ctrl, shift, target) {
                (true, true, k) if k == keycode_e => Some(ShortcutType::CtrlShiftE),
                (true, true, k) if k == keycode_q => Some(ShortcutType::CtrlShiftQ),
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

        self.keycode_e = modifier_mapper.get_keycode(0x0065);
        self.keycode_q = modifier_mapper.get_keycode(0x0071);
    }

    /// Get currently pressed keys
    pub fn get_pressed_keys(&self) -> Vec<Keycode> {
        self.pressed_keys.iter().copied().collect()
    }

    /// Cleanup functions
    pub fn clear_all_keys(&mut self) {
        self.pressed_keys.clear();
        self.state = ShortcutState::Idle;
    }

    pub fn cleanup_stale_keys(&mut self) {
        if self.pressed_keys.len() > 8 {
            self.clear_all_keys();
        }
    }

    pub fn reset_modifier_states(&mut self) {
        match self.state {
            ShortcutState::Complete { timestamp, .. } => {
                if timestamp.elapsed() > Duration::from_secs(2) {
                    self.state = ShortcutState::Idle;
                }
            }
            ShortcutState::AwaitingTargetKey { timestamp, .. } => {
                if timestamp.elapsed() > Duration::from_secs(5) {
                    self.state = ShortcutState::Idle;
                }
            }
            _ => {}
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
