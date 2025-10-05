use std::error::Error;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

use crate::config::OverlayConfig;

pub struct Renderer {
    config: OverlayConfig,
    font: Option<Font>,
    text: String,
    font_ascent: u16,
    font_descent: u16,
    scroll_offset: i16,
}

impl Renderer {
    pub fn new(config: OverlayConfig) -> Self {
        Self {
            config,
            font: None,
            text: String::new(),
            font_ascent: 0,
            font_descent: 0,
            scroll_offset: 0,
        }
    }

    pub fn with_font(mut self, font: Font, ascent: u16, descent: u16) -> Self {
        self.font = Some(font);
        self.font_ascent = ascent;
        self.font_descent = descent;
        self
    }

    pub fn with_text(mut self, text: String) -> Self {
        self.text = text;
        self
    }

    pub fn with_scroll_offset(mut self, offset: i16) -> Self {
        self.scroll_offset = offset;
        self
    }

    pub fn scroll_offset(&self) -> i16 {
        self.scroll_offset
    }

    pub fn scroll_up(&mut self) {
        let line_height = (self.font_ascent + self.font_descent + 4) as i16;
        self.scroll_offset = (self.scroll_offset - line_height).max(0);
    }

    pub fn scroll_down(&mut self) {
        let line_height = (self.font_ascent + self.font_descent + 4) as i16;
        let line_count = self.text.lines().count() as i16;
        let max_offset = (line_count * line_height) - self.config.height as i16;
        self.scroll_offset = (self.scroll_offset + line_height).min(max_offset.max(0));
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
                let line_height = (self.font_ascent + self.font_descent) as i16 + 4; // padding

                // Calculate initial y position with scroll offset
                let base_y = self.font_ascent as i16 + 20 - self.scroll_offset;

                // Draw outline/shadow in 4 directions
                for &(dx, dy) in &[(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                    let gc_outline = conn.generate_id()?;
                    conn.create_gc(
                        gc_outline,
                        window,
                        &CreateGCAux::new()
                            .foreground(self.config.text_outline_color)
                            .background(self.config.color)
                            .font(font),
                    )?;

                    let mut y = base_y;
                    for line in self.text.lines() {
                        // Check if any part of the text line is visible
                        // Text extends from (y - ascent) to (y + descent)
                        let text_top = y - self.font_ascent as i16;
                        let text_bottom = y + self.font_descent as i16;
                        if text_bottom >= 0 && text_top < self.config.height as i16 {
                            conn.image_text8(window, gc_outline, 20 + dx, y + dy, line.as_bytes())?;
                        }
                        y += line_height;
                    }
                    conn.free_gc(gc_outline)?;
                }

                // Draw main text on top
                let gc_text = conn.generate_id()?;
                conn.create_gc(
                    gc_text,
                    window,
                    &CreateGCAux::new()
                        .foreground(self.config.text_color)
                        .background(self.config.color)
                        .font(font),
                )?;

                let mut y = base_y;
                for line in self.text.lines() {
                    // Check if any part of the text line is visible
                    let text_top = y - self.font_ascent as i16;
                    let text_bottom = y + self.font_descent as i16;
                    if text_bottom >= 0 && text_top < self.config.height as i16 {
                        conn.image_text8(window, gc_text, 20, y, line.as_bytes())?;
                    }
                    y += line_height;
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
