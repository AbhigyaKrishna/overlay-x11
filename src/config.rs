/// Configuration for the overlay window
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    /// X position of the window
    pub x: i16,
    /// Y position of the window
    pub y: i16,
    /// Width of the window
    pub width: u16,
    /// Height of the window
    pub height: u16,
    /// ARGB color (e.g., 0x80FF0000 for 50% transparent red)
    pub color: u32,
    /// Text color (RGB format, e.g., 0xFFFFFF for white)
    pub text_color: u32,
    /// Text outline/shadow color (RGB format, e.g., 0x000000 for black)
    pub text_outline_color: u32,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            x: 100,
            y: 100,
            width: 800,
            height: 600,
            color: 0x80000000,            // 50% alpha red
            text_color: 0xFFFFFF,         // white
            text_outline_color: 0x000000, // black outline
        }
    }
}

impl OverlayConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_position(mut self, x: i16, y: i16) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    pub fn with_size(mut self, width: u16, height: u16) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_color(mut self, color: u32) -> Self {
        self.color = color;
        self
    }

    pub fn with_text_color(mut self, color: u32) -> Self {
        self.text_color = color;
        self
    }

    pub fn with_text_outline_color(mut self, color: u32) -> Self {
        self.text_outline_color = color;
        self
    }
}
