use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;

const GEMINI_API_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent";

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Part {
    Text { text: String },
    InlineData { inline_data: InlineData },
}

#[derive(Serialize)]
struct InlineData {
    mime_type: String,
    data: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: ResponseContent,
}

#[derive(Deserialize)]
struct ResponseContent {
    parts: Vec<ResponsePart>,
}

#[derive(Deserialize)]
struct ResponsePart {
    text: String,
}

/// Analyze a screenshot using Gemini API (from PNG data in memory)
pub fn analyze_screenshot_data(png_data: &[u8], api_key: &str) -> Result<String, Box<dyn Error>> {
    // Base64 encode the PNG data
    let base64_image = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png_data);

    // Create the request with a meta prompt
    let meta_prompt = r#"Analyze this screenshot and provide a concise summary of what you see. 
Focus on:
1. Main applications or windows visible
2. Key content or activities
3. Any notable UI elements or patterns
4. Overall context of what the user is doing

Keep the response brief (3-5 sentences) and informative."#;

    let request = GeminiRequest {
        contents: vec![Content {
            parts: vec![
                Part::Text {
                    text: meta_prompt.to_string(),
                },
                Part::InlineData {
                    inline_data: InlineData {
                        mime_type: "image/png".to_string(),
                        data: base64_image,
                    },
                },
            ],
        }],
    };

    // Make the API request
    let client = reqwest::blocking::Client::new();
    let url = format!("{}?key={}", GEMINI_API_URL, api_key);

    let response = client.post(&url).json(&request).send()?;

    if !response.status().is_success() {
        let error_text = response.text()?;
        return Err(format!("Gemini API error: {}", error_text).into());
    }

    let gemini_response: GeminiResponse = response.json()?;

    // Extract the text from the response
    if let Some(candidate) = gemini_response.candidates.first() {
        if let Some(part) = candidate.content.parts.first() {
            return Ok(part.text.clone());
        }
    }

    Err("No response from Gemini API".into())
}

/// Analyze a screenshot using Gemini API (deprecated - use analyze_screenshot_data)
#[deprecated(note = "Use analyze_screenshot_data to avoid saving files")]
pub fn analyze_screenshot(image_path: &str, api_key: &str) -> Result<String, Box<dyn Error>> {
    // Read the image file
    let image_data = fs::read(image_path)?;
    analyze_screenshot_data(&image_data, api_key)
}

/// Get API key from environment variable
pub fn get_api_key() -> Result<String, Box<dyn Error>> {
    std::env::var("GEMINI_API_KEY")
        .map_err(|_| "GEMINI_API_KEY environment variable not set".into())
}
