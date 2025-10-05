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
        let wrapped_lines = self.wrap_text();
        let line_count = wrapped_lines.len() as i16;
        let max_offset = (line_count * line_height) - self.config.height as i16;
        self.scroll_offset = (self.scroll_offset + line_height).min(max_offset.max(0));
    }

    /// Wrap text to fit within overlay width, accounting for margins
    fn wrap_text(&self) -> Vec<String> {
        let margin_x = 20i16;
        let usable_width = (self.config.width as i16 - margin_x * 2).max(100) as usize;
        // Approximate characters that fit (assuming ~6 pixels per char for fixed font)
        let chars_per_line = (usable_width / 6).max(10);

        let mut wrapped_lines = Vec::new();

        for line in self.text.lines() {
            if line.is_empty() {
                wrapped_lines.push(String::new());
                continue;
            }

            let mut current_line = String::new();
            let words: Vec<&str> = line.split_whitespace().collect();

            for (i, word) in words.iter().enumerate() {
                let word_with_space = if i == 0 {
                    word.to_string()
                } else {
                    format!(" {}", word)
                };

                // Check if adding this word would exceed the line width
                if current_line.len() + word_with_space.len() > chars_per_line {
                    // If current line is not empty, save it and start new line
                    if !current_line.is_empty() {
                        wrapped_lines.push(current_line.clone());
                        current_line.clear();
                        current_line.push_str(word);
                    } else {
                        // Single word is too long, split it
                        if word.len() > chars_per_line {
                            for chunk in word.as_bytes().chunks(chars_per_line) {
                                if let Ok(s) = std::str::from_utf8(chunk) {
                                    wrapped_lines.push(s.to_string());
                                }
                            }
                        } else {
                            current_line.push_str(word);
                        }
                    }
                } else {
                    current_line.push_str(&word_with_space);
                }
            }

            // Add any remaining text
            if !current_line.is_empty() {
                wrapped_lines.push(current_line);
            }
        }

        wrapped_lines
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

                // Wrap text to fit overlay width
                let wrapped_lines = self.wrap_text();

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
                    for line in &wrapped_lines {
                        // Check if any part of the text line is visible
                        // Text extends from (y - ascent) to (y + descent)
                        let text_top = y - self.font_ascent as i16;
                        let text_bottom = y + self.font_descent as i16;
                        if text_bottom >= 0 && text_top < self.config.height as i16 {
                            // image_text8 has a max length of 255 bytes, split long lines
                            let line_bytes = line.as_bytes();
                            for chunk in line_bytes.chunks(255) {
                                conn.image_text8(window, gc_outline, 20 + dx, y + dy, chunk)?;
                            }
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
                for line in &wrapped_lines {
                    // Check if any part of the text line is visible
                    let text_top = y - self.font_ascent as i16;
                    let text_bottom = y + self.font_descent as i16;
                    if text_bottom >= 0 && text_top < self.config.height as i16 {
                        // image_text8 has a max length of 255 bytes, split long lines
                        let line_bytes = line.as_bytes();
                        let mut x_offset = 20i16;
                        for chunk in line_bytes.chunks(255) {
                            conn.image_text8(window, gc_text, x_offset, y, chunk)?;
                            // Calculate approximate width of this chunk to offset next chunk
                            // Using average character width (this is approximate)
                            x_offset += (chunk.len() as i16) * 6; // Rough estimate for fixed font
                        }
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
