use std::collections::HashMap;
use std::error::Error;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

/// Minimal keysym to keycode mapper
pub struct ModifierMapper {
    keysym_to_keycode: HashMap<u32, Keycode>,
}

impl ModifierMapper {
    /// Create a new modifier mapper by querying the X server
    pub fn new(conn: &RustConnection) -> Result<Self, Box<dyn Error>> {
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

        Ok(ModifierMapper { keysym_to_keycode })
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
