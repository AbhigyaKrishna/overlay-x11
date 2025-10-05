use std::error::Error;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

use crate::config::OverlayConfig;

pub struct Renderer {
    config: OverlayConfig,
    font: Option<Font>,
    text: String,
}

impl Renderer {
    pub fn new(config: OverlayConfig) -> Self {
        Self {
            config,
            font: None,
            text: String::new(),
        }
    }

    pub fn with_font(mut self, font: Font) -> Self {
        self.font = Some(font);
        self
    }

    pub fn with_text(mut self, text: String) -> Self {
        self.text = text;
        self
    }

    /// Render the overlay on the given window
    pub fn render(&self, conn: &RustConnection, window: u32) -> Result<(), Box<dyn Error>> {
        // Draw translucent background
        let gc_bg = conn.generate_id()?;
        conn.create_gc(
            gc_bg,
            window,
            &CreateGCAux::new().foreground(self.config.color),
        )?;

        conn.poly_fill_rectangle(
            window,
            gc_bg,
            &[Rectangle {
                x: 0,
                y: 0,
                width: self.config.width,
                height: self.config.height,
            }],
        )?;
        conn.free_gc(gc_bg)?;

        // Draw text if font is set and text is not empty
        if let Some(font) = self.font {
            if !self.text.is_empty() {
                let gc_text = conn.generate_id()?;
                let white = 0xFFFFFFFFu32; // opaque white
                conn.create_gc(
                    gc_text,
                    window,
                    &CreateGCAux::new().foreground(white).font(font),
                )?;

                // Use poly_text8 for transparent background (instead of image_text8)
                // Split text by newlines and render each line
                let mut y = 40;
                for line in self.text.lines() {
                    if !line.is_empty() {
                        conn.poly_text8(window, gc_text, 20, y, line.as_bytes())?;
                    }
                    y += 20; // Line spacing
                }
                conn.free_gc(gc_text)?;
            }
        }

        conn.flush()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn config(&self) -> &OverlayConfig {
        &self.config
    }
}
