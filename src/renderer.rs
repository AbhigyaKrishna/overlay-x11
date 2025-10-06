use std::error::Error;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

use crate::config::OverlayConfig;

pub struct Renderer {
    config: OverlayConfig,
    font: Option<Font>,
    lines: Vec<(String, i32)>, // text lines + pixel width
    font_ascent: u16,
    font_descent: u16,
    scroll_offset: i16,
    horizontal_scroll_offset: i16,
}

impl Renderer {
    pub fn new(config: OverlayConfig) -> Self {
        Self {
            config,
            font: None,
            lines: Vec::new(),
            font_ascent: 0,
            font_descent: 0,
            scroll_offset: 0,
            horizontal_scroll_offset: 0,
        }
    }

    pub fn with_font(mut self, font: Font, ascent: u16, descent: u16) -> Self {
        self.font = Some(font);
        self.font_ascent = ascent;
        self.font_descent = descent;
        self
    }

    pub fn with_text(
        mut self,
        conn: &RustConnection,
        text: String,
    ) -> Result<Self, Box<dyn Error>> {
        // Break into lines and compute widths using QueryTextExtents
        let mut lines = Vec::new();
        if let Some(font) = self.font {
            for line in text.lines() {
                let line_bytes = line.as_bytes();
                if line_bytes.is_empty() {
                    lines.push((String::new(), 0));
                    continue;
                }

                // Query text extents to get precise pixel width
                // x11rb's query_text_extents expects &[Char2b] for 16-bit chars
                // For 8-bit text, we need to convert bytes to Char2b
                let chars: Vec<Char2b> = line_bytes
                    .iter()
                    .map(|&b| Char2b { byte1: 0, byte2: b })
                    .collect();

                let reply = conn.query_text_extents(font, &chars)?.reply()?;
                // Calculate total width
                let width = reply.overall_width as i32;
                lines.push((line.to_string(), width));
            }
        } else {
            // No font set, just store lines with zero width
            for line in text.lines() {
                lines.push((line.to_string(), 0));
            }
        }
        self.lines = lines;
        Ok(self)
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
        let line_count = self.lines.len() as i16;
        let max_offset = (line_count * line_height) - self.config.height as i16;
        self.scroll_offset = (self.scroll_offset + line_height).min(max_offset.max(0));
    }

    pub fn scroll_left(&mut self) {
        // Scroll left by 100 pixels
        self.horizontal_scroll_offset = self.horizontal_scroll_offset.saturating_sub(100);
    }

    pub fn scroll_right(&mut self) {
        // Compute max line width from measured widths
        let max_width = self.lines.iter().map(|(_, w)| *w).max().unwrap_or(0);
        let limit = (max_width - self.config.width as i32 + 40).max(0);
        self.horizontal_scroll_offset = (self.horizontal_scroll_offset + 100).min(limit as i16);
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

        // Draw text if font is set and we have lines
        if let Some(font) = self.font {
            if !self.lines.is_empty() {
                let line_height = (self.font_ascent + self.font_descent) as i16 + 4; // padding

                // Calculate initial y position with scroll offset
                let base_y = self.font_ascent as i16 + 20 - self.scroll_offset;

                // Draw outline + text in passes
                for &(dx, dy, color) in &[
                    (-1, -1, self.config.text_outline_color),
                    (1, -1, self.config.text_outline_color),
                    (-1, 1, self.config.text_outline_color),
                    (1, 1, self.config.text_outline_color),
                    (0, 0, self.config.text_color),
                ] {
                    let gc = conn.generate_id()?;
                    conn.create_gc(gc, window, &CreateGCAux::new().foreground(color).font(font))?;

                    let mut y = base_y;
                    for (line, width) in &self.lines {
                        // Vertical clipping: check if any part of the text line is visible
                        let text_top = y - self.font_ascent as i16;
                        let text_bottom = y + self.font_descent as i16;

                        if text_bottom >= 0 && text_top < self.config.height as i16 {
                            // Horizontal clipping: only draw if line extends beyond scroll offset
                            if *width as i16 > self.horizontal_scroll_offset {
                                let x_pos = 20 - self.horizontal_scroll_offset + dx;

                                // image_text8 has a max length of 255 bytes, split long lines
                                let line_bytes = line.as_bytes();
                                let mut x_offset = x_pos;

                                for chunk in line_bytes.chunks(255) {
                                    // Only draw chunks that are at least partially visible
                                    if x_offset < self.config.width as i16 && x_offset + 100 > 0 {
                                        conn.image_text8(window, gc, x_offset, y + dy, chunk)?;
                                    }
                                    // Query chunk width for accurate positioning
                                    if !chunk.is_empty() {
                                        let chars: Vec<Char2b> = chunk
                                            .iter()
                                            .map(|&b| Char2b { byte1: 0, byte2: b })
                                            .collect();
                                        let chunk_reply =
                                            conn.query_text_extents(font, &chars)?.reply()?;
                                        x_offset += chunk_reply.overall_width as i16;
                                    }
                                }
                            }
                        }
                        y += line_height;
                    }

                    conn.free_gc(gc)?;
                }
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
