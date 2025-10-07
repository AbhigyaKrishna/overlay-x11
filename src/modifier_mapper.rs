use std::collections::HashMap;
use std::error::Error;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

/// Dynamic modifier detection and mapping system
pub struct ModifierMapper {
    modifier_map: GetModifierMappingReply,
    keysym_to_keycode: HashMap<u32, Keycode>,
    alt_modifiers: Vec<ModMask>,
}

impl ModifierMapper {
    /// Create a new modifier mapper by querying the X server
    pub fn new(conn: &RustConnection) -> Result<Self, Box<dyn Error>> {
        // Get current modifier mapping
        let modifier_map = conn.get_modifier_mapping()?.reply()?;

        // Build keycode lookup table
        let setup = conn.setup();
        let min_keycode = setup.min_keycode;
        let max_keycode = setup.max_keycode;

        let keyboard_mapping = conn
            .get_keyboard_mapping(min_keycode, max_keycode - min_keycode + 1)?
            .reply()?;

        let mut keysym_to_keycode = HashMap::new();
        let keysyms_per_keycode = keyboard_mapping.keysyms_per_keycode as usize;

        for (i, chunk) in keyboard_mapping
            .keysyms
            .chunks(keysyms_per_keycode)
            .enumerate()
        {
            let keycode = min_keycode + i as u8;
            for &keysym in chunk {
                if keysym != 0 {
                    keysym_to_keycode.insert(keysym, keycode);
                }
            }
        }

        let alt_modifiers = Self::detect_alt_modifiers(&modifier_map, &keysym_to_keycode);

        Ok(ModifierMapper {
            modifier_map,
            keysym_to_keycode,
            alt_modifiers,
        })
    }

    /// Detect which modifier masks correspond to Alt keys
    fn detect_alt_modifiers(
        modifier_map: &GetModifierMappingReply,
        keysym_lookup: &HashMap<u32, Keycode>,
    ) -> Vec<ModMask> {
        let mut alt_masks = Vec::new();

        // X11 keysyms for Alt keys
        const ALT_KEYSYMS: &[u32] = &[
            0xffe9, // Alt_L
            0xffea, // Alt_R
            0xffe7, // Meta_L
            0xffe8, // Meta_R
        ];

        // Check each modifier (Mod1 through Mod5)
        let modifier_masks = [
            ModMask::M1,
            ModMask::M2,
            ModMask::M3,
            ModMask::M4,
            ModMask::M5,
        ];

        for (mod_idx, &mask) in modifier_masks.iter().enumerate() {
            let mod_keycodes = get_modifier_keycodes(modifier_map, mod_idx + 3); // Mod1=3, etc.

            for &keycode in &mod_keycodes {
                // Check if this keycode corresponds to an Alt key
                for &alt_keysym in ALT_KEYSYMS {
                    if keysym_lookup.get(&alt_keysym) == Some(&keycode) {
                        if !alt_masks.contains(&mask) {
                            alt_masks.push(mask);
                        }
                        break;
                    }
                }
            }
        }

        // Fallback: if no Alt detected, assume common mappings
        if alt_masks.is_empty() {
            alt_masks.extend_from_slice(&[ModMask::M1, ModMask::M4]); // Common Alt locations
        }

        #[cfg(debug_assertions)]
        println!(
            "Debug: Detected Alt modifiers: {:?}",
            alt_masks
                .iter()
                .map(|m| format!("{:?}", m))
                .collect::<Vec<_>>()
        );

        alt_masks
    }

    /// Get all possible modifier combinations including lock modifiers
    pub fn get_all_modifier_combinations(&self, base_modifiers: ModMask) -> Vec<ModMask> {
        let mut combinations = Vec::new();

        // Lock modifiers that can interfere
        let lock_modifiers = [
            ModMask::from(0u16), // No lock
            ModMask::M2,         // Usually NumLock
            ModMask::LOCK,       // CapsLock
            ModMask::M5,         // Often Scroll Lock or AltGr
        ];

        // Generate all combinations with lock modifiers
        for &lock1 in &lock_modifiers {
            for &lock2 in &lock_modifiers {
                let combined = base_modifiers | lock1 | lock2;
                if !combinations.contains(&combined) {
                    combinations.push(combined);
                }
            }
        }

        combinations
    }

    /// Check if event state matches the target base modifiers (ignoring lock keys)
    pub fn matches_modifier_combo(&self, event_state: ModMask, target_base: ModMask) -> bool {
        // Check if event matches any of the possible combinations
        let combinations = self.get_all_modifier_combinations(target_base);

        for combo in combinations {
            if event_state == combo {
                return true;
            }
        }

        false
    }

    /// Get all possible combinations for Alt key presses
    pub fn get_alt_combinations(&self) -> Vec<ModMask> {
        let mut combinations = Vec::new();

        for &alt_mask in &self.alt_modifiers {
            combinations.extend(self.get_all_modifier_combinations(alt_mask));
        }

        combinations
    }

    /// Get all possible combinations for Ctrl+Alt key presses
    pub fn get_ctrl_alt_combinations(&self) -> Vec<ModMask> {
        let mut combinations = Vec::new();

        for &alt_mask in &self.alt_modifiers {
            let base = ModMask::CONTROL | alt_mask;
            combinations.extend(self.get_all_modifier_combinations(base));
        }

        combinations
    }

    /// Check if the event state matches Ctrl+Alt combination
    pub fn matches_ctrl_alt(&self, event_state: ModMask) -> bool {
        for &alt_mask in &self.alt_modifiers {
            let base = ModMask::CONTROL | alt_mask;
            if self.matches_modifier_combo(event_state, base) {
                return true;
            }
        }
        false
    }

    /// Convert a keysym to a keycode
    pub fn get_keycode(&self, keysym: u32) -> Option<Keycode> {
        self.keysym_to_keycode.get(&keysym).copied()
    }

    /// Refresh modifier mapping when keyboard layout changes
    pub fn refresh(&mut self, conn: &RustConnection) -> Result<(), Box<dyn Error>> {
        *self = Self::new(conn)?;
        Ok(())
    }
}

/// Helper function to extract keycodes for a specific modifier
fn get_modifier_keycodes(modifier_map: &GetModifierMappingReply, modifier_index: usize) -> Vec<Keycode> {
    let mut keycodes = Vec::new();
    let keycodes_per_modifier = modifier_map.keycodes_per_modifier();
    let start_idx = modifier_index * keycodes_per_modifier as usize;
    let end_idx = start_idx + keycodes_per_modifier as usize;

    if end_idx <= modifier_map.keycodes.len() {
        for &keycode in &modifier_map.keycodes[start_idx..end_idx] {
            if keycode != 0 {
                keycodes.push(keycode);
            }
        }
    }

    keycodes
}
