use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Configuration for the overlay window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayConfig {
    /// X position of the window
    #[serde(default = "default_x")]
    pub x: i16,
    /// Y position of the window
    #[serde(default = "default_y")]
    pub y: i16,
    /// Width of the window
    #[serde(default = "default_width")]
    pub width: u16,
    /// Height of the window
    #[serde(default = "default_height")]
    pub height: u16,
    /// ARGB color (e.g., 0x80FF0000 for 50% transparent red)
    #[serde(default = "default_color")]
    pub color: u32,
    /// Text color (RGB format, e.g., 0xFFFFFF for white)
    #[serde(default = "default_text_color")]
    pub text_color: u32,
    /// Text outline/shadow color (RGB format, e.g., 0x000000 for black)
    #[serde(default = "default_text_outline_color")]
    pub text_outline_color: u32,
    /// Font name (X11 font string)
    #[serde(default = "default_font")]
    pub font: String,
    /// Gemini API key (optional, falls back to env var)
    #[serde(default)]
    pub gemini_api_key: Option<String>,
}

// Default value functions for serde
fn default_x() -> i16 {
    100
}
fn default_y() -> i16 {
    100
}
fn default_width() -> u16 {
    800
}
fn default_height() -> u16 {
    600
}
fn default_color() -> u32 {
    0x80000000
}
fn default_text_color() -> u32 {
    0xFFFFFF
}
fn default_text_outline_color() -> u32 {
    0x000000
}
fn default_font() -> String {
    "-misc-fixed-medium-r-normal--20-200-75-75-C-100-iso8859-1".to_string()
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            x: default_x(),
            y: default_y(),
            width: default_width(),
            height: default_height(),
            color: default_color(),
            text_color: default_text_color(),
            text_outline_color: default_text_outline_color(),
            font: default_font(),
            // 🔑 HARDCODE YOUR API KEY HERE
            gemini_api_key: Some("YOUR_GEMINI_API_KEY_HERE".to_string()),
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

    pub fn with_font(mut self, font: String) -> Self {
        self.font = font;
        self
    }

    /// Load configuration from a YAML file
    /// Falls back to default values for missing fields
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let config: OverlayConfig = serde_yaml::from_str(&contents)?;
        Ok(config)
    }

    /// Load configuration from file if it exists, otherwise use defaults
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        Self::from_file(path).unwrap_or_default()
    }

    /// Try to load from config file, with fallback chain:
    /// 1. Provided path (if Some)
    /// 2. ./overlay.yml in current directory
    /// 3. Default values
    pub fn load(path: Option<String>) -> Self {
        // Try provided path first
        if let Some(p) = path {
            if let Ok(config) = Self::from_file(&p) {
                #[cfg(debug_assertions)]
                eprintln!("Loaded config from: {}", p);
                return config;
            }
        }

        // Try default location in current directory
        let default_path = "overlay.yml";
        if Path::new(default_path).exists() {
            if let Ok(config) = Self::from_file(default_path) {
                #[cfg(debug_assertions)]
                eprintln!("Loaded config from: {}", default_path);
                return config;
            }
        }

        // Fall back to defaults
        #[cfg(debug_assertions)]
        eprintln!("Using default configuration");
        Self::default()
    }

    /// Save configuration to a YAML file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let yaml = serde_yaml::to_string(self)?;
        fs::write(path, yaml)?;
        Ok(())
    }
}
